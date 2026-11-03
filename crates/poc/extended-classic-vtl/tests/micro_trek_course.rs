use std::io::{self, Cursor, Read};
use vtlxx_poc_extended_classic_vtl::{Machine, RuntimeError, SourceError};

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const PREFIX: &[i16] = &[-32768, 1234, -7, 32767];
const PROMPT: &str = "COURSE: ";

fn machine(input: impl Read + 'static) -> Machine {
    let mut m = Machine::new();
    m.execute_source("I=0 *=I<240 [ @=I,I+2000 I=I+1 ]")
        .unwrap();
    let mut parts = SOURCE.split("; EC08_TEST_DEFINITIONS_END");
    m.execute_source(parts.next().unwrap()).unwrap();
    assert!(parts.next().is_some() && parts.next().is_none());
    m.execute_source("I=109 J=110 K=111 L=112 M=113 N=114 O=115 P=116 Q=117 R=118 S=119 T=120 U=121 V=122 W=123 X=124 Y=125 Z=126").unwrap();
    m.set_input(input);
    for &value in PREFIX {
        m.push(value);
    }
    m
}

fn check_call(m: &mut Machine, expected: (i16, i16, i16), prompts: usize) {
    m.execute_source("c=()").unwrap();
    let mut stack = PREFIX.to_vec();
    stack.extend([expected.0, expected.1, expected.2]);
    assert_eq!(m.stack(), stack);
    assert_eq!(m.output(), PROMPT.repeat(prompts).as_bytes());
    for i in 8..26 {
        assert_eq!(m.register(i), Some(i as i16 + 101), "register {i}");
    }
    for i in 0..240 {
        assert_eq!(m.storage(i), i as i16 + 2000, "storage {i}");
    }
    m.execute_source("A=~ A=~ A=~").unwrap();
    assert_eq!(m.stack(), PREFIX);
}

#[test]
fn course_directions_and_octant_boundaries() {
    for (angle, dx, dy) in [
        (0, -45, 0),
        (44, -45, 44),
        (45, -45, 45),
        (46, -44, 45),
        (89, -1, 45),
        (90, 0, 45),
        (134, 44, 45),
        (135, 45, 45),
        (180, 45, 0),
        (225, 45, -45),
        (270, 0, -45),
        (315, -45, -45),
        (359, -45, -1),
        (360, -45, 0),
    ] {
        let mut m = machine(Cursor::new(format!("{angle}\n").into_bytes()));
        check_call(&mut m, (dx, dy, 1), 1);
        check_call(&mut m, (0, 0, -1), 2);
    }
}

#[test]
fn retry_cursor_and_eof() {
    let mut m = machine(Cursor::new(b"bad\n\n+\n32768\n-1\n361\n45\r\n90".to_vec()));
    check_call(&mut m, (-45, 45, 1), 7);
    check_call(&mut m, (0, 45, 1), 8);
    check_call(&mut m, (0, 0, -1), 9);
    check_call(&mut m, (0, 0, -1), 10);

    let mut m = machine(Cursor::new(b"bad\ninvalid".to_vec()));
    check_call(&mut m, (0, 0, -1), 3);
    let mut m = machine(Cursor::new(Vec::new()));
    check_call(&mut m, (0, 0, -1), 1);
}

#[test]
fn fatal_input_error_propagates() {
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("failure"))
        }
    }
    let mut m = machine(Broken);
    assert_eq!(
        m.execute_source("c=()"),
        Err(SourceError::Runtime(RuntimeError::InputError))
    );
    assert_eq!(m.output(), PROMPT.as_bytes());
}
