use vtlxx_poc_extended_classic_vtl::Machine;

#[test]
fn public_machine_api_smoke() {
    let mut machine = Machine::new();
    assert_eq!(machine.register(0), Some(0));
    assert_eq!(machine.storage(0), 0);
    machine.push(12);
    assert_eq!(machine.stack(), &[12]);
    assert_eq!(machine.pop(), Some(12));
    assert!(machine.output().is_empty());
    machine.clear_output();
}
