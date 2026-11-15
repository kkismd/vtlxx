use std::collections::HashSet;
use vtlxx::Machine;

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
    m.set_entropy_source(move || Ok(seed));
    m.execute_source(definitions()).unwrap();
    for &value in PREFIX {
        m.push(value);
    }
    m
}

fn x16(state: i16) -> i16 {
    let mut value = state as u16;
    value ^= value.wrapping_shl(7);
    value ^= value >> 9;
    value ^= value.wrapping_shl(8);
    value as i16
}

fn draw(state: &mut i16, bound: i16) -> i16 {
    *state = x16(*state);
    (((*state as u16) >> 1) % bound as u16) as i16 + 1
}

struct Expected {
    cells: [i16; 64],
    enemies: i16,
    bases: i16,
    y: i16,
    attempts: Vec<i16>,
    draws: HashSet<(char, i16)>,
}

fn expected(seed: i16) -> Expected {
    let mut state = if seed == 0 { -21279 } else { seed };
    let y = 599 + draw(&mut state, 400);
    let mut attempts = Vec::new();
    let mut draws = HashSet::new();
    loop {
        let mut cells = [0; 64];
        let mut enemies = 0;
        let mut bases = 0;
        for cell in &mut cells {
            let v = draw(&mut state, 100) - 1;
            let u = draw(&mut state, y) - 1;
            let s = draw(&mut state, 8) - 1;
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
    let mut state = -21279_i16;
    for bound in [1, 2, 3, 6, 8, 600, 999, 32767] {
        let value = draw(&mut state, bound);
        m.execute_source(&format!("random={bound}")).unwrap();
        assert_eq!(m.pop().unwrap(), value);
        assert_eq!(m.stack(), PREFIX);
        assert_eq!(m.storage(192), 2192);
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
        m.execute_source("gengal=()").unwrap();
        assert_eq!(m.stack(), PREFIX, "seed {seed}");
        assert_eq!(m.storage(163), expected.enemies, "K seed {seed}");
        assert_eq!(m.storage(164), expected.bases, "B seed {seed}");
        assert_eq!(m.storage(192), 2192, "unowned storage[192], seed {seed}");
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
    assert!(ys.len() > 1 && ys.iter().all(|&y| (600..=999).contains(&y)));
    assert!(seen.iter().all(|(kind, value)| match kind {
        'u' => (0..=998).contains(value),
        'v' => (0..100).contains(value),
        's' => (0..8).contains(value),
        'e' => *value == 0,
        _ => false,
    }));
    assert!(
        seeds
            .iter()
            .all(|&seed| expected(seed).attempts.len() <= 11)
    );
    assert!(seeds.iter().any(|&seed| expected(seed).attempts.len() > 1));
}

#[test]
fn seed_zero_has_a_reproducible_legal_galaxy() {
    let mut expected_cells = None;
    for _ in 0..2 {
        let mut m = machine(0);
        m.execute_source("gengal=()").unwrap();
        let cells: Vec<_> = (0..64).map(|i| m.storage(i)).collect();
        assert!(cells.iter().all(|cell| (0..=517).contains(cell)));
        assert!((1..=3).contains(&m.storage(164)));
        assert_eq!(m.storage(163), cells.iter().map(|cell| cell / 100).sum());
        assert_eq!(m.storage(192), 2192);
        if let Some(expected) = &expected_cells {
            assert_eq!(&cells, expected);
        }
        expected_cells = Some(cells);
    }
}
