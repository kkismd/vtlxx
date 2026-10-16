use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const CYCLE_LIMIT: u32 = 4_000_000;
const QUEENS_CYCLE_LIMIT: u32 = 256_000_000;
const ARENA_CAPACITY: u16 = 2048;
const RESOURCE_RECORD_LEN: usize = 24;
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("temporary directory clock: {error}"))?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "vtlxx-resource-sim65-{}-{nonce}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)
            .map_err(|error| format!("create temporary directory {}: {error}", path.display()))?;
        Ok(Self(path))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Workload {
    name: &'static str,
    source: &'static [u8],
    expected_stdout: &'static [u8],
    cycle_limit: u32,
}

const WORKLOADS: &[Workload] = &[
    Workload {
        name: "small-smoke",
        source: b"A=40\nA=A+2\n?=A\n",
        expected_stdout: b"42",
        cycle_limit: CYCLE_LIMIT,
    },
    Workload {
        name: "if-while",
        source: include_bytes!("fixtures/resource_usage/if_while.vtl"),
        expected_stdout: b"1",
        cycle_limit: CYCLE_LIMIT,
    },
    Workload {
        name: "named-write",
        source: include_bytes!("fixtures/resource_usage/named_write.vtl"),
        expected_stdout: b"42",
        cycle_limit: CYCLE_LIMIT,
    },
    Workload {
        name: "storage",
        source: include_bytes!("fixtures/resource_usage/storage.vtl"),
        expected_stdout: b"42",
        cycle_limit: CYCLE_LIMIT,
    },
    Workload {
        name: "p0a-template",
        source: include_bytes!("../examples/p0a_template_calibration.vtl"),
        expected_stdout: b"58\n20\n40\n",
        cycle_limit: CYCLE_LIMIT,
    },
    Workload {
        name: "eight-queens",
        source: include_bytes!("../examples/eight_queens.vtl"),
        expected_stdout: include_bytes!("fixtures/eight_queens_example_output.txt"),
        cycle_limit: QUEENS_CYCLE_LIMIT,
    },
    Workload {
        name: "eight-queens-storage",
        source: include_bytes!("../examples/eight_queens_storage.vtl"),
        expected_stdout: include_bytes!("fixtures/eight_queens_example_output.txt"),
        cycle_limit: QUEENS_CYCLE_LIMIT,
    },
];

