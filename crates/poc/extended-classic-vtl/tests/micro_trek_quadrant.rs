use std::collections::HashSet;
use vtlxx_poc_extended_classic_vtl::Machine;

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const MARKER: &str = "; EC08_TEST_DEFINITIONS_END";
const PREFIX: &[i16] = &[i16::MIN, 1234, -7, i16::MAX];

fn machine(seed: i16) -> Machine {
    let mut m = Machine::new();
    m.execute_source("I=0 *=I<240 [ @=I,I+2000 I=I+1 ] @=-1,-123")
        .unwrap();
    for i in 0..26 {
        m.execute_source(&format!("{}={}", char::from(b'A' + i), 3000 + i16::from(i)))
            .unwrap();
    }
    m.execute_source(&format!("@=192,{seed}")).unwrap();
    let mut markers = SOURCE.match_indices(MARKER).filter(|(i, _)| {
        (*i == 0 || SOURCE.as_bytes()[i - 1] == b'\n')
            && SOURCE
                .as_bytes()
                .get(i + MARKER.len())
                .is_none_or(|b| *b == b'\n')
    });
    let (end, _) = markers.next().expect("missing definitions delimiter");
    assert!(markers.next().is_none(), "duplicate definitions delimiter");
    m.execute_source(&SOURCE[..end]).unwrap();
    for &value in PREFIX {
        m.push(value);
    }
    m
}

fn draw(state: &mut i16) -> usize {
    *state = (i32::from(*state) * 25173 + 13849) as i16;
    state.rem_euclid(64) as usize
}

struct Expected {
    cells: [i16; 82],
    state: i16,
    attempts: Vec<usize>,
}

fn expected(seed: i16, content: i16, x: i16, y: i16) -> Expected {
    let mut cells = [0; 82];
    cells[((x - 1) * 8 + y - 1) as usize] = 4;
    let mut state = seed;
    let mut attempts = Vec::new();
    for (kind, count) in [
        (1, content / 100),
        (2, content % 100 / 10),
        (3, content % 10),
    ] {
        for slot in 0..count {
            let mut tries = 0;
            loop {
                let index = draw(&mut state);
                tries += 1;
                assert!(tries <= 15, "candidate bound");
                if cells[index] != 0 {
                    continue;
                }
                cells[index] = kind;
                if kind == 1 {
                    cells[64 + slot as usize] = 300;
                    cells[70 + slot as usize] = (index / 8 + 1) as i16;
                    cells[76 + slot as usize] = (index % 8 + 1) as i16;
                }
                attempts.push(tries);
                break;
            }
        }
    }
    Expected {
        cells,
        state,
        attempts,
    }
}

fn run(seed: i16, cell: i16, u: i16, v: i16, x: i16, y: i16) -> Expected {
    let mut m = machine(seed);
    m.execute_source(&format!(
        "U={u} V={v} X={x} Y={y} @={},{}",
        (u - 1) * 8 + v - 1,
        cell
    ))
    .unwrap();
    let before: Vec<_> = (0..=u16::MAX).map(|i| m.storage(i)).collect();
    let registers: Vec<_> = (0..26).map(|i| m.register(i)).collect();
    let exp = expected(seed, cell % 1000, x, y);
    m.execute_source("q=()").unwrap();
    assert_eq!(m.stack(), PREFIX);
    assert_eq!(m.register(13), Some(cell % 1000 / 100));
    assert!(m.output().is_empty());
    for i in 0..=u16::MAX {
        let value = match i {
            71..=152 => exp.cells[(i - 71) as usize],
            192 => exp.state,
            _ => before[i as usize],
        };
        assert_eq!(
            m.storage(i),
            value,
            "storage[{i}], seed {seed}, cell {cell}"
        );
    }
    for (i, value) in registers.iter().enumerate() {
        if !b"ABCHIJLMNPR".contains(&(b'A' + i as u8)) {
            assert_eq!(
                m.register(i),
                *value,
                "register {}",
                char::from(b'A' + i as u8)
            );
        }
    }
    exp
}

#[test]
fn placement_matches_reference_and_preserves_unowned_state() {
    for &(seed, cell, u, v, x, y) in &[
        (0, 0, 1, 1, 1, 1),
        (0, 1101, 8, 8, 8, 8),
        (-32768, 517, 1, 8, 1, 8),
        (1234, 1517, 8, 1, 8, 1),
        (42, 7, 4, 5, 4, 5),
        (-1, 110, 5, 4, 2, 7),
    ] {
        let exp = run(seed, cell, u, v, x, y);
        let content = cell % 1000;
        for (kind, count) in [
            (1, content / 100),
            (2, content % 100 / 10),
            (3, content % 10),
            (4, 1),
        ] {
            assert_eq!(
                exp.cells[..64].iter().filter(|&&c| c == kind).count(),
                count as usize
            );
        }
        let repeat = run(seed, cell, u, v, x, y);
        assert_eq!(exp.cells, repeat.cells);
        assert_eq!(exp.state, repeat.state);
        assert_eq!(exp.attempts, repeat.attempts);
    }
    // With seed 0 the first candidate is index 25: deliberately occupy it.
    let exp = run(0, 517, 1, 1, 4, 2);
    assert_eq!(exp.attempts[0], 2);
}

#[test]
fn canonical_rng_covers_all_sectors_and_endpoints() {
    for seed in [0, 1, -1, i16::MIN, i16::MAX] {
        let mut m = machine(seed);
        let mut state = seed;
        let mut seen = HashSet::new();
        for _ in 0..64 {
            let index = draw(&mut state);
            m.execute_source("r=64").unwrap();
            assert_eq!(m.pop().unwrap(), index as i16 + 1);
            assert_eq!(m.storage(192), state);
            assert_eq!(m.stack(), PREFIX);
            assert!(seen.insert(index));
        }
        assert_eq!(seen, (0..64).collect());
    }
    // Vary seed to make every sector the first candidate; the ship is elsewhere.
    for seed in 0..64 {
        let mut state = seed;
        let index = draw(&mut state);
        let ship = (index + 1) % 64;
        let exp = run(
            seed,
            100,
            1,
            1,
            (ship / 8 + 1) as i16,
            (ship % 8 + 1) as i16,
        );
        assert_eq!(exp.attempts, [1]);
        assert_eq!(exp.cells[index], 1);
        assert_eq!(exp.cells[70], (index / 8 + 1) as i16);
        assert_eq!(exp.cells[76], (index % 8 + 1) as i16);
    }
}
