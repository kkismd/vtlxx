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
    build_and_run_custom(input, frontend_fixture, None)
}

fn build_and_run_custom(input: &[u8], frontend_fixture: &str, basic: Option<&str>) -> Output {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = [
        frontend_fixture,
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
    expect("==VALUE,42 &=p [ ?=VALUE ] p=()", 0, b"42Z");
    expect("~=42 &=p [ ] p=~ ?=~", 0, b"42Z");
    expect("A=0 &=p [ A=A+1 ] p=() ?=A", 0, b"1Z");
    expect("&=p [ %=1 [ ?=7 ] ] p=()", 0, b"7Z");
    expect("&=p [ A=0 *=A<2 [ A=A+1 ] ?=A ] p=()", 0, b"2Z");
    expect("&=p [ #=3 ?=1 ^=3 ?=9 ] p=()", 0, b"9Z");
    expect("&=p [ ^=3 ?=9 ] ^=3 ?=8 p=()", 0, b"89Z");
    expect("?=1 &=p [ ?=8 ] ?=2 p=() ?=3", 0, b"1283Z");
    expect("&=p [ ?=1 ] ?=2 &=q [ ?=3 ] ?=4 q=() p=()", 0, b"2431Z");
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
    expect("==LIMIT,3 A=0 *=(A<LIMIT) [ A=A+1 ] ?=A", 0, b"3Z");
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
    expect("==VALUE,42 %=1 [ ?=VALUE ]", 0, b"42Z");
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
    expect("%=~ [ ?=1 ]", 1, b"");
    let nested_17 = format!("{}?=9{}", "%=1 [ ".repeat(17), " ]".repeat(17));
    expect(&nested_17, 2, b"");
    let nested_while_17 = format!("{}?=9{}", "*=0 [ ".repeat(17), " ]".repeat(17));
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
fn template_definitions_are_saved_and_published_without_runtime_code() {
    expect("&=a{} [ {}={}+1 ]", 0, b"Z");
    expect("&=a{} [ A=MAX{} ; ignored ]\n]", 0, b"Z");
    expect("&=a{} [ A=MAX{}\n{}=A+ ]", 0, b"Z");
    expect("&=a{} [ A={} B= ]", 0, b"Z");
    expect("&=a{} [ A={} ] &=b{} [ {}=A{} ]", 0, b"Z");
    expect("&=p [ ?=7 ] &=a{} [ A={} ] p=()", 0, b"7Z");
    expect("&=a{} [ A={} ] &=p [ ?=7 ] p=()", 0, b"7Z");

    let many_holes = format!("&=a{{}} [ A={} ]", "{}".repeat(256));
    expect(&many_holes, 0, b"Z");
    let many_statements = format!("&=a{{}} [ {}]", "A={} ".repeat(256));
    expect(&many_statements, 0, b"Z");

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

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn template_calls_expand_into_the_current_caller() {
    expect(
        "&=a{} [ {}={}+1 ] A=0 B=10 a{A}=() a{B}=() ?=A ?=B",
        0,
        b"111Z",
    );
    expect("&=a{} [ A=MAX{} ] a{A}=()", 2, b"");
    expect("==MAXA,3 &=a{} [ A=MAX{} ] a{A}=() ?=A", 0, b"3Z");
    expect("&=a{} [ A={} ] a{C}=() ?=C", 0, b"0Z");
    expect("&=a{} [ {}={} ] a{A}=()", 0, b"Z");
    expect("&=a{} [ {}={}+1 ] A=0 &=p [ a{A}=() ?=A ] p=()", 0, b"1Z");
    expect("&=a{} [ {}={}+1 ] A=0 %=1 [ a{A}=() ] ?=A", 0, b"1Z");
    expect("&=a{} [ {}={}+1 ] A=0 *=A<1 [ a{A}=() ] ?=A", 0, b"1Z");
    expect("&=a{} [ {}={} ] a{A}=() ; tail\n?=A", 0, b"0Z");
    expect("&=a{} [ {}=A+ ] a{B}=()", 2, b"");
    for source in [
        "a{a}=()",
        "a{}=()",
        "a{AA}=()",
        "a{A}=1",
        "a{A}=(1)",
        "a{A}=()?=1",
    ] {
        expect(source, 2, b"");
    }
    let mut long_body = String::from("A=0 &=a{} [ ");
    long_body.push_str(&"A={} ".repeat(60));
    long_body.push_str("] a{A}=() ?=A");
    expect(&long_body, 0, b"0Z");

    let mut all_identities = String::new();
    for identity in b'a'..=b'z' {
        let identity = char::from(identity);
        all_identities.push_str(&format!(
            "&={identity}{{}} [ {{}}={{}} ] {identity}{{A}}=() "
        ));
    }
    expect(&all_identities, 0, b"Z");

    let mut capacity_failure = String::from("&=a{} [ ");
    capacity_failure.push_str(&"A={} ".repeat(300));
    capacity_failure.push_str("] a{A}=()");
    expect(&capacity_failure, 4, b"");
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
    build_and_run_custom(&input, "6502/target/sim65_cli.s", Some(basic))
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
            false,
        ),
        (
            "required_block",
            format!("?=9 {}A=1{}", "%=1 [ ".repeat(16), " ]".repeat(16)),
            false,
        ),
        (
            "operand",
            format!("?=9 %=0 [ A={}0{} ]", "0+(".repeat(16), ")".repeat(16)),
            false,
        ),
        (
            "value",
            format!("?=9 %=0 [ A={}0{} ]", "0+(".repeat(16), ")".repeat(16)),
            false,
        ),
        (
            "value",
            format!(
                "&=a{{}} [ {{}}={}0{} ] ?=9 %=0 [ a{{A}}=() ]",
                "0+(".repeat(16),
                ")".repeat(16)
            ),
            true,
        ),
    ];
    for (entry, source, virtual_source) in cases {
        let mut basic = stack_probe_basic();
        let baseline = run_stack_probe(&source, &basic);
        assert_eq!(baseline.status.code(), Some(0), "{baseline:?}");
        assert_eq!(baseline.stdout, b"9");
        // Only the selected entry gets a stricter threshold. For P0a, ensure
        // that the failure actually occurs while reading the expansion.
        let check = if virtual_source {
            "    lda virtual_active\n    beq :+\n    tsx\n    cpx #200\n    bcs :+\n    jmp compile_stack_exhausted\n:\n"
        } else {
            "    tsx\n    cpx #200\n    bcs :+\n    jmp compile_stack_exhausted\n:\n"
        };
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
        assert_eq!(
            result.status.code(),
            Some(132),
            "{entry}, virtual={virtual_source}: {result:?}"
        );
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

// #427 expectation/observation ledger: fixtures/template_p0a/README.md.
fn p0a_source(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/template_p0a")
            .join(format!("{name}.vtl")),
    )
    .unwrap()
}

fn p0a_frame(source: &str) -> Vec<u8> {
    let mut input = u16::try_from(source.len()).unwrap().to_le_bytes().to_vec();
    input.extend_from_slice(source.as_bytes());
    input
}

fn p0a_result(result: &Output, status: i32, stdout: &[u8]) {
    assert_eq!(result.status.code(), Some(status), "{result:?}");
    assert_eq!(result.stdout, stdout, "{result:?}");
    assert!(result.stderr.is_empty(), "{result:?}");
}

// This private adapter checks the live registry before process exit, never by
// compiling another source after failure. Only 'a' may be published in these
// fixtures; compare its entire canonical body, and reject every other slot.
fn p0a_probe(source: &str, body: Option<&str>, failure: Option<bool>) -> Output {
    p0a_probe_status(source, body, failure.map(|reader| (reader, 2)))
}

fn p0a_probe_status(source: &str, body: Option<&str>, failure: Option<(bool, u8)>) -> Output {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut basic = fs::read_to_string(root.join("6502/frontend/basic.s")).unwrap();
    let mut checks = String::new();
    if let Some((virtual_rhs, status)) = failure {
        // Record the input reader at the actual syntax error, before cleanup
        // clears virtual_active. Runtime entry is a trap, not an output proxy.
        let marker = if status == 4 {
            "backend_error:\n"
        } else {
            "syntax_error:\n"
        };
        assert_eq!(basic.matches(marker).count(), 1);
        basic = basic.replace(
            marker,
            &format!("{marker}    lda virtual_active\n    sta p0a_error_reader\n"),
        );
        let marker = "    jsr rt_init";
        assert_eq!(basic.matches(marker).count(), 1);
        basic = basic.replace(marker, "    lda #99\n    jmp halt");
        basic.push_str("\n.export p0a_error_reader\n.segment \"BSS\"\np0a_error_reader: .res 1\n");
        checks.push_str(&format!(
            ".import p0a_error_reader\n    jcc failed\n    lda fe_compile_status\n    cmp #{status}\n    jne failed\n    lda p0a_error_reader\n    cmp #{}\n    jne failed\n", u8::from(virtual_rhs)
        ));
    } else {
        checks.push_str("    jcs failed\n    lda rt_depth\n    jne failed\n");
    }
    for identity in b'a'..=b'z' {
        checks.push_str(&format!("    lda #{identity}\n    jsr template_lookup\n"));
        if let Some(body) = body.filter(|_| identity == b'a') {
            checks.push_str(&format!("    jcs failed\n    lda cc_data_length\n    cmp #{}\n    jne failed\n    lda cc_data_length+1\n    jne failed\n", body.len()));
            for (index, byte) in body.bytes().enumerate() {
                checks.push_str(&format!(
                    "    ldy #{index}\n    lda (cc_arg),y\n    cmp #{byte}\n    jne failed\n"
                ));
            }
        } else {
            checks.push_str("    jcc failed\n");
        }
    }
    let adapter = format!(
        ".setcpu \"6502\"\n.macpack longbranch\n.export _main\n.import fe_compile_run, template_lookup, halt\n.importzp fe_compile_status, cc_arg, cc_data_length, rt_depth\n.segment \"CODE\"\n_main:\n    jsr fe_compile_run\n{checks}    lda #0\n    jmp halt\nfailed:\n    lda #98\n    jmp halt\n"
    );
    let temp = TempDir::new();
    let path = temp.0.join("p0a_probe.s");
    fs::write(&path, adapter).unwrap();
    build_and_run_custom(&p0a_frame(source), path.to_str().unwrap(), Some(&basic))
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn p0a_shared_success_sources_match_reference_and_empty_stack() {
    for (name, stdout) in [
        ("minimal_two_arguments", b"1\n11\n".as_slice()),
        ("block_then_else", b"1\n7\n0\n4\n".as_slice()),
    ] {
        let source = p0a_source(name);
        let reference = Command::new(env!("CARGO_BIN_EXE_ecvtl"))
            .arg(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("tests/fixtures/template_p0a/{name}.vtl")),
            )
            .output()
            .unwrap();
        p0a_result(&reference, 0, stdout);
        // Unmodified production CLI adapter: no appended runtime sentinel.
        let result =
            build_and_run_input_with_frontend(&p0a_frame(&source), "6502/target/sim65_cli.s");
        p0a_result(&result, 0, &reference.stdout);
        p0a_result(&p0a_probe(&source, Some("{}={}+1"), None), 0, stdout);
    }
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn p0a_shared_failures_preserve_phase_publication_and_never_start_runtime() {
    let definition = p0a_source("deferred_rhs_definition");
    p0a_result(
        &build_and_run_input_with_frontend(&p0a_frame(&definition), "6502/target/sim65_cli.s"),
        0,
        b"",
    );
    p0a_result(&p0a_probe(&definition, Some("{}=A+"), None), 0, b"");

    let failures = [
        (
            format!("{definition}{}", p0a_source("deferred_rhs_call")),
            Some("{}=A+"),
            true,
        ),
        (
            p0a_source("partial_expansion_failure"),
            Some("A=3\n{}=A+"),
            true,
        ),
        (p0a_source("chunk_failure_profiles"), Some("A={}"), false),
    ];
    for (source, body, virtual_rhs) in failures {
        // CLI status 128 + syntax(2), distinct from source(129) and resource(132).
        p0a_result(
            &build_and_run_input_with_frontend(&p0a_frame(&source), "6502/target/sim65_cli.s"),
            130,
            b"",
        );
        p0a_result(&p0a_probe(&source, body, Some(virtual_rhs)), 0, b"");
    }
    for (name, count) in [("invalid_definitions", 7), ("invalid_call_forms", 5)] {
        let fixture = p0a_source(name);
        assert_eq!(
            fixture.lines().count(),
            count,
            "{name}: update the expectation ledger when rows change"
        );
        for (line, statement) in fixture.lines().enumerate() {
            assert!(!statement.trim().is_empty());
            if name == "invalid_call_forms" && statement == "a{A}=1" {
                // The P0a source is retained for history; P0b accepts this row.
                let source = format!("&=a{{}} [ A={{}} ]\n{statement} ?=~");
                p0a_result(
                    &build_and_run_input_with_frontend(
                        &p0a_frame(&source),
                        "6502/target/sim65_cli.s",
                    ),
                    0,
                    b"1",
                );
                continue;
            }
            let (source, body) = if name == "invalid_call_forms" {
                (format!("&=a{{}} [ A={{}} ]\n{statement}"), Some("A={}"))
            } else {
                (statement.to_owned(), None)
            };
            eprintln!("{name}:{}: syntax failure before virtual RHS", line + 1);
            p0a_result(
                &build_and_run_input_with_frontend(&p0a_frame(&source), "6502/target/sim65_cli.s"),
                130,
                b"",
            );
            p0a_result(&p0a_probe(&source, body, Some(false)), 0, b"");
        }
    }
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn p0b_shared_fixture_and_rhs_forms_match_reference() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/template_p0b/minimal.vtl");
    let source = fs::read_to_string(&path).unwrap();
    let reference = Command::new(env!("CARGO_BIN_EXE_ecvtl"))
        .arg(path)
        .output()
        .unwrap();
    p0a_result(&reference, 0, b"15\n");
    p0a_result(
        &build_and_run_input_with_frontend(&p0a_frame(&source), "6502/target/sim65_cli.s"),
        0,
        &reference.stdout,
    );
    p0a_result(&p0a_probe(&source, Some("{}=~+{}"), None), 0, b"15\n");

    for (source, stdout) in [
        (
            "==NUM,3 &=a{} [ {}=~+{} ] B=2 a{B}=(B+NUM) a{B}=1,4 ?=B ?=~",
            b"111".as_slice(),
        ),
        ("&=a{} [ {}=~+{} ] ~=4 A=1 a{A}=~+2 ?=A", b"7".as_slice()),
        (
            "&=a{} [ {}=~+{} ] &=p [ a{A}=2 ] A=0 %=1 [ a{A}=3 ] [ a{A}=99 ] *=A<7 [ a{A}=1 ] p=() ?=A",
            b"9".as_slice(),
        ),
    ] {
        p0a_result(
            &build_and_run_input_with_frontend(&p0a_frame(source), "6502/target/sim65_cli.s"),
            0,
            stdout,
        );
    }
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn p0b_failures_keep_published_template_and_do_not_start_runtime() {
    let fixture = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/template_p0b/invalid_rhs.vtl"),
    )
    .unwrap();
    for (source, body, virtual_rhs) in [
        (fixture, "{}=~", false),
        ("&=a{} [ {}=~ ] a{A}=".to_owned(), "{}=~", false),
        ("&=a{} [ {}=~ ] a{A}=q".to_owned(), "{}=~", false),
        ("&=a{} [ {}=A+ ] a{A}=1".to_owned(), "{}=A+", true),
        ("&=a{} [ {}=~ ] z{A}=1".to_owned(), "{}=~", false),
        ("&=a{} [ {}=~ ] a{AB}=1".to_owned(), "{}=~", false),
    ] {
        p0a_result(
            &build_and_run_input_with_frontend(&p0a_frame(&source), "6502/target/sim65_cli.s"),
            130,
            b"",
        );
        p0a_result(&p0a_probe(&source, Some(body), Some(virtual_rhs)), 0, b"");
    }
    let source = format!("&=a{{}} [ {{}}=~ ] a{{A}}={}", vec!["1"; 500].join(","));
    p0a_result(
        &build_and_run_input_with_frontend(&p0a_frame(&source), "6502/target/sim65_cli.s"),
        132,
        b"",
    );
    p0a_result(
        &p0a_probe_status(&source, Some("{}=~"), Some((false, 4))),
        0,
        b"",
    );
}
