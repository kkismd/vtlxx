use vtlxx_poc_extended_classic_vtl::Machine;

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const PREFIX: &[i16] = &[-1234, 0, 32767];

fn machine(u: i16, v: i16, damage: i16) -> Machine {
    let mut m = Machine::new();
    let definitions = SOURCE.split_once("; EC08_TEST_DEFINITIONS_END").unwrap().0;
    m.execute_source(definitions).unwrap();
    // Mixed observed and unobserved cells with distinct three-digit contents.
    for i in 0..64 {
        let value = (i * 7 + 3) % 400 + if i % 2 == 0 { 1000 } else { 0 };
        m.execute_source(&format!("@={i},{value}")).unwrap();
    }
    m.execute_source(&format!(
        "@=165,{u} @=166,{v} @=167,4 @=168,5 @=66,{damage} @=192,12345"
    ))
    .unwrap();
    for &value in PREFIX {
        m.push(value);
    }
    m
}

fn expected(u: i16, v: i16) -> String {
    let mut out = format!("NOW: QUADRANT({u}{v}) SECTOR(45)\n*LONG RANGE SENSOR*\n");
    for x in u - 1..=u + 1 {
        out.push_str(&x.to_string());
        for y in v - 1..=v + 1 {
            if !(1..=8).contains(&x) || !(1..=8).contains(&y) {
                out.push_str(" ***");
            } else {
                let i = (x - 1) * 8 + y - 1;
                out.push_str(&format!(" {:03}", (i * 7 + 3) % 400));
            }
        }
        out.push('\n');
    }
    out.push_str(&format!(" ...{}...{}...{}\n", v - 1, v, v + 1));
    out
}

#[test]
fn scan_marks_only_visible_cells_and_is_idempotent() {
    for (u, v) in [(4, 4), (1, 1), (1, 8), (8, 1), (8, 8), (1, 4)] {
        let mut m = machine(u, v, 0);
        let before: Vec<_> = (0..240).map(|i| m.storage(i)).collect();
        for count in 1..=2 {
            m.execute_source("l=()").unwrap();
            assert_eq!(m.stack(), PREFIX);
            assert_eq!(m.output(), expected(u, v).repeat(count).as_bytes());
            for i in 0..240 {
                let x = i / 8 + 1;
                let y = i % 8 + 1;
                let visible = i < 64 && (x as i16 - u).abs() <= 1 && (y as i16 - v).abs() <= 1;
                let value = before[i as usize];
                let after = if visible && value < 1000 {
                    value + 1000
                } else {
                    value
                };
                assert_eq!(m.storage(i), after, "({u},{v}) cell {i}");
            }
        }
    }
}

#[test]
fn damaged_sensor_only_displays_device_and_position() {
    for damage in [1, 3] {
        let mut m = machine(4, 4, damage);
        let before: Vec<_> = (0..240).map(|i| m.storage(i)).collect();
        m.execute_source("l=()").unwrap();
        assert_eq!(m.stack(), PREFIX);
        assert_eq!(m.output(), format!("NOW: QUADRANT(44) SECTOR(45)\n*LONG RANGE SENSOR*DAMAGED!\n{damage}\n YEARS ESTIMATED FOR REPAIR.\n").as_bytes());
        for i in 0..240 {
            assert_eq!(m.storage(i), before[i as usize]);
        }
    }
}
