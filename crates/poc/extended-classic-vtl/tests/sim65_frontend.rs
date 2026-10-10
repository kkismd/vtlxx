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
    build_and_run_input_with_frontend(input, "tests/fixtures/sim65/frontend_basic.s")
}

fn build_and_run_input_with_frontend(input: &[u8], frontend_fixture: &str) -> Output {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = [
        frontend_fixture,
        "6502/frontend/basic.s",
        "6502/frontend/template.s",
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
    expect("?=1 ?=2 A=3 ?=A ?=4", 0, b"1234Z");
    expect("&=p [ ] p=()", 0, b"Z");
    expect("&=p [ ?=~ ] p=42", 0, b"42Z");
    expect("&=p [ ?=~ ] p=(40+2)", 0, b"42Z");
    expect("&=p [ ?=~ ?=~ ] p=10,42", 0, b"4210Z");
    expect("&=p [ ] &=q [ ] p=() q=()", 0, b"Z");
    expect("&=p [ ?=1 ] &=q [ ?=2 ] q=() p=()", 0, b"21Z");
    expect("&=p [ ?=1 ] &=q [ p=() ] q=()", 0, b"1Z");
    expect("==VALUE,42 &=p [ ?=VALUE ] p=()", 0, b"42Z");
    expect("~=42 &=p [ ] p=~ ?=~", 0, b"42Z");
    expect("A=0 &=p [ A=A+1 ] p=() ?=A", 0, b"1Z");
    expect("&=p [ %=1 [ ?=7 ] ] p=()", 0, b"7Z");
    expect("&=p [ A=0 *=() [ ~=A<2 ] [ A=A+1 ] ?=A ] p=()", 0, b"2Z");
    expect("&=p [ #=3 ?=1 ^=3 ?=9 ] p=()", 0, b"9Z");
    expect("&=p [ ^=3 ?=9 ] ^=3 ?=8 p=()", 0, b"89Z");
    expect("?=1 &=p [ ?=8 ] ?=2 p=() ?=3", 0, b"1283Z");
    expect("&=p [ ?=1 ] ?=2 &=q [ ?=3 ] ?=4 q=() p=()", 0, b"2431Z");
    expect("%=1 [ ] ?=7", 0, b"7Z");
    expect("%=1 [ ?=1 ]", 0, b"1Z");
    expect("%=0 [ ?=1 ]", 0, b"Z");
    expect("%=1 [ ?=1 ] [ ?=2 ]", 0, b"1Z");
    expect("%=0 [ ?=1 ] [ ?=2 ]", 0, b"2Z");
    expect("%=1 [ ?=1 ] ; else follows comment\n [ ?=2 ]", 0, b"1Z");
    expect("%=1 [ ] ?=3", 0, b"3Z");
    expect("%=1 [ %=0 [ ?=1 ] [ ?=2 ] ] [ ?=3 ]", 0, b"2Z");
    expect("%=0 [ %=1 [ ?=1 ] ] [ ?=2 ]", 0, b"2Z");
    expect("A=0 *=() [ ~=0 ] [ A=99 ] ?=A", 0, b"0Z");
    expect("A=0 *=() [ ~=A<1 ] [ A=A+1 ] ?=A", 0, b"1Z");
    expect("A=0 *=() [ ~=A<4 ] [ A=A+1 ] ?=A", 0, b"4Z");
    expect("A=0 *=() [ ~=A<3 ] [ A=A+1 ] A=A+5 ?=A", 0, b"8Z");
    // A jump from an inline While body resolves in the surrounding owner's
    // label namespace and reaches the label after the loop.
    expect("A=0 *=() [ ~=A<1 ] [ #=9 A=99 ] ^=9 ?=A", 0, b"0Z");
    expect("==LIMIT,3 A=0 *=() [ ~=A<LIMIT ] [ A=A+1 ] ?=A", 0, b"3Z");
    expect("A=0 *=() [ ~=A<2 ] [ %=1 [ A=A+1 ] ] ?=A", 0, b"2Z");
    expect("A=0 %=1 [ *=() [ ~=A<2 ] [ A=A+1 ] ] ?=A", 0, b"2Z");
    expect(
        "A=0 *=() [ ~=A<2 ] [ A=A+1 *=() [ ~=0 ] [ A=99 ] ] ?=A",
        0,
        b"2Z",
    );
    expect("==VALUE,42 %=1 [ ?=VALUE ]", 0, b"42Z");
    expect("%=1 [ #=2 ?=1 ^=2 ?=8 ]", 0, b"8Z");
    let nested_16 = format!("{}?=9{}", "%=1 [ ".repeat(16), " ]".repeat(16));
    expect(&nested_16, 0, b"9Z");
    let nested_while_16 = format!("~=0 {}?=9{}", "*=() [ ~=0 ] [ ".repeat(16), " ]".repeat(16));
    expect(&nested_while_16, 0, b"Z");
    // Keep expression grouping at its independent limit while all sixteen
    // While frames are active during source compilation.
    let mut nested_while_grouping = "*=() [ ~=0 ] [ ".repeat(15);
    nested_while_grouping.push_str(&format!(
        "*=() [ ~={}0{} ] [ ]",
        "(".repeat(16),
        ")".repeat(16)
    ));
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
        "?=7 A=",
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
        "&=",
        "&=P [ ]",
        "&=pq [ ]",
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
        "%=1[ ?=1 ]",
        "%=1 []",
        "%=1 [?=1 ]",
        "%=1 [ ?=1 ][ ?=2 ]",
        "%=1 [ ]?=3",
        "%=1 [ ?=1]",
        "*=",
        "*=1",
        "*= ( ) [ ] [ ]",
        "*=( ) [ ] [ ]",
        "*=()[] [ ]",
        "*=() [ ]",
        "*=() [ ] [",
        "*=() [",
        "*=() [ ] [ A= ]",
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
    let nested_17 = format!("{}?=9{}", "%=1 [ ".repeat(17), " ]".repeat(17));
    expect(&nested_17, 2, b"");
    let nested_while_17 = format!("~=0 {}?=9{}", "*=() [ ~=0 ] [ ".repeat(17), " ]".repeat(17));
    expect(&nested_while_17, 2, b"");
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
    let source = format!("?=7 {}", "A=1 ".repeat(300));
    expect(&source, 4, b"");
    let mut driver_arena_full = "A=1 ".repeat(169);
    driver_arena_full.push_str("?=7 #=1 ^=1 #=2 ^=2");
    expect(&driver_arena_full, 4, b"");
    let source = format!("{}A=~ A=~", "A=1 ".repeat(170));
    expect(&source, 4, b"");
    let truncated = build_and_run_input(&[4, 0, b'?', b'=', b'1']);
    assert_eq!(truncated.status.code(), Some(1), "{truncated:?}");
    assert!(truncated.stdout.is_empty(), "{truncated:?}");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn template_definitions_are_saved_and_published_without_runtime_code() {
    expect("&=a{} [ {}={}+1 ]", 0, b"Z");
    expect("&=a{} [ A=MAX{} ; ignored ]\n]", 0, b"Z");
    expect("&=a{} [ A=MAX{}\n{}=A+ ]", 0, b"Z");
    expect("&=a{} [ A={} B= ]", 0, b"Z");
    expect("&=a{} [ A={} ] &=b{} [ {}=A{} ]", 0, b"Z");
    expect("&=p [ ?=7 ] &=a{} [ A={} ] p=()", 0, b"7Z");
    expect("&=a{} [ A={} ] &=p [ ?=7 ] p=()", 0, b"7Z");

    for source in [
        "&=a{} [ ]",
        "&=a{} [ A=1 ]",
        "&=a{}[ A={} ]",
        "&=a{ } [ A={} ]",
        "&=a{} [ A={X ]",
        "&=a{} [ [ A={} ] ]",
        "&=a{} [ ?=1 ]",
        "&=a{} [ @=1,{} ]",
        "&=a{} [ a={} ]",
        "&=a{} [ A={} &= ]",
        "&=a{} [ A={} ] &=a{} [ B={} ]",
        "&=a{} [ A={} ] &=p [ ] &=p{} [ B={} ]",
        "&=p [ ] &=a{} [ A={} ] &=p [ ]",
        "&=a{} [ A={} ] &=b{} [ B=1 ]",
        "&=A{} [ A={} ]",
        "&=ab{} [ A={} ]",
        "&=é{} [ A={} ]",
        "&=a{} [ A={} ]?=1",
    ] {
        expect(source, 2, b"");
    }
    for source in include_str!("fixtures/template_p0a/invalid_definitions.vtl")
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        // Tracker #354 requires each shared invalid fixture line to be a
        // separate length-prefixed WholeProgramCompileRun input.
        expect(source, 2, b"");
    }

    let mut too_large = String::from("&=a{} [ A={}");
    too_large.push_str(&"X".repeat(2100));
    too_large.push_str(" ]");
    expect(&too_large, 4, b"");

    let mut source = b"&=a{} [ A={} ]".to_vec();
    let framed_len = (source.len() + 2) as u16;
    let mut framed = vec![framed_len as u8, (framed_len >> 8) as u8];
    framed.append(&mut source);
    let truncated = build_and_run_input(&framed);
    assert_eq!(truncated.status.code(), Some(1), "{truncated:?}");
    assert!(truncated.stdout.is_empty(), "{truncated:?}");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn template_registry_points_to_canonical_saved_source() {
    let source = b"&=a{} [ A={} ; discarded ]\n{}=MAX{} ]";
    let mut framed = vec![source.len() as u8, (source.len() >> 8) as u8];
    framed.extend_from_slice(source);
    framed.push(b'Z');
    let result =
        build_and_run_input_with_frontend(&framed, "tests/fixtures/sim65/template_probe.s");
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    assert_eq!(result.stdout, b"OKZ", "{result:?}");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn failed_duplicate_template_does_not_replace_published_source() {
    let source = b"&=a{} [ A={} ] &=a{} [ B={} ]";
    let mut framed = vec![source.len() as u8, (source.len() >> 8) as u8];
    framed.extend_from_slice(source);
    framed.push(b'Z');
    let result =
        build_and_run_input_with_frontend(&framed, "tests/fixtures/sim65/template_failure_probe.s");
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    assert_eq!(result.stdout, b"P", "{result:?}");
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn template_registry_keeps_u16_source_length() {
    let mut source = String::from("&=a{} [ A={}");
    source.push_str(&"X".repeat(300));
    source.push_str(" ]");
    let source = source.into_bytes();
    let mut framed = vec![source.len() as u8, (source.len() >> 8) as u8];
    framed.extend_from_slice(&source);
    let result =
        build_and_run_input_with_frontend(&framed, "tests/fixtures/sim65/template_length_probe.s");
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    assert_eq!(result.stdout, b"L", "{result:?}");
}
