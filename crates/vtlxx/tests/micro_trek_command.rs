use std::io::{self, Cursor, Read};
use vtlxx::{Machine, RuntimeError, SourceError};

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const PREFIX: &[i16] = &[-32768, 123, 32767];

fn machine(input: &[u8]) -> Machine {
    let mut m = Machine::new();
    m.execute_source(SOURCE.split_once("; EC08_TEST_DEFINITIONS_END").unwrap().0)
        .unwrap();
    m.execute_source("@=160,30 @=161,4000 @=162,3 @=163,1 @=165,4 @=166,4 @=167,4 @=168,4 @=98,4")
        .unwrap();
    m.set_input(Cursor::new(input.to_vec()));
    for &v in PREFIX {
        m.push(v);
    }
    m
}

fn call(m: &mut Machine, status: i16) {
    m.execute_source("command=()").unwrap();
    let mut expected = PREFIX.to_vec();
    expected.push(status);
    assert_eq!(m.stack(), expected);
    assert_eq!(m.pop(), Some(status));
}

#[test]
fn invalid_lines_are_drained_without_dispatch_or_mutation() {
    for input in [
        b"Pxyz\n".as_slice(),
        b" P\n",
        b"P P\n",
        b"\n",
        b"P\rX\n",
        b"YES\n",
    ] {
        let mut m = machine(input);
        let before: Vec<_> = (0..240).map(|i| m.storage(i)).collect();
        call(&mut m, 0);
        assert_eq!(m.output(), b"COMMAND :");
        assert_eq!((0..240).map(|i| m.storage(i)).collect::<Vec<_>>(), before);
        call(&mut m, -1);
    }
    for input in [b"".as_slice(), b"Pxyz", b"P\rX", b" "] {
        let mut m = machine(input);
        call(&mut m, -1);
        assert_eq!(m.output(), b"COMMAND :");
    }
}

#[test]
fn help_is_static_and_eof_and_cr_are_handled() {
    for input in [b"?\n".as_slice(), b"? \t\r\n", b"?\r", b"?\x00\n"] {
        let mut m = machine(input);
        call(&mut m, 0);
        assert_eq!(m.output(), b"COMMAND :*HELP*\nS : SHORT RANGE SENSOR\nG : GALAXY MAP\nL : LONG RANGE SENSOR\nP : PHASER\nW : WARP\nT : TORPEDOES\nR : STATUS REPORT\nX : EXIT\n? : HELP\n");
        call(&mut m, -1);
    }
    for input in [b"X\n".as_slice(), b"x", b"x\r\n"] {
        let mut m = machine(input);
        call(&mut m, -1);
    }
}

#[test]
fn real_commands_consume_their_own_input_and_return_status() {
    for line in ["P\n1\n", "p\r\n1\r\n", "P \t\n1\n"] {
        let mut m = machine(line.as_bytes());
        call(&mut m, 1);
        assert!(
            m.output()
                .starts_with(b"COMMAND :*PHASER*\nENERGY ? MISSED!!\n")
        );
        assert_eq!(m.storage(161), 3999);
        call(&mut m, -1);
    }
    for command in ["G", "g", "L", "l", "S", "s", "R", "r"] {
        let mut m = machine(format!("{command}\n").as_bytes());
        call(&mut m, 0);
        assert!(m.output().len() > b"COMMAND :".len());
    }
    for command in ["W", "w", "T", "t"] {
        let mut m = machine(format!("{command}\n").as_bytes());
        call(&mut m, -1);
    }
    for command in ["W", "w"] {
        let mut m = machine(format!("{command}\n1\n0\n").as_bytes());
        call(&mut m, 1);
        assert_eq!(m.storage(160), 29);
        assert_eq!(m.storage(161), 3999);
    }
    for command in ["T", "t"] {
        let mut m = machine(format!("{command}\n0\n").as_bytes());
        call(&mut m, 1);
        assert_eq!(m.storage(162), 2);
    }
    let mut m = machine(b"P\n0\n");
    call(&mut m, 0);
    assert_eq!(m.storage(161), 4000);
    let mut m = machine(b"G\nP\n1\n");
    call(&mut m, 0);
    call(&mut m, 1);
    assert_eq!(m.storage(161), 3999);
}

struct FailedInput;
impl Read for FailedInput {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
}

#[test]
fn fatal_input_error_is_not_eof() {
    let mut m = machine(b"");
    m.set_input(FailedInput);
    assert_eq!(
        m.execute_source("command=()"),
        Err(SourceError::Runtime(RuntimeError::InputError))
    );
}
