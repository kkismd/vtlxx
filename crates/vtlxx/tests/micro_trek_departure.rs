use std::cell::RefCell;
use std::io::{self, Cursor, Write};
use std::rc::Rc;
use vtlxx::{Machine, RuntimeError, SourceError};

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const PREFIX: &[i16] = &[-32768, 123, 32767];

fn machine(input: &str) -> Machine {
    let mut m = Machine::new();
    m.execute_source(SOURCE.split_once("; EC08_TEST_DEFINITIONS_END").unwrap().0)
        .unwrap();
    m.execute_source("@=160,19 @=161,4000 @=162,7 @=165,4 @=166,4 @=167,4 @=168,4 @=170,1 @=98,4 @=192,0 @=70,42").unwrap();
    m.set_input(Cursor::new(input.as_bytes().to_vec()));
    for &v in PREFIX {
        m.push(v);
    }
    m
}

fn call(m: &mut Machine, status: i16) {
    m.execute_source("warp=()").unwrap();
    let mut expected = PREFIX.to_vec();
    expected.push(status);
    assert_eq!(m.stack(), expected);
}

#[test]
fn direct_input_to_real_trajectory_commits_once() {
    let mut m = machine("2\n0\n");
    m.execute_source("@=64,0 @=65,1 @=66,3").unwrap();
    call(&mut m, 1);
    assert_eq!(m.output(), b"*WARP*\n FACTOR ? COURSE: * ION ENGINE START\n44-44\n44-34\nNOW: QUADRANT(44) SECTOR(24)\n");
    assert_eq!(
        (m.storage(160), m.storage(161), m.storage(170)),
        (18, 3998, 0)
    );
    assert_eq!(
        (m.storage(64), m.storage(65), m.storage(66), m.storage(70)),
        (0, 0, 2, 42)
    );
    assert_eq!(
        (m.storage(98), m.storage(82), m.storage(192)),
        (0, 4, 13849)
    );
    assert_eq!((71..135).filter(|&i| m.storage(i) == 4).count(), 1);
}

#[test]
fn departure_crosses_multiple_quadrants_before_rebuilding_final_sector() {
    let mut m = machine("10\n0\n");
    // The old sector's (8,4) is occupied, but belongs to neither intermediate
    // nor final quadrant. The middle quadrant has enemies; only the final
    // quadrant's one enemy and one star must be constructed.
    m.execute_source("@=167,1 @=98,0 @=74,4 @=130,3 @=19,300 @=11,101 @=163,4 @=65,3 @=192,1")
        .unwrap();
    call(&mut m, 1);
    assert_eq!(
        m.output(),
        b"*WARP*\n FACTOR ? COURSE: * ION ENGINE START\n44-14\n*WARP ENGINE START\n34-84\n34-74\n34-64\n34-54\n34-44\n34-34\n34-24\n34-14\n24-84\n## SPACE STORM ##\n*PHASER*DAMAGED!\n1\n YEARS ESTIMATED FOR REPAIR.\nNOW: QUADRANT(24) SECTOR(74)\n"
    );
    assert_eq!(
        (
            m.storage(165),
            m.storage(166),
            m.storage(167),
            m.storage(168)
        ),
        (2, 4, 7, 4)
    );
    assert_eq!(
        (m.storage(160), m.storage(161), m.storage(170)),
        (18, 3950, 0)
    );
    assert_eq!((m.storage(65), m.storage(67), m.storage(70)), (2, 1, 42));
    // seed 1: storm, j's two draws, then q's enemy and star draws.
    assert_eq!(m.storage(192), 3882);
    assert_eq!(
        (
            m.storage(169),
            m.storage(135),
            m.storage(141),
            m.storage(147)
        ),
        (1, 300, 2, 6)
    );
    assert_eq!((71..135).filter(|&i| m.storage(i) == 4).count(), 1);
    assert_eq!((71..135).filter(|&i| m.storage(i) == 1).count(), 1);
    assert_eq!((71..135).filter(|&i| m.storage(i) == 3).count(), 1);
    assert_eq!(
        (
            m.storage(162),
            m.storage(163),
            m.storage(164),
            m.storage(11),
            m.storage(19)
        ),
        (7, 4, 0, 101, 300)
    );
}

