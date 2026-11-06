use vtlxx::Machine;

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const PREFIX: &[i16] = &[-1234, 0, 32767];

fn machine(damage: i16) -> Machine {
    let mut m = Machine::new();
    m.execute_source(SOURCE.split_once("; EC08_TEST_DEFINITIONS_END").unwrap().0)
        .unwrap();
    for i in 0..64 {
        // Every kind appears, including both endpoints; one ship at (8,8).
        let kind = if i == 63 { 4 } else { i % 4 };
        m.execute_source(&format!("@={},{}", 71 + i, kind)).unwrap();
    }
    m.execute_source(&format!(
        "@=165,3 @=166,6 @=167,8 @=168,8 @=64,{damage} @=192,12345"
    ))
    .unwrap();
    for (i, name) in ('A'..='Z').enumerate() {
        m.execute_source(&format!("{name}={}", i + 101)).unwrap();
    }
    for &value in PREFIX {
        m.push(value);
    }
    m
}

#[test]
fn healthy_sensor_prints_all_cells_without_mutating_state() {
    let mut m = machine(0);
    let before: Vec<_> = (0..240).map(|i| m.storage(i)).collect();
    let mut expected = String::from("NOW: QUADRANT(36) SECTOR(88)\n*SHORT RANGE SENSOR*\n");
    let symbols = ['.', 'K', 'B', '*'];
    for row in 0..8 {
        expected.push_str(&(row + 1).to_string());
        for col in 0..8 {
            let i = row * 8 + col;
            expected.push(' ');
            expected.push(if i == 63 { 'E' } else { symbols[i % 4] });
        }
        expected.push('\n');
    }
    expected.push_str("  1 2 3 4 5 6 7 8\n");
    for count in 1..=2 {
        m.execute_source("shortmap=()").unwrap();
        assert_eq!(m.stack(), PREFIX);
        assert_eq!(m.output(), expected.repeat(count).as_bytes());
        for i in 0..26 {
            if ![0, 1, 8, 9, 10].contains(&i) {
                assert_eq!(m.register(i), Some(i as i16 + 101), "register {i}");
            }
        }
        for i in 0..240 {
            assert_eq!(m.storage(i), before[i as usize], "cell {i}");
        }
    }
}

#[test]
fn damaged_sensor_displays_status_without_map_or_state_changes() {
    for damage in [1, 3] {
        let mut m = machine(damage);
        let before: Vec<_> = (0..240).map(|i| m.storage(i)).collect();
        m.execute_source("shortmap=()").unwrap();
        assert_eq!(m.stack(), PREFIX);
        for i in 0..26 {
            if ![0, 1].contains(&i) {
                assert_eq!(m.register(i), Some(i as i16 + 101), "register {i}");
            }
        }
        assert_eq!(
            m.output(),
            format!("NOW: QUADRANT(36) SECTOR(88)\n*SHORT RANGE SENSOR*DAMAGED!\n{damage}\n YEARS ESTIMATED FOR REPAIR.\n").as_bytes()
        );
        for i in 0..240 {
            assert_eq!(m.storage(i), before[i as usize]);
        }
    }
}
