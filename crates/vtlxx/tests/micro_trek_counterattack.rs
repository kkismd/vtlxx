use std::io::{self, Write};
use vtlxx::{Machine, RuntimeError, SourceError};

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const PREFIX: &[i16] = &[-32768, 1234, -7, 32767];
const NAMES: [&str; 6] = [
    "*SHORT RANGE SENSOR*",
    "*COMPUTER*",
    "*LONG RANGE SENSOR*",
    "*PHASER*",
    "*WARP*",
    "*TORPEDOES*",
];

fn machine(seed: i16, hp: [i16; 6], docked: i16) -> Machine {
    let mut m = Machine::new();
    m.execute_source(SOURCE.split_once("; EC08_TEST_DEFINITIONS_END").unwrap().0)
        .unwrap();
    m.execute_source(&format!(
        "@=192,{seed} @=169,{} @=170,{docked} @=161,4000 @=167,1 @=168,1",
        hp.iter().filter(|&&h| h > 0).count()
    ))
    .unwrap();
    for (index, &h) in hp.iter().enumerate() {
        m.execute_source(&format!(
            "@={},{} @={},1 @={},1",
            135 + index,
            h,
            141 + index,
            147 + index
        ))
        .unwrap();
    }
    for &v in PREFIX {
        m.push(v);
    }
    m
}

fn draw(s: &mut i16, bound: i16) -> i16 {
    *s = (i32::from(*s) * 25173 + 13849) as i16;
    s.rem_euclid(bound) + 1
}

#[test]
fn empty_and_docked_do_not_attack() {
    for (hp, docked, output) in [
        ([0; 6], 0, ""),
        (
            [300, 0, 0, 0, 0, 0],
            1,
            "*KLINGON ATTACK\nSTARBASE PROTECT ENTERPRISE.\n",
        ),
    ] {
        let mut m = machine(42, hp, docked);
        m.execute_source("enemyatk=()").unwrap();
        assert_eq!(m.output(), output.as_bytes());
        assert_eq!(m.storage(192), 42);
        assert_eq!(m.storage(161), 4000);
        assert_eq!(m.storage(135), hp[0]);
        assert_eq!(m.stack(), PREFIX);
    }
}

#[test]
fn attack_slots_and_threshold_preserve_state_and_draw_order() {
    for (seed, hp, expected_total) in [
        (-32376, [300, 0, 300, 0, 0, 300], Some(199)),
        (-32753, [300, 0, 300, 0, 0, 300], Some(200)),
        (42, [1, 2, 0, 0, 0, 300], None),
        (51, [300; 6], None),
    ] {
        let mut m = machine(seed, hp, 0);
        let before: Vec<_> = (0..240).map(|i| m.storage(i)).collect();
        let mut state = seed;
        let mut expected = before.clone();
        let mut total = 0;
        let mut output = String::from("*KLINGON ATTACK\n");
        for (slot, &h) in hp.iter().enumerate() {
            if h == 0 {
                continue;
            }
            let power = (draw(&mut state, h) - 1) / 2;
            expected[135 + slot] = h - power;
            let hit = power + 1; // dx=dy=0, denominator 30
            total += hit;
            output.push_str(&format!("{hit} UNITS HIT FROM KLINGON (11)\n"));
        }
        if let Some(v) = expected_total {
            assert_eq!(total, v);
        }
        expected[161] -= total;
        if total >= 200 {
            output.push_str("## SHIELD SCREEN FELL DOWN ##\n");
            let device = draw(&mut state, 6) as usize;
            let amount = draw(&mut state, 3);
            expected[63 + device] += amount;
            output.push_str(&format!(
                "{}DAMAGED!\n{}\n YEARS ESTIMATED FOR REPAIR.\n",
                NAMES[device - 1],
                expected[63 + device]
            ));
        }
        expected[192] = state;
        m.execute_source("enemyatk=()").unwrap();
        assert_eq!(m.output(), output.as_bytes());
        assert_eq!(m.stack(), PREFIX);
        for (i, value) in expected.iter().enumerate() {
            assert_eq!(m.storage(i as u16), *value, "storage {i}");
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
fn output_error_is_fatal() {
    let mut m = machine(42, [300, 0, 0, 0, 0, 0], 0);
    m.set_output(Broken);
    assert!(matches!(
        m.execute_source("enemyatk=()"),
        Err(SourceError::Runtime(RuntimeError::OutputError))
    ));
    assert_eq!(m.storage(192), 42);
    assert_eq!(m.storage(135), 300);
    assert_eq!(m.storage(161), 4000);
    assert!(m.stack().starts_with(PREFIX));
}

#[test]
fn random_endpoints_and_distance_are_applied_per_enemy() {
    for h in [1, 2, 300] {
        for u in [0, h - 1] {
            let seed = (i16::MIN..=i16::MAX)
                .find(|&seed| {
                    let mut s = seed;
                    draw(&mut s, h) - 1 == u
                })
                .unwrap();
            let mut m = machine(seed, [0, 0, 0, 0, 0, h], 0);
            m.execute_source("@=146,8 @=152,8").unwrap();
            let power = u / 2;
            let hit = (30 * i32::from(power) / 128 + 1) as i16;
            m.execute_source("enemyatk=()").unwrap();
            assert_eq!(m.storage(140), h - power);
            assert!(m.storage(140) > 0);
            assert_eq!(m.storage(161), 4000 - hit);
            assert_eq!(
                m.output(),
                format!("*KLINGON ATTACK\n{hit} UNITS HIT FROM KLINGON (88)\n").as_bytes()
            );
            assert_eq!(m.stack(), PREFIX);
        }
    }
}