#[derive(Debug, PartialEq, Eq)]
struct Metrics {
    code_bytes: u16,
    template_source_bytes: u16,
    value_stack_cells: u8,
    hardware_stack_bytes: u8,
    storage_cells: u8,
    storage_extent: u8,
    source_work_stack_cells: u8,
    numeric_labels_per_owner: u8,
    unresolved_fixups_per_owner: u8,
    structured_nesting: u8,
    while_nesting: u8,
    runtime_chunks: u8,
    named_integers: u8,
    completed_owners: u8,
    published_writes: u8,
    published_templates: u8,
    pending_patch_entries: u16,
    expression_nesting: u8,
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored --nocapture"]
fn resource_usage_stays_within_the_ec06_profile() {
    let temp = TempDir::new().expect("temporary directory");
    let mut rows = Vec::new();

    for workload in WORKLOADS {
        let source_path = temp.0.join(format!("{}.vtl", workload.name));
        fs::write(&source_path, workload.source).expect("write source workload");

        let rust = run(env!("CARGO_BIN_EXE_ecvtl"), &[source_path.as_os_str()], &[])
            .unwrap_or_else(|error| panic!("{}: Rust reference: {error}", workload.name));
        assert!(rust.status.success(), "{}: {rust:?}", workload.name);
        assert_eq!(rust.stdout, workload.expected_stdout, "{}", workload.name);

        let cycle_limit = workload.cycle_limit.to_string();
        let baseline = run(
            env!("CARGO_BIN_EXE_ecvtl6502"),
            &[
                source_path.as_os_str(),
                OsStr::new("--cycle-limit"),
                OsStr::new(&cycle_limit),
            ],
            &[],
        )
        .unwrap_or_else(|error| panic!("{}: production 6502 build: {error}", workload.name));
        assert!(baseline.status.success(), "{}: {baseline:?}", workload.name);
        assert_eq!(
            baseline.status.code(),
            rust.status.code(),
            "{}: baseline exit",
            workload.name
        );
        assert_eq!(
            baseline.stdout, rust.stdout,
            "{}: Rust/6502 stdout",
            workload.name
        );

        let instrumented = build_and_run_instrumented(workload.source, workload.cycle_limit, 0xa5)
            .unwrap_or_else(|error| panic!("{}: instrumented sim65: {error}", workload.name));
        assert!(
            instrumented.status.success(),
            "{}: {instrumented:?}",
            workload.name
        );
        assert_eq!(
            instrumented.status.code(),
            baseline.status.code(),
            "{}: instrumented exit",
            workload.name
        );
        assert!(
            instrumented.stdout.starts_with(&baseline.stdout),
            "{}: instrumented output changed workload stdout (base {}, instrumented {})",
            workload.name,
            describe_bytes(&baseline.stdout),
            describe_bytes(&instrumented.stdout)
        );
        let record = &instrumented.stdout[baseline.stdout.len()..];
        let mut metrics =
            Metrics::decode(record).unwrap_or_else(|error| panic!("{}: {error}", workload.name));

        // A distinct canary byte per run rules out a stack write that happens
        // to equal the first run's marker at the deepest occupied address.
        let alternate = build_and_run_instrumented(workload.source, workload.cycle_limit, 0x5a)
            .unwrap_or_else(|error| panic!("{}: alternate stack probe: {error}", workload.name));
        assert_eq!(
            alternate.status.code(),
            baseline.status.code(),
            "{}: alternate exit",
            workload.name
        );
        assert!(
            alternate.stdout.starts_with(&baseline.stdout),
            "{}: alternate stdout",
            workload.name
        );
        let alternate_record = &alternate.stdout[baseline.stdout.len()..];
        let mut alternate_metrics = Metrics::decode(alternate_record)
            .unwrap_or_else(|error| panic!("{}: alternate {error}", workload.name));
        alternate_metrics.hardware_stack_bytes = metrics.hardware_stack_bytes;
        assert_eq!(
            alternate_metrics, metrics,
            "{}: canary runs disagree",
            workload.name
        );
        metrics.hardware_stack_bytes = metrics.hardware_stack_bytes.max(
            Metrics::decode(alternate_record)
                .expect("decoded above")
                .hardware_stack_bytes,
        );
        assert_within_profile(workload.name, &metrics);
        println!("{}: {metrics:?}", workload.name);
        rows.push((workload.name, metrics));
    }

    print_markdown_table(&rows);
}

impl Metrics {
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != RESOURCE_RECORD_LEN {
            return Err(format!(
                "resource record has {} bytes, expected {RESOURCE_RECORD_LEN}: {}",
                bytes.len(),
                describe_bytes(bytes)
            ));
        }
        if &bytes[..3] != b"RU1" {
            return Err(format!(
                "invalid resource record signature: {}",
                describe_bytes(bytes)
            ));
        }
        let word = |offset| u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        Ok(Self {
            code_bytes: word(3),
            template_source_bytes: word(5),
            value_stack_cells: bytes[7],
            hardware_stack_bytes: bytes[8],
            storage_cells: bytes[9],
            storage_extent: bytes[10],
            source_work_stack_cells: bytes[11],
            numeric_labels_per_owner: bytes[12],
            unresolved_fixups_per_owner: bytes[13],
            structured_nesting: bytes[14],
            while_nesting: bytes[15],
            runtime_chunks: bytes[16],
            named_integers: bytes[17],
            completed_owners: bytes[18],
            published_writes: bytes[19],
            published_templates: bytes[20],
            pending_patch_entries: word(21),
            expression_nesting: bytes[23],
        })
    }
}

fn assert_within_profile(name: &str, metrics: &Metrics) {
    let shared_arena = metrics.code_bytes + metrics.template_source_bytes;
    assert!(
        shared_arena <= ARENA_CAPACITY,
        "{name}: shared arena uses {shared_arena}/{} B (code {}, retained template source {})",
        ARENA_CAPACITY,
        metrics.code_bytes,
        metrics.template_source_bytes
    );
    assert!(metrics.value_stack_cells <= 16, "{name}: {metrics:?}");
    assert!(metrics.hardware_stack_bytes <= 128, "{name}: {metrics:?}");
    assert!(metrics.storage_cells <= 128, "{name}: {metrics:?}");
    assert!(metrics.storage_extent <= 128, "{name}: {metrics:?}");
    assert!(metrics.source_work_stack_cells <= 16, "{name}: {metrics:?}");
    assert!(
        metrics.numeric_labels_per_owner <= 32,
        "{name}: {metrics:?}"
    );
    assert!(
        metrics.unresolved_fixups_per_owner <= 32,
        "{name}: {metrics:?}"
    );
    assert!(metrics.structured_nesting <= 16, "{name}: {metrics:?}");
    assert!(metrics.while_nesting <= 16, "{name}: {metrics:?}");
    assert!(metrics.runtime_chunks <= 53, "{name}: {metrics:?}");
    assert!(metrics.named_integers <= 16, "{name}: {metrics:?}");
    assert!(metrics.published_writes <= 26, "{name}: {metrics:?}");
    assert!(metrics.published_templates <= 26, "{name}: {metrics:?}");
    assert!(metrics.pending_patch_entries <= 2048, "{name}: {metrics:?}");
    assert!(metrics.expression_nesting <= 16, "{name}: {metrics:?}");
}

