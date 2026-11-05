use std::io::{self, Cursor, Read};
use vtlxx_poc_extended_classic_vtl::{Machine, RuntimeError, SourceError};

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const PREFIX: &[i16] = &[-32768, 1234, -7, 32767];

fn machine(input: impl Read + 'static) -> Machine {
    let mut m = Machine::new();
    m.execute_source("I=0 *=I<240 [ @=I,I+2000 I=I+1 ]")
        .unwrap();
    let mut parts = SOURCE.split("; EC08_TEST_DEFINITIONS_END");
    m.execute_source(parts.next().unwrap()).unwrap();
    assert!(parts.next().is_some() && parts.next().is_none());
    m.execute_source("I=109 J=110 K=111 L=112 M=113 N=114 O=115 P=116 Q=117 R=118 S=119 T=120 U=121 V=122 W=123 X=124 Y=125 Z=126").unwrap();
    m.set_input(input);
    for &v in PREFIX {
        m.push(v);
    }
    m
}

fn check(input: &[u8], expected: &[(i16, i16)]) {
    let mut m = machine(Cursor::new(input.to_vec()));
    for &(value, status) in expected {
        m.execute_source("readnum=()").unwrap();
        let mut stack = PREFIX.to_vec();
        stack.extend([value, status]);
        assert_eq!(m.stack(), stack, "input {input:?}");
        m.execute_source("A=~ A=~").unwrap();
        assert_eq!(m.stack(), PREFIX);
        assert!(m.output().is_empty());
        for i in 8..26 {
            assert_eq!(m.register(i), Some(i as i16 + 101), "register {i}");
        }
        for i in 0..240 {
            assert_eq!(m.storage(i), i as i16 + 2000, "storage {i}");
        }
    }
}

#[test]
fn decimal_lines_and_boundaries() {
    for (input, value) in [
        (&b"0\n"[..], 0),
        (&b"+12\n"[..], 12),
        (&b"-13\n"[..], -13),
        (&b"  -32768 \n"[..], -32768),
        (&b"32767\n"[..], 32767),
        (&b"0\r\n"[..], 0),
        (&b"42"[..], 42),
        (&b"12\r"[..], 12),
        (&b"1\x002\n"[..], 12),
        (&b"\t+0007\t\n"[..], 7),
    ] {
        check(input, &[(value, 1), (0, -1), (0, -1)]);
    }
    for input in [
        &b"\n"[..],
        &b"+\n"[..],
        &b"--1\n"[..],
        &b"1 2\n"[..],
        &b"32768\n"[..],
        &b"-32769\n"[..],
        &b"\xff\n"[..],
        &b"1\rX\n"[..],
        &b"\x00\n"[..],
        &b"4x"[..],
        &b"\r\n"[..],
    ] {
        check(input, &[(0, 0), (0, -1)]);
    }
    check(b"\x00", &[(0, -1), (0, -1)]);
    check(b"bad\n17\n", &[(0, 0), (17, 1), (0, -1)]);
    check(b"32768more\n-1\n", &[(0, 0), (-1, 1), (0, -1)]);
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
        m.execute_source("readnum=()"),
        Err(SourceError::Runtime(RuntimeError::InputError))
    );
}
