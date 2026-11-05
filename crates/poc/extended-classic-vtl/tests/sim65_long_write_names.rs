use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct SourceFile(PathBuf);

impl SourceFile {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "vtlxx-long-name-{}-{}.vtl",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, source).unwrap();
        Self(path)
    }
}

impl Drop for SourceFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn run(source: &str) -> Output {
    let source = SourceFile::new(source);
    Command::new(env!("CARGO_BIN_EXE_ecvtl6502"))
        .arg(&source.0)
        .args(["--cycle-limit", "200000000"])
        .output()
        .unwrap()
}

fn success(source: &str, expected: &[u8]) {
    let output = run(source);
    assert!(output.status.success(), "{source}: {output:?}");
    assert_eq!(output.stdout, expected, "{source}");
}

fn failure(source: &str) {
    let output = run(&format!("?=42 {source}"));
    assert!(!output.status.success(), "{source}: {output:?}");
    assert!(output.stdout.is_empty(), "{source}: {output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("compilation failed"),
        "{source}: {output:?}"
    );
}

fn name(n: usize) -> String {
    format!(
        "p{}{}",
        (b'a' + (n / 26) as u8) as char,
        (b'a' + (n % 26) as u8) as char
    )
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn names_bind_completely_and_share_the_stack() {
    let source = "&=a [ ?=\"a\" ] &=ab [ ?=\"b\" ] &=read [ A=~ ] &=reads [ read=~ ] &=reader [ reads=~+1 ] &=eightchr [ %=1 [ reader=~ ] ] a=() ab=() read=2+3 eightchr=7 ?=A";
    let mut reference = vtlxx_poc_extended_classic_vtl::Machine::new();
    reference.execute_source(source).unwrap();
    assert_eq!(reference.output(), b"ab8");
    success(source, reference.output());
    success(
        "&=read [ ?=1 ] &=road [ ?=2 ] &=reader [ ?=3 ] read=() road=() reader=()",
        b"123",
    );
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn invalid_names_and_definitions_do_not_start_runtime() {
    for source in [
        "&= [ ]",
        "&=abcdefghi [ ]",
        "&=abC [ ]",
        "&=ab1 [ ]",
        "&=ab_cd [ ]",
        "abcdefghi=()",
        "abC=()",
        "ab1=()",
        "ab_cd=()",
        "missing=()",
        "later=() &=later [ ]",
        "&=open [ A=1",
        "&=self [ self=() ]",
        "&=same [ ] &=same [ ]",
        "&=ab [ ] ab{A}=()",
        "&=a{} [ {}=1 ] &=a [ ]",
        "&=a [ ] &=a{} [ {}=1 ]",
    ] {
        failure(source);
    }
}

#[test]
#[ignore = "requires ca65, ld65, and sim65; run with --ignored"]
fn sixty_four_definitions_and_deep_calls_succeed_but_sixty_five_fail() {
    let mut definitions = String::new();
    for i in 0..64 {
        let body = if i == 0 {
            "?=7".to_owned()
        } else {
            format!("{}=()", name(i - 1))
        };
        definitions.push_str(&format!("&={} [ {body} ] ", name(i)));
    }
    success(&format!("{definitions} {}=()", name(63)), b"7");
    failure(&format!("{definitions} &={} [ ]", name(64)));
}
