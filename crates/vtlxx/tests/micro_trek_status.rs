use std::io::Cursor;
use vtlxx::Machine;

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const PREFIX: &[i16] = &[-1234, 0, 32767];

#[test]
fn reports_condition_priority_and_latest_owned_state_without_side_effects() {
    // (dock, enemies here, energy, days, coordinates, expected condition)
    let cases = [
        (1, 1, 999, 30, (1, 8, 8, 1), "DOCKED"),
        (0, 1, 999, 0, (8, 1, 1, 8), "RED"),
        (0, 0, 999, 30, (4, 5, 6, 7), "YELLOW"),
        (0, 0, 1000, 0, (8, 8, 1, 1), "GREEN"),
    ];
    for (dock, enemies, energy, days, (u, v, x, y), condition) in cases {
        let mut m = Machine::new();
        m.execute_source(SOURCE.split_once("; EC08_TEST_DEFINITIONS_END").unwrap().0)
            .unwrap();
        m.execute_source(&format!("@=160,{days} @=161,{energy} @=162,7 @=163,12 @=164,3 @=165,{u} @=166,{v} @=167,{x} @=168,{y} @=169,{enemies} @=170,{dock} @=192,12345")).unwrap();
        for (i, name) in ('A'..='Z').enumerate() {
            m.execute_source(&format!("{name}={}", i + 101)).unwrap();
        }
        m.set_input(Cursor::new(b"Z".to_vec()));
        for &value in PREFIX {
            m.push(value);
        }
        let before: Vec<_> = (0..240).map(|i| m.storage(i)).collect();
        let expected = format!(
            "*STATUS REPORT*\n\n*STARDATE : {}\n\n*CONDITION: {condition}\n*QUADRANT : ({u}{v})\n*SECTOR...: ({x}{y})\n*ENERGY...: {energy}\n\n*TORPEDOES: 7\n*KLINGON..: 12\n*BASE.....: 3\n",
            3230 - days
        );
        for count in 1..=2 {
            m.execute_source("status=()").unwrap();
            assert_eq!(m.output(), expected.repeat(count).as_bytes());
            assert_eq!(m.stack(), PREFIX);
            for i in 0..240 {
                assert_eq!(m.storage(i), before[i as usize], "cell {i}");
            }
            for i in 0..26 {
                assert_eq!(m.register(i), Some(i as i16 + 101), "register {i}");
            }
        }
        m.execute_source("A=$").unwrap();
        assert_eq!(m.register(0), Some(b'Z' as i16));
    }
}