fn print_markdown_table(rows: &[(&str, Metrics)]) {
    println!(
        "\n| workload | code B | template source B | shared arena B | value stack Cells | hardware stack B | storage Cells / extent | source work stack | labels / owner | fixups / owner | pending patches | expression depth | IF depth | While depth | chunks | constants | completed owners | Write publishes | templates |"
    );
    println!(
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"
    );
    for (name, metrics) in rows {
        println!(
            "| {name} | {} | {} | {} | {} | {} | {} / {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            metrics.code_bytes,
            metrics.template_source_bytes,
            metrics.code_bytes + metrics.template_source_bytes,
            metrics.value_stack_cells,
            metrics.hardware_stack_bytes,
            metrics.storage_cells,
            metrics.storage_extent,
            metrics.source_work_stack_cells,
            metrics.numeric_labels_per_owner,
            metrics.unresolved_fixups_per_owner,
            metrics.pending_patch_entries,
            metrics.expression_nesting,
            metrics.structured_nesting,
            metrics.while_nesting,
            metrics.runtime_chunks,
            metrics.named_integers,
            metrics.completed_owners,
            metrics.published_writes,
            metrics.published_templates,
        );
    }
}

fn run(binary: &str, args: &[&OsStr], input: &[u8]) -> Result<Output, String> {
    let mut child = Command::new(binary)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start {binary}: {error}"))?;
    if !input.is_empty() {
        use std::io::Write;
        child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(input)
            .map_err(|error| format!("write {binary} stdin: {error}"))?;
    }
    child
        .wait_with_output()
        .map_err(|error| format!("collect {binary} output: {error}"))
}

fn build_and_run_instrumented(
    source: &[u8],
    cycle_limit: u32,
    canary: u8,
) -> Result<Output, String> {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = [
        "6502/target/resource_probe.s",
        "6502/frontend/basic.s",
        "6502/frontend/template.s",
        "6502/frontend/template_reader.s",
        "6502/frontend/source.s",
        "6502/compiler/arena.s",
        "6502/compiler/emitter.s",
        "6502/compiler/owner.s",
        "6502/compiler/binding.s",
        "6502/compiler/work_stack.s",
        "6502/runtime/state.s",
        "6502/runtime/primitives.s",
        "6502/runtime/arithmetic.s",
        "6502/target/sim65_adapter.s",
    ];
    let temp = TempDir::new()?;
    let mut objects = Vec::with_capacity(sources.len());
    for (index, source_path) in sources.iter().enumerate() {
        let source_path = crate_root.join(source_path);
        let object = temp.0.join(format!("{index}.o"));
        let mut assemble_args = vec![
            OsStr::new("-t"),
            OsStr::new("sim6502"),
            OsStr::new("-D"),
            OsStr::new("RESOURCE_USAGE_PROBE"),
        ];
        let canary_define;
        if index == 0 {
            canary_define = format!("RESOURCE_CANARY={canary}");
            assemble_args.extend([OsStr::new("-D"), OsStr::new(&canary_define)]);
        }
        assemble_args.extend([
            source_path.as_os_str(),
            OsStr::new("-o"),
            object.as_os_str(),
        ]);
        let output = run("ca65", &assemble_args, &[])?;
        require_success("assemble", "ca65", &output)?;
        objects.push(object);
    }

    let executable = temp.0.join("resource-probe.prg");
    let mut link_args = vec![
        OsStr::new("-t"),
        OsStr::new("sim6502"),
        OsStr::new("-o"),
        executable.as_os_str(),
    ];
    link_args.extend(objects.iter().map(|object| object.as_os_str()));
    link_args.push(OsStr::new("sim6502.lib"));
    let output = run("ld65", &link_args, &[])?;
    require_success("link", "ld65", &output)?;

    let cycle_limit = cycle_limit.to_string();
    let input = frame_source(source)?;
    run(
        "sim65",
        &[
            OsStr::new("-x"),
            OsStr::new(&cycle_limit),
            executable.as_os_str(),
        ],
        &input,
    )
}

fn frame_source(source: &[u8]) -> Result<Vec<u8>, String> {
    let len = u16::try_from(source.len()).map_err(|_| "source exceeds u16 framing".to_string())?;
    let mut framed = Vec::with_capacity(2 + source.len());
    framed.extend_from_slice(&len.to_le_bytes());
    framed.extend_from_slice(source);
    Ok(framed)
}

fn require_success(stage: &str, tool: &str, output: &Output) -> Result<(), String> {
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{stage}: {tool} exited with {}; stdout: {}; stderr: {}",
            output.status,
            describe_bytes(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn describe_bytes(bytes: &[u8]) -> String {
    format!("{} bytes {:?}", bytes.len(), String::from_utf8_lossy(bytes))
}
