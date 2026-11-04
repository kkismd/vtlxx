use std::io::{self, Cursor, Write};
use vtlxx_poc_extended_classic_vtl::{Machine, RuntimeError, SourceError};

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const PREFIX: &[i16] = &[-32768, 123, 32767];

fn machine(input: &str) -> Machine {
    let mut m = Machine::new();
    m.execute_source(SOURCE.split_once("; EC08_TEST_DEFINITIONS_END").unwrap().0)
        .unwrap();
    m.execute_source("@=160,19 @=161,4000 @=162,7 @=165,4 @=166,4 @=167,4 @=168,4 @=170,1 @=27,1200 @=163,2 @=169,2 @=135,300 @=137,2 @=141,4 @=143,4 @=147,4 @=149,5 @=98,1 @=99,1 @=192,123 @=70,42").unwrap();
    m.set_input(Cursor::new(input.as_bytes().to_vec()));
    for &value in PREFIX {
        m.push(value);
    }
    m
}

fn call(m: &mut Machine, status: i16) {
    m.execute_source("f=()").unwrap();
    let mut expected = PREFIX.to_vec();
    expected.push(status);
    assert_eq!(m.stack(), expected);
}

#[test]
fn cancel_eof_and_damaged_device_leave_state_unchanged() {
    for (input, damaged, status) in [("0\n", 0, 0), ("", 0, -1), ("5\n", 1, 0), ("5\n", 3, 0)] {
        let mut m = machine(input);
        m.execute_source(&format!("@=67,{damaged}")).unwrap();
        call(&mut m, status);
        assert_eq!(
            (
                m.storage(161),
                m.storage(135),
                m.storage(137),
                m.storage(192),
                m.storage(67)
            ),
            (4000, 300, 2, 123, damaged)
        );
        if damaged != 0 {
            assert_eq!(
                m.output(),
                format!("*PHASER*DAMAGED!\n{damaged}\n YEARS ESTIMATED FOR REPAIR.\n").as_bytes()
            );
            m.execute_source("A=~ n=()").unwrap();
            let mut expected = PREFIX.to_vec();
            expected.extend([5, 1]);
            assert_eq!(m.stack(), expected);
        }
    }
}

#[test]
fn invalid_and_insufficient_input_retry_then_miss() {
    let mut m = machine("bad\n-1\n4001\n1\n");
    m.execute_source("@=169,0").unwrap();
    call(&mut m, 1);
    assert_eq!(m.output(), b"*PHASER*\nENERGY ? ENERGY ? ENERGY ? SORRY ! WE HAVE ONLY 4000 UNITS\nENERGY ? MISSED!!\n");
    assert_eq!((m.storage(161), m.storage(192)), (3999, 123));
}

#[test]
fn fixed_initial_count_and_holes_preserve_distribution() {
    let mut m = machine("4\n");
    m.execute_source("@=135,2 @=137,300").unwrap();
    call(&mut m, 1);
    assert_eq!(
        (
            m.storage(135),
            m.storage(137),
            m.storage(169),
            m.storage(163)
        ),
        (0, 298, 1, 1)
    );
    assert_eq!(m.storage(27), 1100);
    assert_eq!(m.output(), b"*PHASER*\nENERGY ? 3 UNITS HIT  KLINGON(44)\n### DESTROYED ###\n2 UNITS HIT  KLINGON(45)\n# DAMAGED #\n");
    assert_eq!(
        (m.storage(161), m.storage(192), m.storage(70)),
        (3996, 123, 42)
    );
}

#[test]
fn overload_is_once_and_power_nine_persists() {
    for (input, overload) in [("998\n", false), ("1000\n", true)] {
        let mut m = machine(input);
        call(&mut m, 1);
        let output = String::from_utf8(m.output().to_vec()).unwrap();
        assert_eq!(
            output.matches("PHASER'S OVER LOAD !").count(),
            usize::from(overload)
        );
        if overload {
            assert_eq!(m.storage(67), 1);
            assert!(output.contains("10 UNITS HIT  KLINGON(44)"));
            assert!(output.contains("9 UNITS HIT  KLINGON(45)"));
        } else {
            assert_eq!(m.storage(67), 0);
        }
        assert_eq!(m.storage(192), 123);
    }
}

struct Fail;
impl Write for Fail {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn output_error_propagates() {
    let mut m = machine("1\n");
    m.set_output(Fail);
    assert_eq!(
        m.execute_source("f=()"),
        Err(SourceError::Runtime(RuntimeError::OutputError))
    );
    assert_eq!(m.storage(161), 4000);
}

#[test]
fn energy_one_and_five_live_slots() {
    let mut m = machine("1\n");
    m.execute_source("@=161,1 @=169,1 @=137,0 @=163,1 @=27,1100")
        .unwrap();
    call(&mut m, 1);
    assert_eq!(
        (m.storage(161), m.storage(135), m.storage(192)),
        (0, 298, 123)
    );
    let mut m = machine("5\n");
    m.execute_source("@=169,5 @=163,5 @=27,1500 @=136,300 @=137,300 @=138,300 @=139,300 @=142,4 @=143,4 @=144,4 @=145,4 @=148,4 @=149,4 @=150,4 @=151,4").unwrap();
    call(&mut m, 1);
    assert_eq!(
        String::from_utf8_lossy(m.output())
            .matches(" UNITS HIT  KLINGON(")
            .count(),
        5
    );
    assert_eq!((m.storage(161), m.storage(192)), (3995, 123));
}
