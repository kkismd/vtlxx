use std::collections::HashSet;
use vtlxx_poc_extended_classic_vtl::Machine;

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const MARKER: &str = "; EC08_TEST_DEFINITIONS_END";
const PREFIX: &[i16] = &[i16::MIN, 1234, -7, i16::MAX];

fn definitions() -> &'static str {
    let mut markers = SOURCE.match_indices(MARKER).filter(|(i, _)| {
        (*i == 0 || SOURCE.as_bytes()[i - 1] == b'\n')
            && SOURCE
                .as_bytes()
                .get(i + MARKER.len())
                .is_none_or(|b| *b == b'\n')
    });
    let (end, _) = markers.next().expect("missing definitions delimiter");
    assert!(markers.next().is_none(), "duplicate definitions delimiter");
    &SOURCE[..end]
}

fn machine(seed: i16) -> Machine {
    let mut m = Machine::new();
    m.execute_source("I=0 *=I<240 [ @=I,I+2000 I=I+1 ] @=-1,-123")
        .unwrap();
    for index in 0..26 {
        m.execute_source(&format!(
            "{}={}",
            char::from(b'A' + index),
            3000 + i16::from(index)
        ))
        .unwrap();
    }
    m.execute_source(&format!("@=192,{seed}")).unwrap();
    m.execute_source(definitions()).unwrap();
    for &value in PREFIX {
        m.push(value);
    }
    m
}

fn draw(state: &mut i16, bound: i16) -> i16 {
    *state = (i32::from(*state) * 25173 + 13849) as i16;
    state.rem_euclid(bound)
}

struct Expected {
    cells: [i16; 64],
    enemies: i16,
    bases: i16,
    state: i16,
    y: i16,
    attempts: Vec<i16>,
    draws: HashSet<(char, i16)>,
}

fn expected(seed: i16) -> Expected {
    let mut state = seed;
    let y = 600 + draw(&mut state, 400);
    let mut attempts = Vec::new();
    let mut draws = HashSet::new();
    loop {
        let mut cells = [0; 64];
        let mut enemies = 0;
        let mut bases = 0;
        for cell in &mut cells {
            let v = draw(&mut state, 100);
            let u = draw(&mut state, y);
            let s = draw(&mut state, 8);
            draws.extend([('v', v), ('u', u), ('s', s)]);
            if u == y - 1 {
                draws.insert(('e', 0));
            }
            let b = i16::from(v < 5);
            let k = [209, 49, 24, 9, 2].iter().filter(|&&t| u < t).count() as i16;
            *cell = 100 * k + 10 * b + s;
            enemies += k;
            bases += b;
        }
        attempts.push(bases);
        if (1..=3).contains(&bases) {
            return Expected {
                cells,
                enemies,
                bases,
                state,
                y,
                attempts,
                draws,
            };
        }
    }
}

#[test]
fn canonical_prng_is_callable_and_advances_once() {
    let mut m = machine(0);
    for (bound, value, state) in [
        (1, 1, 13849),
        (2, 1, -16794),
        (3, 3, 31223),
        (6, 3, 17180),
        (8, 6, 13925),
        (600, 11, -4190),
        (999, 390, -13597),
        (32767, 31097, 31096),
    ] {
        m.execute_source(&format!("r={bound}")).unwrap();
        assert_eq!(m.pop().unwrap(), value);
        assert_eq!(m.stack(), PREFIX);
        assert_eq!(m.storage(192), state);
    }
}

#[test]
fn galaxy_draw_order_retries_thresholds_and_owned_state() {
    let seeds = [
        0, -32768, -32767, -32766, -32765, -32764, -32761, -32760, -32759, -32757, -32752, -32751,
        -32658, -32501,
    ];
    let mut seen = HashSet::new();
    let mut ys = HashSet::new();
    for seed in seeds {
        let expected = expected(seed);
        seen.extend(expected.draws.iter().copied());
        ys.insert(expected.y);
        let mut m = machine(seed);
        let storage: Vec<_> = (0..=u16::MAX).map(|i| m.storage(i)).collect();
        let registers: Vec<_> = (0..26).map(|i| m.register(i)).collect();
        m.execute_source("g=()").unwrap();
        assert_eq!(m.stack(), PREFIX, "seed {seed}");
        assert_eq!(m.storage(163), expected.enemies, "K seed {seed}");
        assert_eq!(m.storage(164), expected.bases, "B seed {seed}");
        assert_eq!(m.storage(192), expected.state, "PRNG seed {seed}");
        for (i, &cell) in expected.cells.iter().enumerate() {
            assert_eq!(m.storage(i as u16), cell, "seed {seed}, cell {i}");
            assert!((0..=517).contains(&cell));
        }
        assert_eq!(
            expected.cells.iter().map(|c| c / 100).sum::<i16>(),
            expected.enemies
        );
        assert_eq!(
            expected.cells.iter().map(|c| (c % 100) / 10).sum::<i16>(),
            expected.bases
        );
        for i in 64..=u16::MAX {
            if ![163, 164, 192].contains(&i) {
                assert_eq!(m.storage(i), storage[i as usize], "storage[{i}]");
            }
        }
        for (i, value) in registers.iter().enumerate() {
            if !b"ABCHIJLMT".contains(&(b'A' + i as u8)) {
                assert_eq!(
                    m.register(i),
                    *value,
                    "register {}",
                    char::from(b'A' + i as u8)
                );
            }
        }
        assert!(m.output().is_empty());
    }
    assert!(ys.contains(&600) && ys.contains(&999));
    for u in [0, 1, 2, 8, 9, 23, 24, 48, 49, 208, 209] {
        assert!(seen.contains(&('u', u)), "u={u}");
    }
    for v in [0, 4, 5, 99] {
        assert!(seen.contains(&('v', v)), "v={v}");
    }
    for s in [0, 7] {
        assert!(seen.contains(&('s', s)), "s={s}");
    }
    assert!(seen.contains(&('e', 0)));
    assert_eq!(expected(-32766).attempts, [4, 2]);
    assert_eq!(expected(-32752).attempts, [0, 5, 2]);
    assert_eq!(expected(-32501).attempts, [7, 1]);
    assert_eq!(expected(0).attempts, [3]);
}

#[test]
fn seed_zero_has_a_fixed_galaxy() {
    let expected_cells = [
        4, 103, 106, 105, 100, 7, 102, 1, 4, 13, 6, 5, 0, 7, 2, 101, 104, 3, 106, 5, 0, 7, 112, 1,
        4, 3, 6, 105, 0, 7, 102, 11, 104, 3, 106, 5, 0, 7, 2, 1, 4, 103, 6, 305, 200, 7, 2, 1, 104,
        103, 106, 5, 0, 7, 402, 1, 4, 3, 6, 5, 0, 7, 2, 1,
    ];
    for _ in 0..2 {
        let mut m = machine(0);
        m.execute_source("g=()").unwrap();
        assert_eq!(
            (0..64).map(|i| m.storage(i)).collect::<Vec<_>>(),
            expected_cells
        );
        assert_eq!(
            (m.storage(163), m.storage(164), m.storage(192)),
            (26, 3, -16807)
        );
    }
}
