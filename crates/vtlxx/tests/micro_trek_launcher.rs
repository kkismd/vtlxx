use std::io::Write;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const SOURCE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/micro_trek.vtl");

fn run(binary: &str, args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(binary)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input).unwrap();
    }
    wait_with_output(child)
}

fn wait_with_output(mut child: Child) -> Output {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!("launcher timed out: {output:?}");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn rust_source(input: &[u8]) -> Output {
    run(env!("CARGO_BIN_EXE_vtlxx"), &[SOURCE], input)
}

#[test]
fn fixed_seed_displays_first_and_preserves_game_input_order() {
    let input = b"G\nX\n";
    let expected = rust_source(b"0\nG\nX\n");
    let first = run(env!("CARGO_BIN_EXE_micro_trek"), &["--seed", "0"], input);
    let repeated = run(env!("CARGO_BIN_EXE_micro_trek"), &["--seed", "0"], input);
    assert_eq!(first.status, expected.status);
    assert_eq!(first.status, repeated.status);
    let mut expected_stdout = b"SEED: 0\n".to_vec();
    expected_stdout.extend_from_slice(&expected.stdout);
    assert_eq!(first.stdout, expected_stdout);
    assert_eq!(first.stdout, repeated.stdout);
    assert_eq!(first.stderr, expected.stderr);
    assert_eq!(first.stderr, repeated.stderr);
    assert!(first.stdout.ends_with(b"COMMAND :"));
}

#[test]
fn signed_seed_limits_start_the_game_and_match_the_direct_rust_cli() {
    for seed in [1_i16, -1, i16::MIN, i16::MAX] {
        let input = format!("{seed}\nX\n");
        let expected = rust_source(input.as_bytes());
        let seed_arg = seed.to_string();
        let args = ["--seed", seed_arg.as_str()];
        let launched = run(env!("CARGO_BIN_EXE_micro_trek"), &args, b"X\n");
        assert_eq!(launched.status, expected.status, "seed {seed}");
        let mut expected_stdout = format!("SEED: {seed}\n").into_bytes();
        expected_stdout.extend_from_slice(&expected.stdout);
        assert_eq!(launched.stdout, expected_stdout, "seed {seed}");
        assert_eq!(launched.stderr, expected.stderr, "seed {seed}");
    }
}

#[test]
fn automatic_seed_is_reported_and_used_by_the_game() {
    let launched = run(env!("CARGO_BIN_EXE_micro_trek"), &[], b"X\n");
    assert!(launched.status.success(), "{:?}", launched.stderr);
    let (seed_line, game_output) = launched.stdout.split_at(
        launched
            .stdout
            .iter()
            .position(|byte| *byte == b'\n')
            .expect("launcher prints a seed line")
            + 1,
    );
    let seed_text = std::str::from_utf8(seed_line)
        .unwrap()
        .strip_prefix("SEED: ")
        .unwrap()
        .trim_end();
    let seed: i16 = seed_text
        .parse()
        .expect("reported seed is signed16 decimal");
    let expected = rust_source(format!("{seed}\nX\n").as_bytes());
    assert_eq!(launched.status, expected.status);
    assert_eq!(game_output, expected.stdout);
    assert_eq!(launched.stderr, expected.stderr);
}

#[test]
fn malformed_arguments_do_not_start_or_print_a_seed() {
    for args in [
        vec!["--target", "other"],
        vec!["--target", "rust", "--target", "6502"],
        vec!["--seed"],
        vec!["--seed", "32768"],
        vec!["--seed", "oops"],
        vec!["--seed", "0", "--seed", "1"],
        vec!["unexpected"],
        vec!["--unknown"],
    ] {
        let result = run(env!("CARGO_BIN_EXE_micro_trek"), &args, b"");
        assert!(!result.status.success(), "args {args:?}");
        assert!(
            result.stdout.is_empty(),
            "args {args:?}: {:?}",
            result.stdout
        );
        assert!(!result.stderr.is_empty(), "args {args:?}");
    }
}

#[test]
fn help_describes_target_and_seed_options() {
    let result = run(env!("CARGO_BIN_EXE_micro_trek"), &["--help"], b"");
    assert!(result.status.success());
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert!(stdout.contains("--target rust|6502"));
    assert!(stdout.contains("--seed N"));
}