#[test]
fn cancellation_and_eof_never_commit() {
    for (input, status, out) in [
        ("0\n", 0, "*WARP*\n FACTOR ? "),
        ("", -1, "*WARP*\n FACTOR ? "),
        ("1\n", -1, "*WARP*\n FACTOR ? COURSE: "),
        ("bad\n", -1, "*WARP*\n FACTOR ?  FACTOR ? "),
    ] {
        let mut m = machine(input);
        call(&mut m, status);
        assert_eq!(m.output(), out.as_bytes());
        assert_eq!(
            (
                m.storage(160),
                m.storage(161),
                m.storage(170),
                m.storage(98),
                m.storage(192),
                m.storage(70)
            ),
            (19, 4000, 1, 4, 0, 42)
        );
    }
}

#[test]
fn retry_factor_and_course_in_their_own_fields() {
    let mut m = machine("bad\n-1\n90\n2\n-1\n360\n");
    call(&mut m, 1);
    let out = String::from_utf8(m.output().to_vec()).unwrap();
    assert_eq!(out.matches(" FACTOR ? ").count(), 4);
    assert_eq!(out.matches("COURSE: ").count(), 2);
    assert_eq!(out.matches("*WARP*").count(), 1);
    assert!(out.contains("44-44\n44-34\n"));
    assert_eq!((m.storage(160), m.storage(161)), (18, 3998));
}

#[test]
fn damaged_warp_and_energy_retry() {
    let mut m = machine("3\n2\n0\n");
    m.execute_source("@=68,3").unwrap();
    call(&mut m, 1);
    let out = String::from_utf8(m.output().to_vec()).unwrap();
    assert!(out.starts_with("*WARP*DAMAGED!\n3\n YEARS ESTIMATED FOR REPAIR.\n FACTOR ? TRY 2 AT MOST !\n FACTOR ? COURSE: "));
    assert_eq!(m.storage(68), 2);
    let mut m = machine("2\n1\n0\n");
    m.execute_source("@=161,1").unwrap();
    call(&mut m, 1);
    assert!(
        String::from_utf8(m.output().to_vec())
            .unwrap()
            .contains(" FACTOR ? WE HAVE NO ENERGY !\n FACTOR ? COURSE: ")
    );
    assert_eq!(m.storage(161), 0);
}

#[test]
fn maximum_factor_uses_full_safe_energy_range() {
    let mut m = machine("89\n");
    m.execute_source("@=161,3960").unwrap();
    call(&mut m, -1);
    assert!(
        String::from_utf8(m.output().to_vec())
            .unwrap()
            .contains("WE HAVE NO ENERGY !\n")
    );
    assert_eq!(
        (m.storage(160), m.storage(161), m.storage(192)),
        (19, 3960, 0)
    );
    let mut m = machine("89\n0\n");
    m.execute_source("@=161,3961").unwrap();
    call(&mut m, 1);
    assert_eq!((m.storage(160), m.storage(161)), (18, 0));
}

struct FailOn {
    text: Rc<RefCell<Vec<u8>>>,
    marker: &'static [u8],
}
impl Write for FailOn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut text = self.text.borrow_mut();
        for &b in buf {
            text.push(b);
            if text.ends_with(self.marker) {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn output_failure_before_and_after_commit() {
    for (marker, committed) in [
        (b"* ION ENGINE START".as_slice(), false),
        (b"44-44".as_slice(), true),
    ] {
        let mut m = machine("1\n0\n");
        m.execute_source("@=65,3").unwrap();
        m.set_output(FailOn {
            text: Rc::new(RefCell::new(Vec::new())),
            marker,
        });
        assert_eq!(
            m.execute_source("warp=()"),
            Err(SourceError::Runtime(RuntimeError::OutputError))
        );
        if committed {
            assert_eq!(
                (
                    m.storage(160),
                    m.storage(161),
                    m.storage(170),
                    m.storage(98),
                    m.storage(65)
                ),
                (18, 3999, 0, 0, 2)
            );
        } else {
            assert_eq!(
                (
                    m.storage(160),
                    m.storage(161),
                    m.storage(170),
                    m.storage(98),
                    m.storage(65)
                ),
                (19, 4000, 1, 4, 3)
            );
        }
        assert_eq!(m.storage(70), 42);
    }
}
