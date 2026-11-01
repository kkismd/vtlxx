use vtlxx_poc_extended_classic_vtl::Machine;

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const MARKER: &str = "; EC08_TEST_DEFINITIONS_END";
const PREFIX: &[i16] = &[-32768, 1234, -7, 32767];

fn machine(seed: i16) -> Machine {
    let mut m = Machine::new();
    m.execute_source("I=0 *=I<240 [ @=I,I+2000 I=I+1 ]")
        .unwrap();
    m.execute_source(&format!("@=192,{seed}")).unwrap();
    let mut parts = SOURCE.split(MARKER);
    m.execute_source(parts.next().unwrap()).unwrap();
    assert!(parts.next().is_some() && parts.next().is_none());
    for &value in PREFIX {
        m.push(value);
    }
    m
}

fn draw(state: &mut i16, bound: i16) -> i16 {
    *state = (i32::from(*state) * 25173 + 13849) as i16;
    state.rem_euclid(bound)
}

#[derive(Debug, PartialEq)]
struct Mission {
    galaxy: [i16; 64],
    sector: [i16; 82],
    scalars: [i16; 11],
    state: i16,
    attempts: Vec<i16>,
}

fn expected(seed: i16) -> Mission {
    let mut state = seed;
    let parameter = 600 + draw(&mut state, 400);
    let mut attempts = Vec::new();
    let (galaxy, enemies, bases) = loop {
        let mut cells = [0; 64];
        let (mut enemies, mut bases) = (0, 0);
        for cell in &mut cells {
            let base = i16::from(draw(&mut state, 100) < 5);
            let value = draw(&mut state, parameter);
            let enemy = [209, 49, 24, 9, 2].iter().filter(|&&n| value < n).count() as i16;
            let stars = draw(&mut state, 8);
            *cell = enemy * 100 + base * 10 + stars;
            enemies += enemy;
            bases += base;
        }
        attempts.push(bases);
        if (1..=3).contains(&bases) {
            break (cells, enemies, bases);
        }
    };
    let u = draw(&mut state, 8) + 1;
    let v = draw(&mut state, 8) + 1;
    let (x, y) = (v, u);
    let content = galaxy[((u - 1) * 8 + v - 1) as usize];
    let n = content / 100;
    let mut sector = [0; 82];
    sector[((x - 1) * 8 + y - 1) as usize] = 4;
    for (kind, count) in [(1, n), (2, content % 100 / 10), (3, content % 10)] {
        for slot in 0..count as usize {
            loop {
                let index = draw(&mut state, 64) as usize;
                if sector[index] != 0 {
                    continue;
                }
                sector[index] = kind;
                if kind == 1 {
                    sector[64 + slot] = 300;
                    sector[70 + slot] = (index / 8 + 1) as i16;
                    sector[76 + slot] = (index % 8 + 1) as i16;
                }
                break;
            }
        }
    }
    Mission {
        galaxy,
        sector,
        scalars: [30, 4000, 10, enemies, bases, u, v, x, y, n, 0],
        state,
        attempts,
    }
}

fn check(m: &Machine, exp: &Mission) {
    assert_eq!(m.stack(), PREFIX);
    assert!(m.output().is_empty());
    for (i, &value) in exp.galaxy.iter().enumerate() {
        assert_eq!(m.storage(i as u16), value, "galaxy {i}");
    }
    for i in 64..=70 {
        assert_eq!(m.storage(i), 0);
    }
    for (i, &value) in exp.sector.iter().enumerate() {
        assert_eq!(m.storage(71 + i as u16), value, "sector/enemy {i}");
    }
    for (i, &value) in exp.scalars.iter().enumerate() {
        assert_eq!(m.storage(160 + i as u16), value, "scalar {i}");
        let register = b"DEFKBUVXYNO"[i] - b'A';
        assert_eq!(m.register(register as usize), Some(value));
    }
    assert_eq!(m.storage(192), exp.state);
    for i in (153..160).chain(171..192).chain(193..240) {
        assert_eq!(m.storage(i), i as i16 + 2000, "unowned {i}");
    }
}

#[test]
fn mission_and_replay_follow_the_same_prng_stream() {
    for seed in [0, -32766, -32752, -32501] {
        let first = expected(seed);
        let second = expected(first.state);
        let mut m = machine(seed);
        m.execute_source("i=()").unwrap();
        check(&m, &first);
        // Simulate damage and changed scalars before replay; never reseed.
        m.execute_source("I=64 *=I<=70 [ @=I,99 I=I+1 ] D=1 E=2 F=3 O=1")
            .unwrap();
        m.execute_source("i=()").unwrap();
        check(&m, &second);
        let mut repeat = machine(seed);
        repeat.execute_source("i=() i=()").unwrap();
        check(&repeat, &second);
    }
    assert_eq!(expected(-32766).attempts, [4, 2]);
    assert_eq!(expected(-32752).attempts, [0, 5, 2]);
}
