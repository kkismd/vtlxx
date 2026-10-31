use vtlxx_poc_extended_classic_vtl::Machine;

const ENCODE: &str = include_str!("fixtures/micro_trek/galaxy_encode.vtl");
const DECODE: &str = include_str!("fixtures/micro_trek/galaxy_decode.vtl");
const OBSERVE: &str = include_str!("fixtures/micro_trek/galaxy_observe.vtl");
const REMOVE_ENEMY: &str = include_str!("fixtures/micro_trek/galaxy_remove_enemy.vtl");
const REMOVE_BASE: &str = include_str!("fixtures/micro_trek/galaxy_remove_base.vtl");
const REMOVE_STAR: &str = include_str!("fixtures/micro_trek/galaxy_remove_star.vtl");
const PREFIX: &[i16] = &[i16::MIN, 1234, -7, i16::MAX];

fn machine() -> Machine {
    let mut machine = Machine::new();
    // Distinct sentinels in galaxy, devices, sector, enemies, scalar state,
    // PRNG[192] and scratch. Also exercise the high end of indexed storage.
    machine
        .execute_source("I=0 *=I<240 [ @=I,I+2000 I=I+1 ] @=-1,-123 ?=42")
        .unwrap();
    for index in 0..26 {
        machine
            .execute_source(&format!(
                "{}={}",
                char::from(b'A' + index),
                3000 + i16::from(index)
            ))
            .unwrap();
    }
    for &value in PREFIX {
        machine.push(value);
    }
    machine
}

fn set_cell(machine: &mut Machine, index: i16, stored: i16) {
    machine
        .execute_source(&format!("@={index},{stored}"))
        .unwrap();
}

// Check the entire storage, not just the adjacent cells or known game state.
// Register changes are allowed only in the documented caller-saved set.
fn run(
    machine: &mut Machine,
    source: &str,
    args: &[i16],
    results: &[i16],
    write: Option<(i16, i16)>,
    scratch: &str,
) {
    let mut storage: Vec<_> = (0..=u16::MAX).map(|i| machine.storage(i)).collect();
    if let Some((index, value)) = write {
        storage[index as usize] = value;
    }
    let registers: Vec<_> = (0..26).map(|i| machine.register(i)).collect();
    let output = machine.output().to_vec();
    assert_eq!(machine.stack(), PREFIX);
    for &arg in args {
        machine.push(arg);
    }
    machine.execute_source(source).unwrap();
    assert_eq!(machine.stack(), [PREFIX, results].concat());
    for _ in results {
        machine.pop().unwrap();
    }
    assert_eq!(machine.output(), output);
    for (index, expected) in storage.into_iter().enumerate() {
        assert_eq!(machine.storage(index as u16), expected, "storage[{index}]");
    }
    for (index, expected) in registers.into_iter().enumerate() {
        if !scratch.as_bytes().contains(&(b'A' + index as u8)) {
            assert_eq!(machine.register(index), expected, "register {index}");
        }
    }
}

#[test]
fn all_legal_cells_roundtrip_with_both_observation_states() {
    let mut machine = machine();
    for index in [0, 63] {
        for k in 0..=5 {
            for b in 0..=1 {
                for s in 0..=7 {
                    for observed in 0..=1 {
                        let stored = 100 * k + 10 * b + s + 1000 * observed;
                        run(
                            &mut machine,
                            ENCODE,
                            &[index, k, b, s, observed],
                            &[],
                            Some((index, stored)),
                            "ACDEI",
                        );
                        run(
                            &mut machine,
                            DECODE,
                            &[index],
                            &[k, b, s, observed],
                            None,
                            "ACDEFI",
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn observation_and_guarded_removal_preserve_other_state_for_all_legal_cells() {
    let mut machine = machine();
    for index in [0, 63] {
        for k in 0..=5 {
            for b in 0..=1 {
                for s in 0..=7 {
                    for observed in 0..=1 {
                        let base = 100 * k + 10 * b + s;
                        let stored = base + 1000 * observed;
                        set_cell(&mut machine, index, stored);
                        for _ in 0..2 {
                            run(
                                &mut machine,
                                OBSERVE,
                                &[index],
                                &[],
                                Some((index, base + 1000)),
                                "I",
                            );
                        }
                        for (source, count, amount) in [
                            (REMOVE_ENEMY, k, 100),
                            (REMOVE_BASE, b, 10),
                            (REMOVE_STAR, s, 1),
                        ] {
                            set_cell(&mut machine, index, stored);
                            // Zero count is a caller precondition failure: no write,
                            // including observed zero (1000), with no new runtime error.
                            let expected = stored - if count > 0 { amount } else { 0 };
                            run(
                                &mut machine,
                                source,
                                &[index],
                                &[],
                                Some((index, expected)),
                                "FI",
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn every_galaxy_address_supports_sequential_local_updates() {
    let mut machine = machine();
    for index in 0..64 {
        run(
            &mut machine,
            ENCODE,
            &[index, 1, 1, 1, 0],
            &[],
            Some((index, 111)),
            "ACDEI",
        );
        for (source, expected, scratch) in [
            (OBSERVE, 1111, "I"),
            (REMOVE_ENEMY, 1011, "FI"),
            (REMOVE_BASE, 1001, "FI"),
            (REMOVE_STAR, 1000, "FI"),
            (OBSERVE, 1000, "I"),
        ] {
            run(
                &mut machine,
                source,
                &[index],
                &[],
                Some((index, expected)),
                scratch,
            );
        }
        run(
            &mut machine,
            DECODE,
            &[index],
            &[0, 0, 0, 1],
            None,
            "ACDEFI",
        );
    }
}
