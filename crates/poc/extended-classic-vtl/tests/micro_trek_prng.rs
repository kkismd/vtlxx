use vtlxx_poc_extended_classic_vtl::Machine;

const PRNG: &str = include_str!("fixtures/micro_trek/prng.vtl");
const PREFIX: &[i16] = &[i16::MIN, 1234, -7, i16::MAX];

fn next(state: i16, n: i16) -> (i16, i16) {
    let state = ((i32::from(state) * 25173 + 13849) as u16) as i16;
    let remainder = i32::from(state) % i32::from(n);
    let value = (remainder + if remainder < 0 { i32::from(n) } else { 0 } + 1) as i16;
    (state, value)
}

fn run(seed: i16, bounds: &[i16]) -> (Vec<i16>, i16) {
    let mut machine = Machine::new();
    machine
        .execute_source("I=0 *=I<240 [ @=I,I+2000 I=I+1 ] @=-1,-123")
        .unwrap();
    machine
        .execute_source(&format!("@=192,{seed} {PRNG}"))
        .unwrap();
    for &value in PREFIX {
        machine.push(value);
    }
    let storage: Vec<_> = (0..=u16::MAX).map(|i| machine.storage(i)).collect();
    let registers: Vec<_> = (0..26).map(|i| machine.register(i)).collect();
    let mut state = seed;
    let mut values = Vec::new();
    for &n in bounds {
        let (expected_state, expected_value) = next(state, n);
        machine.execute_source(&format!("r={n}")).unwrap();
        assert_eq!(machine.stack(), [PREFIX, &[expected_value]].concat());
        assert_eq!(machine.pop().unwrap(), expected_value);
        assert_eq!(machine.storage(192), expected_state);
        assert!((1..=n).contains(&expected_value));
        values.push(expected_value);
        state = expected_state;
    }
    for (index, expected) in storage.into_iter().enumerate() {
        if index != 192 {
            assert_eq!(machine.storage(index as u16), expected, "storage[{index}]");
        }
    }
    for (index, expected) in registers.into_iter().enumerate() {
        if index > 1 {
            assert_eq!(machine.register(index), expected, "register {index}");
        }
    }
    assert!(machine.output().is_empty());
    (values, state)
}

#[test]
fn fixed_stream_and_single_updates() {
    let bounds = [1, 2, 3, 6, 8, 600, 999, 32767];
    let states = [13849, -16794, 31223, 17180, 13925, -4190, -13597, 31096];
    let values = [1, 1, 3, 3, 6, 11, 390, 31097];
    let mut state = 0;
    for ((n, expected_state), expected_value) in bounds.into_iter().zip(states).zip(values) {
        assert_eq!(next(state, n), (expected_state, expected_value));
        state = expected_state;
    }
    assert_eq!(run(0, &bounds), (values.to_vec(), 31096));
}

#[test]
fn seeds_callers_and_replay_continue_one_stream() {
    let bounds = [2, 8, 6, 3, 1, 600, 999, 32767, 8, 2];
    for seed in [0, 1, -1, 32767, -32768] {
        let full = run(seed, &bounds);
        assert_eq!(full, run(seed, &bounds));
        let first_mission = run(seed, &bounds[..4]);
        let replay = run(first_mission.1, &bounds[4..]);
        assert_eq!([first_mission.0, replay.0].concat(), full.0);
        assert_eq!(replay.1, full.1);
        assert!((0..=1).contains(&(full.0[0] - 1)));
        assert!((1..=8).contains(&full.0[1]));
        assert!((1..=6).contains(&full.0[2]));
        assert!((1..=3).contains(&full.0[3]));
    }
}

#[test]
fn separate_source_callers_share_state_across_replay() {
    let mut machine = Machine::new();
    machine
        .execute_source(&format!("@=192,0 {PRNG} &=p [ r=2 r=8 ] &=q [ r=6 r=3 ]"))
        .unwrap();
    for &value in PREFIX {
        machine.push(value);
    }
    machine.execute_source("p=() q=()").unwrap();
    let mission_end = machine.storage(192);
    machine.execute_source("q=() p=()").unwrap();
    let (first, state) = run(0, &[2, 8, 6, 3]);
    let (second, final_state) = run(state, &[6, 3, 2, 8]);
    assert_eq!(mission_end, state);
    assert_eq!(
        machine.stack(),
        [PREFIX, first.as_slice(), second.as_slice()].concat()
    );
    assert_eq!(machine.storage(192), final_state);
}
