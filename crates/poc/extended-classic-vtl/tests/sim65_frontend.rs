use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "vtlxx-frontend-sim65-{}-{nonce}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(tool: &str, args: &[&OsStr], input: &[u8]) -> Output {
    use std::io::Write;
    let mut child = Command::new(tool)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("start {tool}: {error}"));
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn build_and_run_input(input: &[u8]) -> Output {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = [
        "tests/fixtures/sim65/frontend_basic.s",
        "6502/frontend/basic.s",
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
    let temp = TempDir::new();
    let mut objects = Vec::new();
    for (index, source_file) in sources.iter().enumerate() {
        let object = temp.0.join(format!("{index}.o"));
        let source_file = root.join(source_file);
        let output = run(
            "ca65",
            &[
                OsStr::new("-t"),
                OsStr::new("sim6502"),
                source_file.as_os_str(),
                OsStr::new("-o"),
                object.as_os_str(),
            ],
            &[],
        );
        assert!(
            output.status.success(),
            "assemble {source_file:?}: {output:?}"
        );
        objects.push(object);
    }
    let executable = temp.0.join("frontend.prg");
    let mut args = vec![
        OsStr::new("-t"),
        OsStr::new("sim6502"),
        OsStr::new("-o"),
        executable.as_os_str(),
    ];
    args.extend(objects.iter().map(|object| object.as_os_str()));
    args.push(OsStr::new("sim6502.lib"));
    let output = run("ld65", &args, &[]);
    assert!(output.status.success(), "link: {output:?}");

    run(
        "sim65",
        &[
            OsStr::new("-x"),
            OsStr::new("4000000"),
            executable.as_os_str(),
        ],
        input,
    )
}

fn build_and_run(source: &[u8], runtime_input: &[u8]) -> Output {
    let mut input = vec![source.len() as u8, (source.len() >> 8) as u8];
    input.extend_from_slice(source);
    input.extend_from_slice(runtime_input);
    build_and_run_input(&input)
}

fn expect(source: &str, status: i32, output: &[u8]) {
    let result = build_and_run(source.as_bytes(), b"Z");
    assert_eq!(result.status.code(), Some(status), "{source}: {result:?}");
    assert_eq!(result.stdout, output, "{source}: {result:?}");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn basic_source_compiles_and_executes() {
    expect("", 0, b"Z");
    expect("A=40", 0, b"Z");
    expect("A=40 ?=A", 0, b"40Z");
    expect("A=40\nA=A+2\n?=A", 0, b"42Z");
    expect("%=1 [ ] ?=7", 0, b"7Z");
    expect("%=1 [ ?=1 ]", 0, b"1Z");
    expect("%=0 [ ?=1 ]", 0, b"Z");
    expect("%=1 [ ?=1 ] [ ?=2 ]", 0, b"1Z");
    expect("%=0 [ ?=1 ] [ ?=2 ]", 0, b"2Z");
    expect("%=1 [ ?=1 ] ; else follows comment\n [ ?=2 ]", 0, b"1Z");
    expect("%=1 [ ] ?=3", 0, b"3Z");
    expect("%=1 [ %=0 [ ?=1 ] [ ?=2 ] ] [ ?=3 ]", 0, b"2Z");
    expect("%=0 [ %=1 [ ?=1 ] ] [ ?=2 ]", 0, b"2Z");
    expect("==VALUE,42 %=1 [ ?=VALUE ]", 0, b"42Z");
    expect("%=1 [ #=2 ?=1 ^=2 ?=8 ]", 0, b"8Z");
    let nested_16 = format!("{}?=9{}", "%=1 [ ".repeat(16), " ]".repeat(16));
    expect(&nested_16, 0, b"9Z");
    let grouped_condition = format!("%={}1{} [ ?=9 ]", "(".repeat(16), ")".repeat(16));
    expect(&grouped_condition, 0, b"9Z");
    expect("A=2+3*4 ?=A $=65", 0, b"20AZ");
    expect("A=2+(3*4) ?=A", 0, b"14Z");
    expect("@=10,42 ?=@(10)", 0, b"42Z");
    expect("~=40 A=~+2 ?=A", 0, b"42Z");
    expect("~=10,20 @=~ ?=@(10)", 0, b"20Z");
    expect("?=-32768 ?=32767", 0, b"-3276832767Z");
    expect("?=2==2 ?=2!=3 ?=2<3 ?=2<=2 ?=3>2 ?=3>=3", 0, b"111111Z");
    expect("?=2==3 ?=2!=2 ?=3<2 ?=3<=2 ?=2>3 ?=2>=3", 0, b"000000Z");
    expect("?=7/2 ?=7%2 ?=3-8", 0, b"31-5Z");
    expect("==BOARD_BASE,16 A=BOARD_BASE+2 ?=A", 0, b"18Z");
    expect("==NEG_ONE,-1 A=NEG_ONE ?=A", 0, b"-1Z");
    // The first jump reaches END; the later backward jump is compiled and
    // patched but remains unreachable, keeping this fixture finite.
    expect(
        "==LOOP,1 ==END,2 A=LOOP #=END ^=LOOP #=LOOP ^=END ?=A",
        0,
        b"1Z",
    );
    expect("#=32767 ^=32767 #=0 ^=0 ?=7", 0, b"7Z");
    expect(
        "==SAME_ONE,1 ==SAME_TWO,1 A=SAME_ONE+SAME_TWO ?=A",
        0,
        b"2Z",
    );
    expect("==ABCDEFGHIJKLMNOP,32767 ?=ABCDEFGHIJKLMNOP", 0, b"32767Z");
    expect(
        "==C0,100 ==C1,101 ==C2,102 ==C3,103 ==C4,104 ==C5,105 ==C6,106 ==C7,107 ==C8,108 ==C9,109 ==C10,110 ==C11,111 ==C12,112 ==C13,113 ==ABCDEFGHIJKLMNOP,114 ==C15,115 ?=C0 ?=C7 ?=C13 ?=ABCDEFGHIJKLMNOP ?=C15",
        0,
        b"100107113114115Z",
    );
    expect("==ZERO,0 ?=2==2", 0, b"1Z");
    expect("A=1 ; a comment\n\tA=A+1 ?=A", 0, b"2Z");
    let long_source = format!("{}?=42", " ".repeat(254));
    expect(&long_source, 0, b"42Z");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn malformed_source_never_starts_runtime() {
    for source in [
        "A=",
        "A=32768",
        "A=-32769",
        "A=1+",
        "A=1+~",
        "A=(~+1)",
        "A=1,~+1",
        "[=1",
        "A=[",
        "A=[+1",
        "A=Q(1)",
        "A=@(1",
        "A=1 garbage",
        "?=1 ?=",
        "A=1+z",
        "A=BEFORE\n==BEFORE,1",
        "==DUP,1 ==DUP,2",
        "==A,1",
        "==aB,1",
        "==AB$,1",
        "==ABCDEFGHIJKLMNOPQ,1",
        "==LOW,-32769",
        "==HIGH,32768",
        "==EXPR,1+2",
        "==REF,OTHER",
        "==C0,0 ==C1,1 ==C2,2 ==C3,3 ==C4,4 ==C5,5 ==C6,6 ==C7,7 ==C8,8 ==C9,9 ==C10,10 ==C11,11 ==C12,12 ==C13,13 ==C14,14 ==C15,15 ==C16,16",
        "A=UNKNOWN",
        "^=",
        "#=",
        "^=12x",
        "#=UNKNOWN",
        "%=1 A=2",
        "%=1 [ A=2",
        "%=1 [ ] [ A=",
        "%=1 [ ] [",
        "%= [ ]",
        "[ ]",
        "]",
        "%=1 [ ] ]",
        "==NEG,-1 ^=NEG",
        "^=-1",
        "#=32768",
        "^=32768",
        "^=12 ^=12",
        "#=12",
    ] {
        expect(
            source,
            if source == "^=12 ^=12" || source == "#=12" {
                4
            } else if source.contains("3276")
                || source.contains("32768")
                || source == "==REF,OTHER"
                || source == "==NEG,-1 ^=NEG"
                || source == "^=-1"
            {
                3
            } else {
                2
            },
            b"",
        );
    }
    let nested_17 = format!("{}?=9{}", "%=1 [ ".repeat(17), " ]".repeat(17));
    expect(&nested_17, 2, b"");
    let mut too_many_labels = String::new();
    for label in 0..33 {
        too_many_labels.push_str(&format!("^={label} "));
    }
    expect(&too_many_labels, 4, b"");

    let mut too_many_fixups = String::new();
    for label in 0..33 {
        too_many_fixups.push_str(&format!("#={label} "));
    }
    expect(&too_many_fixups, 4, b"");
    let source = "A=1 ".repeat(300);
    expect(&source, 4, b"");
    let source = format!("{}A=~ A=~", "A=1 ".repeat(170));
    expect(&source, 4, b"");
    let truncated = build_and_run_input(&[4, 0, b'?', b'=', b'1']);
    assert_eq!(truncated.status.code(), Some(1), "{truncated:?}");
    assert!(truncated.stdout.is_empty(), "{truncated:?}");
}
