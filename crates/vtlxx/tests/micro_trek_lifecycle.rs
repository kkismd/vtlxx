use std::io::{self, Cursor, Read, Write};
use std::process::{Command, Stdio};
use vtlxx::{Machine, RuntimeError, SourceError};

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const PREFIX: &[i16] = &[-32768, 123, 32767];

fn machine(input: &[u8]) -> Machine {
    let mut m = Machine::new();
    m.execute_source(SOURCE.split_once("; EC08_TEST_DEFINITIONS_END").unwrap().0)
        .unwrap();
    m.execute_source("@=160,30 @=192,123").unwrap();
    m.set_input(Cursor::new(input.to_vec()));
    for &value in PREFIX {
        m.push(value);
    }
    m
}

#[test]
fn retry_reads_complete_lines_and_preserves_state() {
    for (input, expected, prompts) in [
        (b"Y\n".as_slice(), 1, 1),
        (b"n \t\r\n", 0, 1),
        (b"y\r", 1, 1),
        (b"N", 0, 1),
        (b"", -1, 1),
        (b"YES\nN\n", 0, 2),
        (b" Y\nY\n", 1, 2),
        (b"Y\rX\nN\n", 0, 2),
        (b"bad", -1, 1),
    ] {
        let mut m = machine(input);
        let before: Vec<_> = (0..240).map(|i| m.storage(i)).collect();
        m.execute_source("gameend=1").unwrap();
        let mut stack = PREFIX.to_vec();
        stack.push(expected);
        assert_eq!(m.stack(), stack, "{input:?}");
        assert_eq!((0..240).map(|i| m.storage(i)).collect::<Vec<_>>(), before);
        let output = String::from_utf8_lossy(m.output());
        assert!(
            output.starts_with(
                "IT IS STARDATE : 3200\n KLINGON ALL DESTROYED!! \nCONGRATURATION!!!\n"
            )
        );
        assert_eq!(output.matches("RETRY?(Y/N)").count(), prompts);
    }
    let mut m = machine(b"N\nX\n");
    m.execute_source("gameend=2").unwrap();
    assert_eq!(m.pop(), Some(0));
    assert!(
        String::from_utf8_lossy(m.output())
            .starts_with("IT IS STARDATE : 3200\nPLANETS UNION WAS CONQUERED \n")
    );
    m.execute_source("command=()").unwrap();
    assert_eq!(m.pop(), Some(-1));
    assert_eq!(m.stack(), PREFIX);
}

#[test]
fn replay_initialization_continues_the_random_stream() {
    fn run() -> (Vec<i16>, i16, Vec<i16>, i16) {
        let mut m = machine(b"Y\n");
        m.execute_source("@=192,0 initgame=()").unwrap();
        let first = (0..171).map(|i| m.storage(i)).collect();
        let first_seed = m.storage(192);
        m.execute_source("gameend=1").unwrap();
        assert_eq!(m.pop(), Some(1));
        assert_eq!(m.storage(192), first_seed);
        m.execute_source("initgame=()").unwrap();
        let second = (0..171).map(|i| m.storage(i)).collect();
        assert_eq!(m.stack(), PREFIX);
        (first, first_seed, second, m.storage(192))
    }
    let result = run();
    assert_eq!(result, run());
    assert_ne!(result.0, result.2);
    assert_ne!(result.1, result.3);
}

#[test]
fn retry_input_failure_is_not_eof() {
    struct FailedInput;
    impl Read for FailedInput {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }
    let mut m = machine(b"");
    m.set_input(FailedInput);
    assert_eq!(
        m.execute_source("gameend=1"),
        Err(SourceError::Runtime(RuntimeError::InputError))
    );
}

#[test]
fn cli_starts_and_exits_on_x_or_eof() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/micro_trek.vtl");
    for input in [b"X\n".as_slice(), b""] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ecvtl"))
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{:?}", output.stderr);
        assert_eq!(output.stdout, b"IT IS STARDATE : 3200\nOBJECTIVE :\nDESTROY 26 KLINGON CRUISERS\nIN 30 YEARS!\nTHE NUMBER OF BASE IS 3\nCOMMAND :");
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn cli_cancel_and_committed_action_return_to_command() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/micro_trek.vtl");
    for (input, marker, stardate) in [
        (
            b"P\n0\nR\nX\n".as_slice(),
            "*PHASER*\nENERGY ? ",
            "*STARDATE : 3200\n",
        ),
        (
            b"W\n1\n0\nR\nX\n",
            "* ION ENGINE START\n",
            "*STARDATE : 3201\n",
        ),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ecvtl"))
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(result.status.success(), "{:?}", result.stderr);
        let output = String::from_utf8(result.stdout).unwrap();
        assert!(output.contains(marker));
        assert!(output.contains(stardate));
        assert_eq!(output.matches("COMMAND :").count(), 3);
        assert!(!output.contains("RETRY?(Y/N)"));
    }
}

#[test]
fn cli_replay_initializes_a_second_mission_without_reseeding() {
    fn play() -> Vec<u8> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/micro_trek.vtl");
        let mut child = Command::new(env!("CARGO_BIN_EXE_ecvtl"))
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        let mut output = child.stdout.take().unwrap();
        let mut captured = Vec::new();
        let mut turns = 0;
        let mut replayed = false;
        let mut byte = [0];
        while output.read(&mut byte).unwrap() != 0 {
            captured.push(byte[0]);
            if captured.ends_with(b"COMMAND :") {
                if replayed {
                    input.write_all(b"X\n").unwrap();
                } else {
                    turns += 1;
                    assert!(turns <= 30);
                    input.write_all(b"W\n1\n0\n").unwrap();
                }
            } else if captured.ends_with(b"RETRY?(Y/N)") {
                assert!(!replayed);
                replayed = true;
                input.write_all(b"Y\n").unwrap();
            }
        }
        assert!(replayed);
        assert!(child.wait().unwrap().success());
        captured
    }
    let first = play();
    assert_eq!(first, play());
    let text = String::from_utf8(first).unwrap();
    let objectives: Vec<_> = text
        .lines()
        .filter(|line| line.starts_with("DESTROY "))
        .collect();
    assert_eq!(objectives.len(), 2);
    assert_ne!(objectives[0], objectives[1]);
    assert_eq!(text.matches("RETRY?(Y/N)").count(), 1);
}
