use std::io::{self, Cursor, Read};
use vtlxx::{Machine, RuntimeError, SourceError};

#[test]
fn characters_nuls_eof_and_output() {
    let mut machine = Machine::new();
    machine.set_input(Cursor::new(vec![0, 0, 10, 13, 65, 127, 128, 255, 0]));
    machine
        .execute_source("A=$ B=$ C=$ D=$ E=$ F=$ G=$ H=$ $=0 $=65 $=-1")
        .unwrap();
    for (i, value) in [10, 13, 65, 127, 128, 255, -1, -1].into_iter().enumerate() {
        assert_eq!(machine.register(i), Some(value));
    }
    assert_eq!(machine.output(), &[0, 65, 255]);
    let mut empty = Machine::new();
    empty.execute_source("A=$ B=$").unwrap();
    assert_eq!((empty.register(0), empty.register(1)), (Some(-1), Some(-1)));
}

#[test]
fn source_and_nested_procedures_share_session_but_machines_do_not() {
    let mut machine = Machine::new();
    machine.set_input(Cursor::new(vec![0, 65, 0, 66, 67, 68]));
    machine
        .execute_source("&=p [ B=$ ] &=q [ p=() C=$ ] A=$ q=() $=A X=$+1")
        .unwrap();
    assert_eq!(
        (
            machine.register(0),
            machine.register(1),
            machine.register(2),
            machine.register(23)
        ),
        (Some(65), Some(66), Some(67), Some(69))
    );
    assert_eq!(machine.output(), b"A");
    let mut other = Machine::new();
    other.set_input(Cursor::new(vec![90]));
    other.execute_source("A=$").unwrap();
    machine.execute_source("D=$").unwrap();
    assert_eq!(
        (other.register(0), machine.register(3)),
        (Some(90), Some(-1))
    );
}

#[test]
fn compile_failures_preserve_only_already_executed_effects() {
    for source in ["A=$ B=2+", "A=$ B=z"] {
        let mut machine = Machine::new();
        machine.set_input(Cursor::new(vec![65, 66]));
        assert!(matches!(
            machine.execute_source(source),
            Err(SourceError::Compile(_))
        ));
        machine.execute_source("A=$").unwrap();
        assert_eq!(machine.register(0), Some(65));
    }
    let mut machine = Machine::new();
    machine.set_input(Cursor::new(vec![65, 66]));
    assert!(matches!(
        machine.execute_source("A=$ $=A &=p [ B=z ]"),
        Err(SourceError::Compile(_))
    ));
    assert_eq!(machine.output(), b"A");
    machine.execute_source("B=$").unwrap();
    assert_eq!(machine.register(1), Some(66));
}

#[test]
fn reader_error_propagates_and_stops_source() {
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("failure"))
        }
    }
    let mut machine = Machine::new();
    machine.set_input(Broken);
    machine.push(99);
    assert_eq!(
        machine.execute_source("A=$ B=7"),
        Err(SourceError::Runtime(RuntimeError::InputError))
    );
    assert_eq!(machine.stack(), &[99]);
    assert_eq!(machine.register(1), Some(0));
}
