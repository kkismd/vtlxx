use std::io::{self, Write};
use vtlxx::{Machine, RuntimeError, SourceError};

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const PREFIX: &[i16] = &[-32768, 123, 32767];

fn machine(k: i16, energy: i16, days: i16, enemy_hp: i16, base: bool) -> Machine {
    let mut m = Machine::new();
    m.execute_source(SOURCE.split_once("; EC08_TEST_DEFINITIONS_END").unwrap().0)
        .unwrap();
    m.execute_source(&format!(
        "@=163,{k} @=161,{energy} @=160,{days} @=162,2 @=64,3 @=70,77 @=167,4 @=168,4 @=169,{} @=135,{enemy_hp} @=141,4 @=147,4 @=192,42",
        i16::from(enemy_hp > 0)
    )).unwrap();
    if base {
        m.execute_source("@=90,2").unwrap();
    }
    for &v in PREFIX {
        m.push(v);
    }
    m
}

fn run(m: &mut Machine, result: i16) {
    m.execute_source("endturn=()").unwrap();
    let mut expected = PREFIX.to_vec();
    expected.push(result);
    assert_eq!(m.stack(), expected);
}

#[test]
fn win_precedes_dock_and_attack_even_when_energy_and_days_are_exhausted() {
    for energy in [0, -1] {
        let mut m = machine(0, energy, 0, 300, true);
        let before: Vec<_> = (0..240).map(|i| m.storage(i)).collect();
        run(&mut m, 1);
        assert!(m.output().is_empty());
        assert_eq!((0..240).map(|i| m.storage(i)).collect::<Vec<_>>(), before);
    }
}

#[test]
fn result_boundaries_and_empty_quadrant() {
    for (energy, days, result) in [(0, 1, 2), (1, 0, 2), (1, 1, 0), (-1, 1, 2), (1, -1, 2)] {
        let mut m = machine(1, energy, days, 0, false);
        run(&mut m, result);
        assert_eq!(m.storage(192), 42);
        assert_eq!(m.storage(161), energy);
        assert_eq!(m.storage(170), 0);
        assert!(m.output().is_empty());
    }
}

#[test]
fn docking_resupplies_before_counterattack_and_protects_ship() {
    let mut m = machine(1, 0, 1, 300, true);
    run(&mut m, 0);
    assert_eq!(
        m.output(),
        b"DOCKED IN\n*KLINGON ATTACK\nSTARBASE PROTECT ENTERPRISE.\n"
    );
    assert_eq!(
        (
            m.storage(170),
            m.storage(161),
            m.storage(162),
            m.storage(64),
            m.storage(70)
        ),
        (1, 4000, 10, 0, 77)
    );
    assert_eq!((m.storage(135), m.storage(192)), (300, 42));
}

#[test]
fn actual_counterattack_can_cause_defeat() {
    let mut m = machine(1, 1, 1, 1, false);
    run(&mut m, 2);
    assert_eq!(m.storage(161), 0);
    assert_eq!(m.storage(135), 1);
    assert_eq!(m.storage(192), (42_i32 * 25173 + 13849) as i16);
    assert_eq!(
        m.output(),
        b"*KLINGON ATTACK\n1 UNITS HIT FROM KLINGON (44)\n"
    );
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
fn callee_output_errors_propagate_without_a_result() {
    for base in [false, true] {
        let mut m = machine(1, 1, 1, 300, base);
        m.set_output(Broken);
        assert_eq!(
            m.execute_source("endturn=()"),
            Err(SourceError::Runtime(RuntimeError::OutputError))
        );
        assert!(m.stack().starts_with(PREFIX));
        if base {
            assert_eq!(
                (
                    m.storage(170),
                    m.storage(161),
                    m.storage(162),
                    m.storage(64),
                    m.storage(70)
                ),
                (1, 4000, 10, 0, 77)
            );
        } else {
            assert_eq!(
                (m.storage(135), m.storage(161), m.storage(192)),
                (300, 1, 42)
            );
        }
    }
}
