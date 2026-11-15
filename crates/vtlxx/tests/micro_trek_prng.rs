use vtlxx::Machine;

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const MARKER: &str = "; EC08_TEST_DEFINITIONS_END";
const PREFIX: &[i16] = &[i16::MIN, 1234, -7, i16::MAX];

fn x16(seed: u16) -> u16 {
    let mut state = seed;
    state ^= state.wrapping_shl(7);
    state ^= state >> 9;
    state ^= state.wrapping_shl(8);
    state
}

fn machine(seed: i16) -> Machine {
    let mut machine = Machine::new();
    machine.set_entropy_source(move || Ok(seed));
    let definitions = SOURCE.split(MARKER).next().unwrap();
    machine.execute_source(definitions).unwrap();
    for value in PREFIX {
        machine.push(*value);
    }
    machine
}

#[test]
fn bounded_random_uses_one_builtin_draw_and_does_not_own_storage_192() {
    let seed = -21279_i16; // xorshift seed 0xACE1
    let mut m = machine(seed);
    m.execute_source("@=192,2468").unwrap();
    let storage: Vec<_> = (0..=255).map(|address| m.storage(address)).collect();
    let registers: Vec<_> = (2..26).map(|register| m.register(register)).collect();
    let mut expected_state = seed as u16;
    for n in [1_i16, 2, 3, 6, 8, 600, 999, 32767] {
        expected_state = x16(expected_state);
        let expected = (expected_state >> 1) as i16 % n + 1;
        m.execute_source(&format!("random={n}")).unwrap();
        assert_eq!(m.pop().unwrap(), expected);
        assert_eq!(m.stack(), PREFIX);
        assert_eq!(m.storage(192), 2468);
    }
    for (address, value) in storage.into_iter().enumerate() {
        assert_eq!(m.storage(address as u16), value, "storage[{address}]");
    }
    for (offset, value) in registers.into_iter().enumerate() {
        assert_eq!(m.register(offset + 2), value, "register {}", offset + 2);
    }
    assert!(m.output().is_empty());
}

#[test]
fn repeated_calls_and_source_executions_continue_the_machine_stream() {
    let mut m = machine(1);
    m.execute_source("&=p [ random=8 ] &=q [ random=6 ] p=() q=()")
        .unwrap();
    let mut state = x16(1);
    let a = (state >> 1) as i16 % 8 + 1;
    state = x16(state);
    let b = (state >> 1) as i16 % 6 + 1;
    m.execute_source("random=8").unwrap();
    state = x16(state);
    let c = (state >> 1) as i16 % 8 + 1;
    assert_eq!(m.stack(), [PREFIX, &[a, b, c]].concat());
}

#[test]
fn canonical_source_contains_no_game_owned_prng_state() {
    assert!(!SOURCE.contains("PRNG_STATE"));
    assert!(!SOURCE.contains("25173"));
    assert!(!SOURCE.contains("13849"));
    assert!(!SOURCE.contains("INVALID SEED"));
}
