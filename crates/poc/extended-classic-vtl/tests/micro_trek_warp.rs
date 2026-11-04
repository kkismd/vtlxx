use std::io::{self, Write};
use vtlxx_poc_extended_classic_vtl::{Machine, RuntimeError, SourceError};

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const PREFIX: &[i16] = &[-32768, 123, 32767];

fn machine(u: i16, v: i16, x: i16, y: i16, seed: i16) -> Machine {
    let mut m = Machine::new();
    let (definitions, _) = SOURCE.split_once("; EC08_TEST_DEFINITIONS_END").unwrap();
    m.execute_source(definitions).unwrap();
    m.execute_source(&format!(
        "@=165,{u} @=166,{v} @=167,{x} @=168,{y} @=160,19 @=161,1234 @=162,7 @=192,{seed}"
    ))
    .unwrap();
    for &cell in PREFIX {
        m.push(cell);
    }
    m
}

fn call(m: &mut Machine, distance: i16, dx: i16, dy: i16) {
    m.execute_source(&format!("~={distance} ~={dx} ~={dy} x=()"))
        .unwrap();
    assert_eq!(m.stack(), PREFIX);
    assert_eq!(
        (m.storage(160), m.storage(161), m.storage(162)),
        (19, 1234, 7)
    );
    assert_eq!((71..135).filter(|&i| m.storage(i) == 4).count(), 1);
}

#[test]
fn straight_diagonal_and_distance_cutoff() {
    for (distance, dx, dy, expected, x, y) in [
        (
            2,
            -45,
            0,
            "44-44\n44-34\nNOW: QUADRANT(44) SECTOR(24)\n",
            2,
            4,
        ),
        (2, -45, 45, "44-44\nNOW: QUADRANT(44) SECTOR(35)\n", 3, 5),
        (
            2,
            -45,
            35,
            "44-44\n44-35\nNOW: QUADRANT(44) SECTOR(26)\n",
            2,
            6,
        ),
        (
            5,
            -45,
            43,
            "44-44\n44-35\n44-26\nNOW: QUADRANT(44) SECTOR(17)\n",
            1,
            7,
        ),
    ] {
        let mut m = machine(4, 4, 4, 4, 0);
        call(&mut m, distance, dx, dy);
        assert_eq!((m.storage(167), m.storage(168)), (x, y));
        assert_eq!(m.output(), expected.as_bytes());
        assert_eq!(m.storage(192), 13849);
    }
}

#[test]
fn collision_keeps_previous_position_and_then_draws_storm() {
    let mut m = machine(4, 4, 4, 4, 0);
    m.execute_source("@=90,3").unwrap();
    call(&mut m, 1, -45, 0);
    assert_eq!((m.storage(167), m.storage(168), m.storage(90)), (4, 4, 3));
    let out = String::from_utf8(m.output().to_vec()).unwrap();
    assert!(out.starts_with("44-44\n*** EMERGENCY STOP ***\nSPOCK : TO ERR IS HUMAN !\n"));
    assert!(out.ends_with("NOW: QUADRANT(44) SECTOR(44)\n"));
}

#[test]
fn crossing_rebuilds_only_after_storm_and_outside_relocates() {
    let mut m = machine(4, 4, 1, 4, 0);
    call(&mut m, 1, -45, 0);
    assert_eq!(
        (
            m.storage(165),
            m.storage(166),
            m.storage(167),
            m.storage(168)
        ),
        (3, 4, 8, 4)
    );
    assert_eq!(
        m.output(),
        b"44-14\n*WARP ENGINE START\nNOW: QUADRANT(34) SECTOR(84)\n"
    );
    assert_eq!(m.storage(192), 13849);
    let mut m = machine(1, 4, 1, 4, 0);
    call(&mut m, 1, -45, 0);
    assert!((1..=8).contains(&m.storage(165)) && (1..=8).contains(&m.storage(166)));
    assert_eq!(
        (m.storage(167), m.storage(168)),
        (m.storage(166), m.storage(165))
    );
    assert_eq!(m.storage(192), 17180); // damage twice, then U/V; no storm
    let out = String::from_utf8(m.output().to_vec()).unwrap();
    assert!(out.contains(" OUTSIDE THE GARAXY !!\nON BOARD COMPUTER TAKES OVER,\n"));
    assert!(!out.contains("SPACE STORM"));
}

#[test]
fn old_quadrant_obstacles_are_ignored_after_crossing() {
    let mut m = machine(4, 4, 8, 4, 0);
    m.execute_source("@=74,3 @=82,3").unwrap();
    call(&mut m, 2, 45, 0);
    assert_eq!(
        (
            m.storage(165),
            m.storage(166),
            m.storage(167),
            m.storage(168)
        ),
        (5, 4, 2, 4)
    );
    assert_eq!(m.storage(192), 13849); // only storm; final empty q draws zero
    assert!(
        !String::from_utf8(m.output().to_vec())
            .unwrap()
            .contains("EMERGENCY")
    );
}

#[test]
fn crossing_storm_damage_precedes_final_quadrant_draws() {
    let mut m = machine(4, 4, 1, 4, 1);
    m.execute_source("@=19,101").unwrap();
    call(&mut m, 1, -45, 0);
    assert_eq!(m.storage(67), 1); // draw 2 selects PHASER; draw 3 adds one
    assert_eq!(m.storage(192), 3882); // storm, damage twice, then q twice
    assert_eq!(m.storage(169), 1);
    assert_eq!((m.storage(141), m.storage(147)), (2, 6));
    let out = String::from_utf8(m.output().to_vec()).unwrap();
    assert_eq!(
        out,
        "44-14\n*WARP ENGINE START\n## SPACE STORM ##\n*PHASER*DAMAGED!\n1\n YEARS ESTIMATED FOR REPAIR.\nNOW: QUADRANT(34) SECTOR(84)\n"
    );
}

#[test]
fn all_cardinal_diagonal_directions_and_maximum_distance_finish() {
    for (dx, dy) in [
        (-45, 0),
        (-45, 45),
        (0, 45),
        (45, 45),
        (45, 0),
        (45, -45),
        (0, -45),
        (-45, -45),
    ] {
        for distance in [1, 89] {
            let mut m = machine(4, 4, 4, 4, 0);
            call(&mut m, distance, dx, dy);
            for i in [165, 166, 167, 168] {
                assert!((1..=8).contains(&m.storage(i)));
            }
        }
    }
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
fn output_failure_propagates() {
    let mut m = machine(4, 4, 4, 4, 0);
    m.set_output(Broken);
    assert_eq!(
        m.execute_source("~=1 ~=-45 ~=0 x=()"),
        Err(SourceError::Runtime(RuntimeError::OutputError))
    );
    assert!(m.stack().starts_with(PREFIX));
}
