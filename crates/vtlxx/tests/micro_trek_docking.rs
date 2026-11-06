use std::io::{self, Write};
use vtlxx::{Machine, RuntimeError, SourceError};

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const PREFIX: &[i16] = &[-32768, 123, 32767];

fn machine(x: i16, y: i16) -> Machine {
    let mut m = Machine::new();
    m.execute_source("I=0 *=I<240 [ @=I,I+1000 I=I+1 ] I=71 *=I<135 [ @=I,0 I=I+1 ]")
        .unwrap();
    m.execute_source(&format!("@=167,{x} @=168,{y} @=170,0"))
        .unwrap();
    let (definitions, _) = SOURCE.split_once("; EC08_TEST_DEFINITIONS_END").unwrap();
    m.execute_source(definitions).unwrap();
    for &value in PREFIX {
        m.push(value);
    }
    m
}

fn put_base(m: &mut Machine, x: i16, y: i16) {
    m.execute_source(&format!("@={},2", 71 + (x - 1) * 8 + y - 1))
        .unwrap();
}

fn snapshot(m: &Machine) -> Vec<i16> {
    (0..240).map(|i| m.storage(i)).collect()
}

fn assert_result(m: &Machine, before: &[i16], docked: bool) {
    assert_eq!(m.stack(), PREFIX);
    assert_storage(m, before, docked);
}

fn assert_storage(m: &Machine, before: &[i16], docked: bool) {
    for (i, &value) in before.iter().enumerate() {
        let expected = match (i, docked) {
            (170, _) => i16::from(docked),
            (161, true) => 4000,
            (162, true) => 10,
            (64..=69, true) => 0,
            _ => value,
        };
        assert_eq!(m.storage(i as u16), expected, "storage {i}");
    }
}

#[test]
fn all_eight_directions_and_multiple_bases_resupply_once() {
    for dx in -1..=1 {
        for dy in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let mut m = machine(4, 4);
            put_base(&mut m, 4 + dx, 4 + dy);
            let before = snapshot(&m);
            m.execute_source("dock=()").unwrap();
            assert_result(&m, &before, true);
            assert_eq!(m.output(), b"DOCKED IN\n");
        }
    }
    let mut m = machine(4, 4);
    put_base(&mut m, 3, 3);
    put_base(&mut m, 5, 5);
    let before = snapshot(&m);
    m.execute_source("dock=()").unwrap();
    assert_result(&m, &before, true);
    assert_eq!(m.output(), b"DOCKED IN\n");
}

#[test]
fn corners_edges_and_row_wrap_do_not_match_outside_cells() {
    for &(x, y, bx, by) in &[(1, 1, 2, 2), (8, 8, 7, 7), (1, 8, 1, 7), (8, 1, 7, 1)] {
        let mut m = machine(x, y);
        put_base(&mut m, bx, by);
        let before = snapshot(&m);
        m.execute_source("dock=()").unwrap();
        assert_result(&m, &before, true);
        assert_eq!(m.output(), b"DOCKED IN\n");
    }
    for &(x, y, bx, by) in &[(1, 8, 2, 1), (8, 1, 7, 8), (4, 4, 4, 4), (1, 1, 3, 1)] {
        let mut m = machine(x, y);
        put_base(&mut m, bx, by);
        let before = snapshot(&m);
        m.execute_source("dock=()").unwrap();
        assert_result(&m, &before, false);
        assert!(m.output().is_empty());
    }
}

#[test]
fn repeated_docking_and_departure() {
    let mut m = machine(4, 4);
    put_base(&mut m, 3, 4);
    for count in 1..=2 {
        m.execute_source("@=161,42 @=162,3 @=64,9").unwrap();
        let before = snapshot(&m);
        m.execute_source("dock=()").unwrap();
        assert_result(&m, &before, true);
        assert_eq!(m.output(), b"DOCKED IN\n".repeat(count));
    }
    m.execute_source("@=90,0 @=161,12 @=162,2 @=64,7").unwrap();
    let before = snapshot(&m);
    m.execute_source("dock=()").unwrap();
    assert_result(&m, &before, false);
    assert_eq!(m.output(), b"DOCKED IN\nDOCKED IN\n");
}

struct Broken;
impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn output_error_propagates_after_resupply_completes() {
    let mut m = machine(4, 4);
    put_base(&mut m, 3, 4);
    let before = snapshot(&m);
    m.set_output(Broken);
    assert_eq!(
        m.execute_source("dock=()"),
        Err(SourceError::Runtime(RuntimeError::OutputError))
    );
    assert!(m.stack().starts_with(PREFIX));
    assert_storage(&m, &before, true);
}
