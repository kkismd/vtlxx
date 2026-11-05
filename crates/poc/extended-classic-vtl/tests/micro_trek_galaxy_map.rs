use vtlxx_poc_extended_classic_vtl::Machine;

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const PREFIX: &[i16] = &[-1234, 0, 32767];

fn check_map(cells: &[i16; 64], expected_rows: &[&str; 8]) {
    let mut machine = Machine::new();
    let definitions = SOURCE.split_once("; EC08_TEST_DEFINITIONS_END").unwrap().0;
    machine.execute_source(definitions).unwrap();
    for (index, value) in cells.iter().enumerate() {
        machine
            .execute_source(&format!("@={index},{value}"))
            .unwrap();
    }
    machine
        .execute_source("@=192,12345 @=64,3 @=170,1")
        .unwrap();
    for register in b'A'..=b'Z' {
        machine
            .execute_source(&format!("{}={}", register as char, register as i16 + 101))
            .unwrap();
    }
    for value in PREFIX {
        machine.push(*value);
    }
    let before: Vec<_> = (0..=u16::MAX).map(|i| machine.storage(i)).collect();
    let expected = format!(
        "\n########## GARAXY MAP ##########\n{}\n\n ..1...2...3...4...5...6...7...8\n",
        expected_rows.join("\n")
    );
    for count in 1..=2 {
        machine.execute_source("galmap=()").unwrap();
        assert_eq!(machine.stack(), PREFIX);
        assert_eq!(machine.output(), expected.repeat(count).as_bytes());
        for register in b'A'..=b'Z' {
            if !matches!(register, b'I' | b'J' | b'K') {
                assert_eq!(
                    machine.register((register - b'A') as usize),
                    Some(register as i16 + 101)
                );
            }
        }
        for i in 0..=u16::MAX {
            assert_eq!(machine.storage(i), before[i as usize], "storage {i}");
        }
    }
}

#[test]
fn unobserved_and_observed_maps_use_fixed_width_cells() {
    let unknown = " *** *** *** *** *** *** *** ***";
    check_map(&[517; 64], &[unknown; 8]);

    let mut observed = [1000; 64];
    observed[0] = 1517;
    observed[63] = 1001;
    check_map(
        &observed,
        &[
            " 517 000 000 000 000 000 000 000",
            " 000 000 000 000 000 000 000 000",
            " 000 000 000 000 000 000 000 000",
            " 000 000 000 000 000 000 000 000",
            " 000 000 000 000 000 000 000 000",
            " 000 000 000 000 000 000 000 000",
            " 000 000 000 000 000 000 000 000",
            " 000 000 000 000 000 000 000 001",
        ],
    );
}

#[test]
fn mixed_map_distinguishes_observed_zero_from_unobserved_cells() {
    let mut cells = [0; 64];
    cells[0] = 1000;
    cells[9] = 1012;
    cells[63] = 1517;
    check_map(
        &cells,
        &[
            " 000 *** *** *** *** *** *** ***",
            " *** 012 *** *** *** *** *** ***",
            " *** *** *** *** *** *** *** ***",
            " *** *** *** *** *** *** *** ***",
            " *** *** *** *** *** *** *** ***",
            " *** *** *** *** *** *** *** ***",
            " *** *** *** *** *** *** *** ***",
            " *** *** *** *** *** *** *** 517",
        ],
    );
}
