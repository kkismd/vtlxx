use std::fs;
use std::process::Command;

// Reuse the actual source definition, stopping before the drawing procedure.
const SOURCE: &str = include_str!("../examples/mandelbrot.vtl");
const CASES: &str = "\n  ~=777\n  fixedmul=0,0\n  fixedmul=124,124\n  fixedmul=0-124,124\n  fixedmul=250,250\n  fixedmul=250,0-250\n  fixedmul=1,1\n  fixedmul=0-1,124\n  ?=~\n  ?=()\n  ?=~\n  ?=()\n  ?=~\n  ?=()\n  ?=~\n  ?=()\n  ?=~\n  ?=()\n  ?=~\n  ?=()\n  ?=~\n  ?=()\n  ?=~\n  ?=()\n";
const EXPECTED: &[u8] = b"0\n0\n-500\n500\n-123\n123\n0\n777\n";

fn check(binary: &str, sim65: bool) {
    let prefix = SOURCE.split("  ; point:").next().unwrap();
    let path =
        std::env::temp_dir().join(format!("vtlxx-fixedmul-{}-{sim65}.vtl", std::process::id()));
    fs::write(&path, format!("{prefix}{CASES}")).unwrap();
    let mut command = Command::new(binary);
    command.arg(&path);
    if sim65 {
        command.args(["--engine", "sim65"]);
    }
    let output = command.output().unwrap();
    fs::remove_file(path).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, EXPECTED);
}

#[test]
fn fixedmul_rust() {
    check(env!("CARGO_BIN_EXE_vtlxx"), false);
}

#[test]
#[ignore = "requires ca65, ld65, and sim65"]
fn fixedmul_sim65() {
    check(env!("CARGO_BIN_EXE_vtlxx6502"), true);
}
