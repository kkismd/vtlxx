use std::io::Cursor;
use vtlxx_poc_extended_classic_vtl::Machine;

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const PREFIX: &[i16] = &[-32768, 123, 32767];

fn machine(input: &str) -> Machine {
    let mut m = Machine::new();
    m.execute_source(SOURCE.split_once("; EC08_TEST_DEFINITIONS_END").unwrap().0)
        .unwrap();
    m.execute_source("@=160,19 @=161,4000 @=162,3 @=163,1 @=164,1 @=165,4 @=166,4 @=167,4 @=168,4 @=169,1 @=170,1 @=27,1111 @=98,4 @=192,123 @=70,42").unwrap();
    m.set_input(Cursor::new(input.as_bytes().to_vec()));
    for &value in PREFIX {
        m.push(value);
    }
    m
}

fn call(m: &mut Machine, status: i16) {
    m.execute_source("torpedo=()").unwrap();
    let mut expected = PREFIX.to_vec();
    expected.push(status);
    assert_eq!(m.stack(), expected);
}

#[test]
fn unavailable_and_eof_do_not_fire() {
    for (device, count, input, status) in
        [(1, 3, "45\n", 0), (0, 0, "45\n", 0), (0, 3, "bad\n", -1)]
    {
        let mut m = machine(input);
        m.execute_source(&format!("@=69,{device} @=162,{count}"))
            .unwrap();
        let before: Vec<_> = (0..240).map(|i| m.storage(i)).collect();
        call(&mut m, status);
        assert_eq!((0..240).map(|i| m.storage(i)).collect::<Vec<_>>(), before);
        assert!(!String::from_utf8_lossy(m.output()).contains("TRACK-"));
        if status == 0 && !input.is_empty() {
            m.execute_source("A=~ readnum=()").unwrap();
            let mut expected = PREFIX.to_vec();
            expected.extend([45, 1]);
            assert_eq!(m.stack(), expected);
        }
    }
}

#[test]
fn star_hit_and_reprompt() {
    for input in ["0\n", "360\n", "bad\n361\n0\n"] {
        let mut m = machine(input);
        m.execute_source("@=82,3").unwrap(); // sector (2,4)
        call(&mut m, 1);
        assert!(
            String::from_utf8_lossy(m.output())
                .ends_with("ENTERPRISE FIRING TORPEDOES.\nTRACK-34-24-HIT\n")
        );
        assert_eq!(
            (m.storage(82), m.storage(27), m.storage(162), m.storage(192)),
            (0, 1110, 2, 123)
        );
        assert_eq!((m.storage(167), m.storage(168), m.storage(98)), (4, 4, 4));
    }
}

#[test]
fn base_hit_and_enemy_slot_hole() {
    let mut m = machine("0\n");
    m.execute_source("@=90,2").unwrap(); // (3,4)
    call(&mut m, 1);
    assert!(String::from_utf8_lossy(m.output()).ends_with("TRACK-34-HIT\n"));
    assert_eq!(
        (m.storage(90), m.storage(27), m.storage(164), m.storage(170)),
        (0, 1101, 0, 0)
    );
    let mut m = machine("45\n");
    m.execute_source("@=91,1 @=137,300 @=143,3 @=149,5")
        .unwrap();
    call(&mut m, 1);
    assert!(String::from_utf8_lossy(m.output()).ends_with("TRACK-35-HIT\n### DESTROYED ###\n"));
    assert_eq!(
        (
            m.storage(137),
            m.storage(91),
            m.storage(27),
            m.storage(163),
            m.storage(169)
        ),
        (0, 0, 1011, 0, 0)
    );
}

#[test]
fn surviving_enemy_and_unrelated_storage_are_preserved() {
    let mut m = machine("45\n");
    m.execute_source("@=91,1 @=137,600 @=143,3 @=149,5 @=136,37 @=142,8 @=148,8")
        .unwrap();
    let before: Vec<_> = (0..240).map(|i| m.storage(i)).collect();
    call(&mut m, 1);
    assert!(String::from_utf8_lossy(m.output()).ends_with("TRACK-35-HIT\n# DAMAGED #\n"));
    assert_eq!(m.storage(137), 100);
    for i in 0..240 {
        if i != 137 && i != 162 {
            assert_eq!(m.storage(i), before[i as usize], "storage {i}");
        }
    }
}

#[test]
fn all_directions_and_edges_miss_without_out_of_range_coordinates() {
    for (course, first) in [
        (0, "34-"),
        (45, "35-"),
        (90, "45-"),
        (135, "55-"),
        (180, "54-"),
        (225, "53-"),
        (270, "43-"),
        (315, "33-"),
        (360, "34-"),
    ] {
        let mut m = machine(&format!("{course}\n"));
        call(&mut m, 1);
        let output = String::from_utf8_lossy(m.output());
        let track = output.split("TRACK-").nth(1).unwrap();
        assert!(track.starts_with(first), "{course}: {track}");
        assert!(track.ends_with("MISS\n"));
        assert_eq!(
            (
                m.storage(162),
                m.storage(192),
                m.storage(167),
                m.storage(168)
            ),
            (2, 123, 4, 4)
        );
    }
    for (x, y, course) in [(1, 4, 0), (8, 4, 180), (4, 1, 270), (4, 8, 90)] {
        let mut m = machine(&format!("{course}\n"));
        m.execute_source(&format!("@=167,{x} @=168,{y}")).unwrap();
        call(&mut m, 1);
        assert!(String::from_utf8_lossy(m.output()).ends_with("TRACK-MISS\n"));
    }
}
