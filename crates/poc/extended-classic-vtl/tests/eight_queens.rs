use vtlxx_poc_extended_classic_vtl::Machine;

#[test]
fn source_defined_write_handler_solves_eight_queens() {
    let mut machine = Machine::new();
    machine
        .execute_source(include_str!("fixtures/eight_queens.vtl"))
        .unwrap();
    assert_eq!(machine.output(), b"92");
}
