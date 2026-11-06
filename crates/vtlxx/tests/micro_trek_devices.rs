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

fn machine(seed: i16) -> Machine {
    let mut m = Machine::new();
    m.execute_source("I=0 *=I<240 [ @=I,I+2000 I=I+1 ]")
        .unwrap();
    m.execute_source(&format!("@=192,{seed} I=64 *=I<70 [ @=I,0 I=I+1 ]"))
        .unwrap();
    let mut parts = SOURCE.split("; EC08_TEST_DEFINITIONS_END");
    m.execute_source(parts.next().unwrap()).unwrap();
    assert!(parts.next().is_some() && parts.next().is_none());
    m.execute_source("C=103 D=104 E=105 F=106 G=107 H=108 I=109 J=110 K=111 L=112 M=113 N=114 O=115 P=116 Q=117 R=118 S=119 T=120 U=121 V=122 W=123 X=124 Y=125 Z=126").unwrap();
    for &value in PREFIX {
        m.push(value);
    }
    m
}

fn expected_output(id: usize, years: i16) -> String {
    let name = NAMES[id - 1];
    if years == 0 {
        format!("{name}\n")
    } else {
        format!("{name}DAMAGED!\n{years}\n YEARS ESTIMATED FOR REPAIR.\n")
    }
}

fn draw(state: &mut i16, bound: i16) -> i16 {
    *state = (i32::from(*state) * 25173 + 13849) as i16;
    state.rem_euclid(bound) + 1
}

fn check_unchanged(m: &Machine, seed: i16, devices: &[i16; 6]) {
    assert_eq!(m.stack(), PREFIX);
    for i in 2..26 {
        assert_eq!(m.register(i), Some(i as i16 + 101), "register {i}");
    }
    for i in 0..240 {
        let expected = if (64..70).contains(&i) {
            devices[(i - 64) as usize]
        } else if i == 192 {
            seed
        } else {
            i as i16 + 2000
        };
        assert_eq!(m.storage(i), expected, "storage {i}");
    }
}

#[test]
fn device_display_all_names_and_states() {
    for id in 1..=6 {
        for years in [0, 1, 3] {
            let mut m = machine(42);
            m.execute_source(&format!("@={},{}", 63 + id, years))
                .unwrap();
            let mut devices = [0; 6];
            devices[id - 1] = years;
            for count in 1..=2 {
                m.execute_source(&format!("devinfo={id}")).unwrap();
                check_unchanged(&m, 42, &devices);
                assert_eq!(
                    m.output(),
                    expected_output(id, years).repeat(count).as_bytes()
                );
            }
        }
    }
}

#[test]
fn damage_draw_order_and_accumulation() {
    for id in 1..=6 {
        for increment in [1, 3] {
            let seed = (i16::MIN..=i16::MAX)
                .find(|&seed| {
                    let mut s = seed;
                    draw(&mut s, 6) == id && draw(&mut s, 3) == increment
                })
                .unwrap();
            let mut m = machine(seed);
            let mut state = seed;
            let mut devices = [0; 6];
            let mut output = String::new();
            for _ in 0..2 {
                let device = draw(&mut state, 6) as usize;
                let amount = draw(&mut state, 3);
                devices[device - 1] += amount;
                output.push_str(&expected_output(device, devices[device - 1]));
                m.execute_source("devdmg=()").unwrap();
                check_unchanged(&m, state, &devices);
                assert_eq!(m.output(), output.as_bytes());
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
fn output_error_preserves_committed_damage_and_draws() {
    let seed = 42;
    let mut state = seed;
    let device = draw(&mut state, 6) as usize;
    let amount = draw(&mut state, 3);
    let mut m = machine(seed);
    m.set_output(Broken);
    assert_eq!(
        m.execute_source("devdmg=()"),
        Err(SourceError::Runtime(RuntimeError::OutputError))
    );
    assert_eq!(m.storage(192), state);
    for id in 1..=6 {
        assert_eq!(
            m.storage(63 + id as u16),
            if id == device { amount } else { 0 }
        );
    }
    assert_eq!(m.storage(70), 2070);
}

#[test]
fn negative_device_state_is_not_displayed_as_healthy() {
    let mut m = machine(42);
    m.execute_source("@=64,-1 devinfo=1").unwrap();
    assert_ne!(m.output(), expected_output(1, 0).as_bytes());
}
