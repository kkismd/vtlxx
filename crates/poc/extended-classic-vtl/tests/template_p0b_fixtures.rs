use vtlxx_poc_extended_classic_vtl::{CompileError, Machine, SourceError};

#[test]
fn portable_operand_example() {
    let mut machine = Machine::new();
    machine
        .execute_source(include_str!("fixtures/template_p0b/minimal.vtl"))
        .unwrap();
    assert_eq!(machine.output(), b"15\n");
    assert_eq!(machine.register(0), Some(15));
    assert!(machine.stack().is_empty());
}

#[test]
fn invalid_operand_retains_rust_flushed_effect_and_template() {
    let mut machine = Machine::new();
    assert_eq!(
        machine.execute_source(include_str!("fixtures/template_p0b/invalid_rhs.vtl")),
        Err(SourceError::Compile(CompileError::Syntax))
    );
    assert_eq!(machine.register(1), Some(7));
    machine.execute_source("a{A}=3").unwrap();
    assert_eq!(machine.register(0), Some(3));
}
