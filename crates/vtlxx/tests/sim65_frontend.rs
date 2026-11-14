use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "common/removed_templates.rs"]
mod removed_templates;

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
    build_and_run_input_with_frontend(input, "tests/fixtures/sim65/frontend_basic.s")
}

fn build_and_run_input_with_frontend(input: &[u8], frontend_fixture: &str) -> Output {
    build_and_run_custom(input, frontend_fixture, None, None)
}

fn build_and_run_custom(
    input: &[u8],
    frontend_fixture: &str,
    basic: Option<&str>,
    entropy: Option<i16>,
) -> Output {
    build_and_run_custom_configured(input, frontend_fixture, basic, entropy, false)
}

fn build_and_run_custom_configured(
    input: &[u8],
    frontend_fixture: &str,
    basic: Option<&str>,
    entropy: Option<i16>,
    entropy_sequence: bool,
) -> Output {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = [
        frontend_fixture,
        "6502/target/zp_layout.s",
        "6502/frontend/basic.s",
        "6502/frontend/source.s",
        "6502/compiler/arena.s",
        "6502/compiler/emitter.s",
        "6502/compiler/owner.s",
        "6502/compiler/binding.s",
        "6502/runtime/state.s",
        "6502/runtime/primitives.s",
        "6502/runtime/arithmetic.s",
        "6502/target/sim65_adapter.s",
    ];
    let temp = TempDir::new();
    if let Some(basic) = basic {
        fs::write(temp.0.join("basic.s"), basic).unwrap();
    }
    let mut objects = Vec::new();
    for (index, source_file) in sources.iter().enumerate() {
        let object = temp.0.join(format!("{index}.o"));
        let source_file = if basic.is_some() && *source_file == "6502/frontend/basic.s" {
            temp.0.join("basic.s")
        } else {
            root.join(source_file)
        };
        let mut args = vec![OsStr::new("-t"), OsStr::new("sim6502")];
        let entropy_define;
        if (entropy.is_some() || entropy_sequence)
            && (source_file.ends_with("6502/target/sim65_adapter.s")
                || source_file.ends_with("6502/runtime/state.s"))
        {
            args.extend([OsStr::new("-D"), OsStr::new("ENTROPY16_TEST")]);
        }
        if entropy_sequence {
            args.extend([OsStr::new("-D"), OsStr::new("ENTROPY16_TEST_SEQUENCE")]);
        } else if let Some(entropy) = entropy
            && source_file.ends_with("6502/target/sim65_adapter.s")
        {
            entropy_define =
                OsString::from(format!("ENTROPY16_TEST_VALUE=${:04X}", entropy as u16));
            args.extend([OsStr::new("-D"), entropy_define.as_os_str()]);
        }
        args.extend([
            source_file.as_os_str(),
            OsStr::new("-o"),
            object.as_os_str(),
        ]);
        let output = run("ca65", &args, &[]);
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
            OsStr::new("200000000"),
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

fn build_and_run_read_char(source: &[u8], runtime_input: &[u8]) -> Output {
    let mut input = vec![source.len() as u8, (source.len() >> 8) as u8];
    input.extend_from_slice(source);
    input.extend_from_slice(runtime_input);
    build_and_run_input_with_frontend(&input, "tests/fixtures/sim65/frontend_read_char.s")
}

fn expect(source: &str, status: i32, output: &[u8]) {
    let result = build_and_run(source.as_bytes(), b"Z");
    assert_eq!(result.status.code(), Some(status), "{source}: {result:?}");
    assert_eq!(result.stdout, output, "{source}: {result:?}");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn entropy_primary_read_returns_the_xorshift16_sequence() {
    for (raw, expected) in [
        (0, [27015, 30930, 2970, 32697, 2984, 6343]),
        (1, [16576, 12304, 29900, 5893, 23247, 27857]),
        (-1, [16319, 12239, 25515, 18160, 4399, 27244]),
        (i16::MIN, [24608, 26648, 20010, 23687, 22344, 753]),
        (i16::MAX, [24479, 18391, 11649, 6775, 18023, 26781]),
        (-21279, [27015, 30930, 2970, 32697, 2984, 6343]),
    ] {
        let source = (0..6).map(|_| "?='").collect::<Vec<_>>().join(" ");
        let mut input = (source.len() as u16).to_le_bytes().to_vec();
        input.extend_from_slice(source.as_bytes());
        let result = build_and_run_custom(
            &input,
            "tests/fixtures/sim65/frontend_basic.s",
            None,
            Some(raw),
        );
        assert_eq!(result.status.code(), Some(0), "{raw}: {result:?}");
        let expected = expected.iter().map(i16::to_string).collect::<String>();
        assert_eq!(result.stdout, expected.as_bytes(), "{raw}: {result:?}");
    }

    let source = b"@=192,' ?=@(192)";
    let mut input = (source.len() as u16).to_le_bytes().to_vec();
    input.extend_from_slice(source);
    let result = build_and_run_custom(
        &input,
        "tests/fixtures/sim65/frontend_basic.s",
        None,
        Some(1),
    );
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    assert_eq!(result.stdout, b"16576", "{result:?}");

    let source = b"?='+'";
    let mut input = (source.len() as u16).to_le_bytes().to_vec();
    input.extend_from_slice(source);
    let result = build_and_run_custom(
        &input,
        "tests/fixtures/sim65/frontend_basic.s",
        None,
        Some(1),
    );
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    assert_eq!(result.stdout, b"28880", "{result:?}");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn each_entropy_evaluation_calls_the_adapter_once() {
    let source = b"?=' ?='";
    let mut input = (source.len() as u16).to_le_bytes().to_vec();
    input.extend_from_slice(source);
    let output = build_and_run_custom_configured(
        &input,
        "tests/fixtures/sim65/frontend_basic.s",
        None,
        None,
        true,
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"1657612304", "{output:?}");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn unavailable_entropy_is_fatal_and_apostrophe_rhs_forms_are_rejected() {
    let mut input = (3u16).to_le_bytes().to_vec();
    input.extend_from_slice(b"?='");
    let result = build_and_run_input(&input);
    assert_eq!(result.status.code(), Some(5), "{result:?}");
    assert!(result.stdout.is_empty(), "{result:?}");

    for source in ["?='=1", "?='(1)"] {
        let result = build_and_run(source.as_bytes(), b"");
        assert_eq!(result.status.code(), Some(2), "{source}: {result:?}");
        assert!(result.stdout.is_empty(), "{source}: {result:?}");
    }
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn phase_transition_initializes_runtime_and_preserves_return_state() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let basic = fs::read_to_string(root.join("6502/frontend/basic.s")).unwrap();
    let marker = "    jsr rt_init";
    assert_eq!(basic.matches(marker).count(), 1);
    // All compilation and publication have completed here. Fill the entire
    // shared range with a nonzero pattern before runtime initialization.
    let basic = basic.replace(marker, "    lda entry\n    sta phase_expected_entry\n    lda entry+1\n    sta phase_expected_entry+1\n    ldx #99\n    lda #$a5\n@poison_phase:\n    sta zp_phase,x\n    dex\n    bpl @poison_phase\n    jsr rt_init");
    let basic = format!(
        "{basic}\n.importzp zp_phase\n.export phase_expected_entry\n.segment \"BSS\"\nphase_expected_entry: .res 2\n"
    );
    let fixture = r#"
.setcpu "6502"
.export _main
.import fe_compile_run, serial_in, halt, phase_expected_entry
.importzp fe_compile_status, cc_status, rt_depth
.segment "CODE"
_main:
    jsr fe_compile_run
    bcs failed
    cmp phase_expected_entry
    bne failed
    cpx phase_expected_entry+1
    bne failed
    lda fe_compile_status
    ora cc_status
    ora rt_depth
    bne failed
    jsr serial_in
    bcc failed
    lda fe_compile_status
    ora cc_status
    bne failed
    lda #0
    jmp halt
failed:
    lda #99
    jmp halt
"#;
    let temp = TempDir::new();
    let fixture_path = temp.0.join("phase_transition.s");
    fs::write(&fixture_path, fixture).unwrap();
    let source = format!(
        "{}?=$ ?=$ ?=-32768 ?=32767",
        (b'A'..=b'Z')
            .map(|reg| format!("?={} ", char::from(reg)))
            .collect::<String>()
    );
    let mut input = (source.len() as u16).to_le_bytes().to_vec();
    input.extend_from_slice(source.as_bytes());
    let result = build_and_run_custom(&input, fixture_path.to_str().unwrap(), Some(&basic), None);
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    assert_eq!(
        result.stdout, b"00000000000000000000000000-1-1-3276832767",
        "{result:?}"
    );
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn invalid_constant_does_not_publish_a_partial_entry() {
    let basic =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("6502/frontend/basic.s"))
            .unwrap();
    let basic = basic.replacen(
        "@failed:\n    ; Abort only an active temporary source save.",
        "@failed:\n    lda constant_count\n    cmp #1\n    beq :+\n    lda #99\n    jmp halt\n:\n    ; Abort only an active temporary source save.",
        1,
    );
    for invalid in [
        ".BAD:=1+2",
        ".BAD:=32768",
        ".BAD:=1A=2",
        ".GOOD:=2",
        ".BAD :=",
    ] {
        let source = format!(".GOOD:=1 {invalid}");
        let mut input = (source.len() as u16).to_le_bytes().to_vec();
        input.extend_from_slice(source.as_bytes());
        let result = build_and_run_custom(
            &input,
            "tests/fixtures/sim65/frontend_basic.s",
            Some(&basic),
            None,
        );
        assert!(
            matches!(result.status.code(), Some(2 | 3)),
            "{source}: {result:?}"
        );
        assert!(result.stdout.is_empty(), "{source}: {result:?}");
    }
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn full_constant_table_survives_failed_declarations_and_references() {
    let original =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("6502/frontend/basic.s"))
            .unwrap();
    for (count, invalid) in [
        (127, ".BAD:=32768"),
        (128, ".N0:=7"),
        (128, "A=.MISSING"),
        (128, ".ABCDEFGHIJKLMNOPQ:=1"),
        (128, ".EXTRA:=1"),
    ] {
        let check = format!(
            "@failed:\n    lda constant_count\n    cmp #{count}\n    bne @table_corrupt\n    ldx #0\n@check_entry:\n    stx constant_slot\n    jsr constant_address\n    ldy #0\n@find_end:\n    lda (constant_ptr),y\n    beq @check_value\n    iny\n    cpy #17\n    bcc @find_end\n    bcs @table_corrupt\n@check_value:\n    iny\n    txa\n    cmp (constant_ptr),y\n    bne @table_corrupt\n    iny\n    lda (constant_ptr),y\n    bne @table_corrupt\n    inx\n    cpx #{count}\n    bne @check_entry\n    jmp @table_intact\n@table_corrupt:\n    lda #99\n    jmp halt\n@table_intact:\n    ; Abort only an active temporary source save."
        );
        let basic = original.replacen(
            "@failed:\n    ; Abort only an active temporary source save.",
            &check,
            1,
        );
        let definitions: String = (0..count).map(|i| format!(".N{i}:={i}\n")).collect();
        let source = format!("?=42\n{definitions}{invalid}\n");
        let mut input = (source.len() as u16).to_le_bytes().to_vec();
        input.extend_from_slice(source.as_bytes());
        let result = build_and_run_custom(
            &input,
            "tests/fixtures/sim65/frontend_basic.s",
            Some(&basic),
            None,
        );
        assert!(
            matches!(result.status.code(), Some(2 | 3)),
            "{invalid}: {result:?}"
        );
        assert!(result.stdout.is_empty(), "{invalid}: {result:?}");
    }
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn micro_trek_passes_the_old_constant_limit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read(root.join("examples/micro_trek.vtl")).unwrap();
    let mut input = (source.len() as u16).to_le_bytes().to_vec();
    input.extend_from_slice(&source);
    input.extend_from_slice(b"0\n");
    let result = build_and_run_input(&input);
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    assert!(result.stdout.starts_with(b"IT IS STARDATE"), "{result:?}");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn basic_source_compiles_and_executes() {
    expect("", 0, b"Z");
    expect("A=40", 0, b"Z");
    expect("A=40 ?=A", 0, b"40Z");
    expect("A=40\nA=A+2\n?=A", 0, b"42Z");
    expect("?=()", 0, b"\nZ");
    expect("?=() ?=()", 0, b"\n\nZ");
    expect("~=42 ?=() ?=~", 0, b"\n42Z");
    expect("?=1 ?=() ?=2", 0, b"1\n2Z");
    expect("?=(42)", 0, b"42Z");
    expect("?=(1)+2", 0, b"3Z");
    expect("?=() ; comment\n?=7", 0, b"\n7Z");
    expect("?=1 ?=2 A=3 ?=A ?=4", 0, b"1234Z");
    expect("&=p [ ] p=()", 0, b"Z");
    expect("&=p [ ?=~ ] p=42", 0, b"42Z");
    expect("&=p [ ?=~ ] p=(40+2)", 0, b"42Z");
    expect("&=p [ ?=~ ?=~ ] p=10,42", 0, b"4210Z");
    expect("&=p [ ] &=q [ ] p=() q=()", 0, b"Z");
    expect("&=p [ ?=1 ] &=q [ ?=2 ] q=() p=()", 0, b"21Z");
    expect("&=p [ ?=1 ] &=q [ p=() ] q=()", 0, b"1Z");
    expect(".VALUE:=42 &=p [ ?=.VALUE ] p=()", 0, b"42Z");
    expect("~=42 &=p [ ] p=~ ?=~", 0, b"42Z");
    expect("A=0 &=p [ A=A+1 ] p=() ?=A", 0, b"1Z");
    expect("&=p [ %=1 [ ?=7 ] ] p=()", 0, b"7Z");
    expect("&=p [ A=0 *=A<2 [ A=A+1 ] ?=A ] p=()", 0, b"2Z");
    expect("&=p [ #=3 ?=1 ^=3 ?=9 ] p=()", 0, b"9Z");
    expect("&=p [ ^=3 ?=9 ] ^=3 ?=8 p=()", 0, b"89Z");
    expect("?=1 &=p [ ?=8 ] ?=2 p=() ?=3", 0, b"1283Z");
    expect("&=p [ ?=1 ] ?=2 &=q [ ?=3 ] ?=4 q=() p=()", 0, b"2431Z");
    expect("&=p [ ?=1 #=() ; return\n ?=9 ] p=() ?=2", 0, b"12Z");
    expect(
        "&=p [ %=1 [ *=1 [ %=1 [ ?=3 #=() ] ] ] ?=9 ] p=() ?=4",
        0,
        b"34Z",
    );
    expect("&=p [ %=0 [ #=() ] ?=5 ] p=() ?=6", 0, b"56Z");
    expect(
        "~=11 &=p [ A=7 @=3,9 ?=\"x\" ~=42 ~=43 #=() A=99 @=3,8 ?=\"bad\" ~=99 ] p=() B=~ C=~ D=~ ?=B ?=C ?=D ?=A ?=@(3)",
        0,
        b"x43421179Z",
    );
    expect("#=() ?=9 &=p [ ] B=7 ?=B", 0, b"7Z");
    expect("#=()", 0, b"Z");
    expect("&=p [ ?=() ] p=()", 0, b"\nZ");
    expect("%=1 [ ?=() ]", 0, b"\nZ");
    expect("A=0 *=A<1 [ ?=() A=A+1 ]", 0, b"\nZ");
    expect("%=1 [ ] ?=7", 0, b"7Z");
    expect("%=1 [ ?=1 ]", 0, b"1Z");
    expect("%=0 [ ?=1 ]", 0, b"Z");
    expect("%=1 [ ?=1 ] [ ?=2 ]", 0, b"1Z");
    expect("%=0 [ ?=1 ] [ ?=2 ]", 0, b"2Z");
    expect("%=1 [ ?=1 ] ; else follows comment\n [ ?=2 ]", 0, b"1Z");
    expect("%=1 [ ] ?=3", 0, b"3Z");
    expect("%=1 [ %=0 [ ?=1 ] [ ?=2 ] ] [ ?=3 ]", 0, b"2Z");
    expect("%=0 [ %=1 [ ?=1 ] ] [ ?=2 ]", 0, b"2Z");
    expect("~=1 %=~ [ ?=7 ]", 0, b"7Z");
    expect("~=0 %=~ [ ?=1 ] [ ?=2 ]", 0, b"2Z");
    expect("~=1 %=~+1 [ ?=3 ]", 0, b"3Z");
    expect("~=1 ~=0 %=~ [ ?=1 ] [ ?=2 ] ?=~", 0, b"21Z");
    expect(
        "~=1 %=~ ; condition continues after comment\n [ ?=8 ]",
        0,
        b"8Z",
    );
    expect("A=0 *=0 [ A=99 ] ?=A", 0, b"0Z");
    expect("A=0 *=A<1 [ A=A+1 ] ?=A", 0, b"1Z");
    expect("A=0 *=A<4 [ A=A+1 ] ?=A", 0, b"4Z");
    expect("A=1 *=A [ A=0 ] ?=A", 0, b"0Z");
    expect("A=-2 *=A [ A=A+1 ] ?=A", 0, b"0Z");
    expect(".LIMIT:=3 A=0 *=(A<.LIMIT) [ A=A+1 ] ?=A", 0, b"3Z");
    expect("A=0 *=A<3*2 [ A=A+1 ] ?=A*4", 0, b"12Z");
    expect("~=1 *=~ [ A=1 ~=0 ] ?=A", 0, b"1Z");
    expect("~=2 *=~-1 [ ~=1 ] ?=A", 0, b"0Z");
    expect("A=0 *=A<3 [ A=A+1 ] A=A+5 ?=A", 0, b"8Z");
    // A jump from an inline While body resolves in the surrounding owner's
    // label namespace and reaches the label after the loop.
    expect("A=0 *=A<1 [ #=9 A=99 ] ^=9 ?=A", 0, b"0Z");
    expect("A=0 *=A<2 [ %=1 [ A=A+1 ] ] ?=A", 0, b"2Z");
    expect("A=0 %=1 [ *=A<2 [ A=A+1 ] ] ?=A", 0, b"2Z");
    expect("A=0 *=A<2 [ A=A+1 *=0 [ A=99 ] ] ?=A", 0, b"2Z");
    // Each pending patch must survive both kinds of nested frame, including
    // the replacement of an IF false patch by its else-end patch.
    expect(
        "A=0 %=1 [ *=A<2 [ %=A [ ?=8 ] [ ?=7 ] A=A+1 ] ] [ ?=9 ] ?=A",
        0,
        b"782Z",
    );
    expect(
        "A=0 *=A<2 [ %=A [ *=0 [ ?=9 ] ?=8 ] [ ?=7 ] A=A+1 ] ?=A",
        0,
        b"782Z",
    );
    expect(
        "%=0 [ ?=9 ] [ ?=1 ] *=0 [ ?=9 ] %=1 [ ?=2 ] [ ?=9 ] ?=3",
        0,
        b"123Z",
    );
    expect(
        "&=p [ A=0 %=1 [ *=A<2 [ %=A [ ?=8 ] [ ?=7 ] A=A+1 ] ] [ ?=9 ] ?=A ] p=()",
        0,
        b"782Z",
    );
    let mixed_16 = format!("{}?=9{}", "%=1 [ *=1 [ ".repeat(8), " ]".repeat(16));
    // The innermost statement is unreachable at runtime; all frames are
    // nevertheless compiled and patched. Avoid executing the infinite loops.
    let mixed_16 = mixed_16.replacen("*=1", "*=0", 1);
    expect(&mixed_16, 0, b"Z");
    expect(".VALUE:=42 %=1 [ ?=.VALUE ]", 0, b"42Z");
    expect("%=1 [ #=2 ?=1 ^=2 ?=8 ]", 0, b"8Z");
    let nested_16 = format!("{}?=9{}", "%=1 [ ".repeat(16), " ]".repeat(16));
    expect(&nested_16, 0, b"9Z");
    let nested_while_16 = format!("{}?=9{}", "*=0 [ ".repeat(16), " ]".repeat(16));
    expect(&nested_while_16, 0, b"Z");
    // Keep expression grouping at its independent limit while all sixteen
    // While frames are active during source compilation.
    let mut nested_while_grouping = "*=0 [ ".repeat(15);
    nested_while_grouping.push_str(&format!("*={}0{} [ ]", "(".repeat(16), ")".repeat(16)));
    nested_while_grouping.push_str(&" ]".repeat(15));
    expect(&nested_while_grouping, 0, b"Z");
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
    expect(".BOARD_BASE:=16 A=.BOARD_BASE+2 ?=A", 0, b"18Z");
    expect(".NEG_ONE:=-1 A=.NEG_ONE ?=A", 0, b"-1Z");
    // The first jump reaches END; the later backward jump is compiled and
    // patched but remains unreachable, keeping this fixture finite.
    expect(
        ".LOOP:=1 .END:=2 A=.LOOP #=.END ^=.LOOP #=.LOOP ^=.END ?=A",
        0,
        b"1Z",
    );
    expect("#=32767 ^=32767 #=0 ^=0 ?=7", 0, b"7Z");
    expect(
        ".SAME_ONE:=1 .SAME_TWO:=1 A=.SAME_ONE+.SAME_TWO ?=A",
        0,
        b"2Z",
    );
    expect(".ABCDEFGHIJKLMNOP:=32767 ?=.ABCDEFGHIJKLMNOP", 0, b"32767Z");
    expect(
        ".C0:=100 .C1:=101 .C2:=102 .C3:=103 .C4:=104 .C5:=105 .C6:=106 .C7:=107 .C8:=108 .C9:=109 .C10:=110 .C11:=111 .C12:=112 .C13:=113 .ABCDEFGHIJKLMNOP:=114 .C15:=115 ?=.C0 ?=.C7 ?=.C13 ?=.ABCDEFGHIJKLMNOP ?=.C15",
        0,
        b"100107113114115Z",
    );
    expect(".ZERO:=0 ?=2==2", 0, b"1Z");
    expect("A=1 ; a comment\n\tA=A+1 ?=A", 0, b"2Z");
    let long_source = format!("{}?=42", " ".repeat(254));
    expect(&long_source, 0, b"42Z");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn malformed_source_never_starts_runtime() {
    for source in [
        "#=",
        "#=!",
        "#=(1)",
        "#=()x",
        "^=()",
        "?=42 #=()x",
        "A=",
        "A=()",
        "?=( )",
        "?=()?=1",
        "?=(",
        "?=()+1",
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
        "?=7 A=",
        "?=1 ?=",
        "A=1+z",
        "A=.BEFORE\n.BEFORE:=1",
        ".DUP:=1 .DUP:=2",
        ".A:=1",
        ".aB:=1",
        ".AB$:=1",
        ".ABCDEFGHIJKLMNOPQ:=1",
        ".LOW:=-32769",
        ".HIGH:=32768",
        ".EXPR:=1+2",
        ".REF:=OTHER",
        "A=UNKNOWN",
        "&=",
        "&=P [ ]",
        "&=p[ ]",
        "&=p",
        "&=p [",
        "&=p []",
        "&=p [ ?=1",
        "&=p [ &=q [ ] ]",
        "?=7 &=p: [ ]",
        "?=7 p:()",
        "&=p [ %=1 [ &=q [ ] ] ]",
        "&=p [ ^=1 #=2 ]",
        "#=2 &=p [ ] ^=2",
        "?=7 &=p [ A= ]",
        "p=() &=p [ ]",
        "&=p [ ] &=p [ ]",
        "&=p [ p=() ]",
        "&=p [ q=() ] &=q [ ]",
        "p=1",
        "^=",
        "#=",
        "^=12x",
        "#=UNKNOWN",
        "%=1 A=2",
        "%=1 [ A=2",
        "%=1 [ ] [ A=",
        "%=1 [ ] [",
        "%= [ ]",
        "%=~",
        "%=1[ ?=1 ]",
        "%=1 []",
        "%=1 [?=1 ]",
        "%=1 [ ?=1 ][ ?=2 ]",
        "%=1+~ [ ?=1 ]",
        "%=(~) [ ?=1 ]",
        "%=1 [ ]?=3",
        "%=1 [ ?=1]",
        "*=",
        "*=1",
        "*=() [ ~=0 ] [ A=1 ]",
        "*=() [ ] [ ]",
        "*=1",
        "*=1 A=1",
        "*=1 [ A=1",
        "*=1 [ A= ]",
        "*=0[ ]",
        "*=0 [A=1 ]",
        "*=0 [ A=1]",
        "*=0 [ ] [ ]",
        "*=A+ [ ]",
        "[ ]",
        "]",
        "%=1 [ ] ]",
        ".NEG:=-1 ^=.NEG",
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
                || source == ".REF:=OTHER"
                || source == ".NEG:=-1 ^=.NEG"
                || source == "^=-1"
            {
                3
            } else if source == "&=p [ ^=1 #=2 ]"
                || source == "p=() &=p [ ]"
                || source == "&=p [ p=() ]"
                || source == "&=p [ q=() ] &=q [ ]"
                || source == "#=2 &=p [ ] ^=2"
                || source == "p=1"
            {
                4
            } else {
                2
            },
            b"",
        );
    }
    expect("%=~ [ ?=1 ]", 1, b"");
    let nested_17 = format!("{}?=9{}", "%=1 [ ".repeat(17), " ]".repeat(17));
    expect(&nested_17, 2, b"");
    let nested_while_17 = format!("{}?=9{}", "*=0 [ ".repeat(17), " ]".repeat(17));
    expect(&nested_while_17, 2, b"");
    let mixed_17 = format!("{}%=1 [ ?=9 ]{}", "%=1 [ *=0 [ ".repeat(8), " ]".repeat(16));
    expect(&mixed_17, 2, b"");
    expect("?=7 %=1 [ *=0 [ %=1 [ ] [", 2, b"");
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
    let source = format!("?=7 {}", "A=1 ".repeat(3000));
    expect(&source, 4, b"");
    let mut driver_arena_full = "A=1 ".repeat(2047);
    driver_arena_full.push_str("?=7 #=1 ^=1 #=2 ^=2");
    expect(&driver_arena_full, 4, b"");
    let source = format!("{}A=~ A=~", "A=1 ".repeat(2048));
    expect(&source, 4, b"");
    let truncated = build_and_run_input(&[4, 0, b'?', b'=', b'1']);
    assert_eq!(truncated.status.code(), Some(1), "{truncated:?}");
    assert!(truncated.stdout.is_empty(), "{truncated:?}");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn primary_read_waits_discards_nul_and_maps_ff_and_eof() {
    let output = build_and_run_read_char(
        b"A=$ B=$ C=$ D=$ E=$ ?=A ?=B ?=C ?=D ?=E",
        &[0, 0x7f, 0x80, 0xff],
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"127128255-1-1", "{output:?}");

    let output = build_and_run_read_char(b"A=$ ?=A", &[0, 0, b'A']);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"65", "{output:?}");

    let output = build_and_run_read_char(b"A=$ ?=A", &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"-1", "{output:?}");

    let output = build_and_run_read_char(b"A=$ ?=A", &[0, 0]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"-1", "{output:?}");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn primary_read_works_in_expressions_and_preserves_character_write() {
    let output = build_and_run_read_char(b"?=$ ?=$", &[65, 66]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"6566", "{output:?}");

    let output = build_and_run_read_char(b"A=1+$ B=($) ?=A ?=B $=A $=0", &[41, 42]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"4242*\0", "{output:?}");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn primary_read_compile_failure_does_not_start_runtime() {
    let output = build_and_run_read_char(b"A=$+", b"Z");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert_eq!(output.stdout, b"", "{output:?}");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn removed_template_forms_fail_before_runtime() {
    for form in removed_templates::FORMS {
        expect(&format!("?=9 {form} ?=99"), 2, b"");
    }
    expect("&=a [ ?=7 ] a=()", 0, b"7Z");
}

fn stack_fixture() -> String {
    format!(
        "%=0 [ {}A={}0+0{}{} ?=A",
        "%=1 [ ".repeat(15),
        "0+(".repeat(16),
        ")".repeat(16),
        " ]".repeat(16)
    )
}

// Instrument only a temporary assembly copy. No test switches or symbols are
// added to the production ABI, and no probe pushes change the observed S.
fn stack_probe_basic() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let basic = fs::read_to_string(root.join("6502/frontend/basic.s")).unwrap();
    let basic = basic.replace(".segment \"BSS\"", ".segment \"BSS\"\naudit_min: .res 1");
    let basic = basic.replace(
        "fe_compile_run:\n",
        "fe_compile_run:\n    lda #$a5\n    ldx #24\n:\n    sta $0100,x\n    dex\n    bpl :-\n    lda #255\n    sta audit_min\n",
    );
    let basic = basic.replace(
        "    tsx\n    cpx #25",
        "    tsx\n    cpx audit_min\n    bcs :+\n    stx audit_min\n:\n    cpx #25",
    );
    let fatal = "compile_stack_exhausted:\n    lda #4\n    sta fe_compile_status\n";
    assert_eq!(basic.matches(fatal).count(), 1);
    basic.replace(
        fatal,
        &format!("{fatal}    lda audit_min\n    jeq audit_failed\n    ldx #24\n:\n    lda $0100,x\n    cmp #$a5\n    jne audit_failed\n    dex\n    bpl :-\n"),
    ) + "\n.segment \"CODE\"\naudit_failed:\n    lda #99\n    jmp halt\n"
}

fn run_stack_probe(source: &str, basic: &str) -> Output {
    let mut input = (source.len() as u16).to_le_bytes().to_vec();
    input.extend_from_slice(source.as_bytes());
    build_and_run_custom(&input, "6502/target/sim65_cli.s", Some(basic), None)
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn compile_stack_guard_comparison_boundary_preserves_operand_modes() {
    for observed in [24, 25] {
        // Change the comparison input, never TXS or the live return frames.
        // Both the value-first and stack-root operand entry branches execute.
        let basic = stack_probe_basic()
            .replace("    cpx #25", &format!("    ldx #{observed}\n    cpx #25"));
        let result = run_stack_probe("~=40 ?=~+2", &basic);
        assert_eq!(
            result.status.code(),
            Some(if observed == 24 { 132 } else { 0 }),
            "{result:?}"
        );
        assert_eq!(
            result.stdout,
            if observed == 24 {
                b"".as_slice()
            } else {
                b"42".as_slice()
            }
        );
    }
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn compile_stack_guard_halts_deep_sources_before_runtime_or_wrap() {
    let cases = [
        (
            "statement",
            format!("?=9 {}A=1{}", "%=1 [ ".repeat(16), " ]".repeat(16)),
        ),
        (
            "required_block",
            format!("?=9 {}A=1{}", "%=1 [ ".repeat(16), " ]".repeat(16)),
        ),
        (
            "operand",
            format!("?=9 %=0 [ A={}0{} ]", "0+(".repeat(16), ")".repeat(16)),
        ),
        (
            "value",
            format!("?=9 %=0 [ A={}0{} ]", "0+(".repeat(16), ")".repeat(16)),
        ),
    ];
    for (entry, source) in cases {
        let mut basic = stack_probe_basic();
        let baseline = run_stack_probe(&source, &basic);
        assert_eq!(baseline.status.code(), Some(0), "{baseline:?}");
        assert_eq!(baseline.stdout, b"9");
        let check = "    tsx\n    cpx #200\n    bcs :+\n    jmp compile_stack_exhausted\n:\n";
        // Operand's first BNE must consume the incoming Z before any probe.
        let marker = if entry == "operand" {
            "operand:\n    bne @operators\n    compile_stack_guard\n".to_owned()
        } else {
            format!("{entry}:\n    compile_stack_guard\n")
        };
        assert_eq!(basic.matches(&marker).count(), 1);
        basic = basic.replace(&marker, &format!("{marker}{check}"));
        basic = basic.replace("    jsr rt_init", "    jmp audit_failed");
        let result = run_stack_probe(&source, &basic);
        assert_eq!(result.status.code(), Some(132), "{entry}: {result:?}");
        assert!(result.stdout.is_empty(), "{result:?}");
    }
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn compile_stack_guard_accepts_201_byte_fixture_and_observes_headroom() {
    let source = stack_fixture();
    assert_eq!(source.len(), 201);
    let basic = stack_probe_basic().replace(
        "@returned:\n",
        "@returned:\n    lda audit_min\n    jmp halt\n",
    );
    let result = run_stack_probe(&source, &basic);
    let minimum = result.status.code().unwrap();
    assert!((25..132).contains(&minimum), "{result:?}");
    assert_eq!(result.stdout, b"0");
    eprintln!("201-byte fixture: minimum guard S={minimum}");
}
