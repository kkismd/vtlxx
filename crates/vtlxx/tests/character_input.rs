use std::io::{self, Cursor, Read};
use vtlxx::{InputEvent, Machine, ReadMode, RuntimeError, RuntimeInput, SourceError};

struct ScriptedInput(Vec<io::Result<InputEvent>>);

impl RuntimeInput for ScriptedInput {
    fn read_event(&mut self, _: ReadMode) -> io::Result<InputEvent> {
        self.0.remove(0)
    }
}

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

#[test]
fn try_read_maps_not_ready_bytes_nul_and_sticky_eof() {
    let events = vec![
        Ok(InputEvent::NotReady),
        Ok(InputEvent::NotReady),
        Ok(InputEvent::Byte(1)),
        Ok(InputEvent::Byte(127)),
        Ok(InputEvent::Byte(128)),
        Ok(InputEvent::Byte(255)),
        Ok(InputEvent::Byte(0)),
        Ok(InputEvent::Byte(0)),
        Ok(InputEvent::Eof),
    ];
    let mut machine = Machine::new();
    machine.set_runtime_input(ScriptedInput(events));
    machine
        .execute_source("A=` B=` C=` D=` E=` F=` G=` H=` I=` J=`")
        .unwrap();
    assert_eq!(
        (0..10)
            .map(|index| machine.register(index).unwrap())
            .collect::<Vec<_>>(),
        [0, 0, 1, 127, 128, 255, 0, 0, -1, -1]
    );
}

#[test]
fn wait_and_try_share_one_input_session_across_source_calls() {
    let mut machine = Machine::new();
    machine.set_runtime_input(ScriptedInput(vec![
        Ok(InputEvent::NotReady),
        Ok(InputEvent::Byte(0)),
        Ok(InputEvent::Byte(65)),
        Ok(InputEvent::Byte(66)),
        Ok(InputEvent::Eof),
    ]));
    machine.execute_source("A=` B=$ C=`").unwrap();
    machine.execute_source("D=$ E=`").unwrap();
    assert_eq!(
        [0, 1, 2, 3, 4].map(|index| machine.register(index).unwrap()),
        [0, 65, 66, -1, -1]
    );

    let mut machine = Machine::new();
    machine.set_runtime_input(ScriptedInput(vec![
        Ok(InputEvent::Byte(70)),
        Ok(InputEvent::Byte(71)),
    ]));
    machine.execute_source("&=p [ B=` ] p=() A=`").unwrap();
    assert_eq!(
        (machine.register(0), machine.register(1)),
        (Some(71), Some(70))
    );
}

#[test]
fn input_failures_and_invalid_wait_events_preserve_stack() {
    let mut machine = Machine::new();
    machine.set_runtime_input(ScriptedInput(vec![Err(io::Error::other("failed"))]));
    machine.push(12);
    assert_eq!(
        machine.execute_source("A=`"),
        Err(SourceError::Runtime(RuntimeError::InputError))
    );
    assert_eq!(machine.stack(), &[12]);

    let mut machine = Machine::new();
    machine.set_runtime_input(ScriptedInput(vec![Ok(InputEvent::NotReady)]));
    machine.push(34);
    assert_eq!(
        machine.execute_source("A=$"),
        Err(SourceError::Runtime(RuntimeError::InputError))
    );
    assert_eq!(machine.stack(), &[34]);
}

#[test]
fn read_input_compatibility_rejects_try_without_reading() {
    struct MustNotRead;
    impl Read for MustNotRead {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            panic!("Try must not call a blocking Read")
        }
    }

    let mut machine = Machine::new();
    machine.set_input(MustNotRead);
    machine.push(55);
    assert_eq!(
        machine.execute_source("A=`"),
        Err(SourceError::Runtime(RuntimeError::InputError))
    );
    assert_eq!(machine.stack(), &[55]);

    machine.set_input(Cursor::new(vec![0, 65]));
    machine.execute_source("A=$").unwrap();
    assert_eq!(machine.register(0), Some(65));
}

#[test]
fn replacing_input_resets_eof_and_default_input_is_eof() {
    let mut machine = Machine::new();
    machine.execute_source("A=`").unwrap();
    assert_eq!(machine.register(0), Some(-1));
    machine.set_runtime_input(ScriptedInput(vec![Ok(InputEvent::Byte(90))]));
    machine.execute_source("B=`").unwrap();
    assert_eq!(machine.register(1), Some(90));
    machine.set_input(Cursor::new(vec![66]));
    machine.execute_source("C=$").unwrap();
    assert_eq!(machine.register(2), Some(66));
}
