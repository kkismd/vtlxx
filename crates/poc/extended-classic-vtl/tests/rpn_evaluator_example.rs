use std::{path::Path, process::Command};
use vtlxx_poc_extended_classic_vtl::Machine;

const EXAMPLE: &str = include_str!("../examples/rpn_evaluator.vtl");
const TOKENS: [i16; 7] = [3, 4, -1, 2, -3, 7, -2];

#[test]
fn rpn_example_consumes_its_result_and_reads_all_source_initialized_tokens() {
    let mut machine = Machine::new();
    machine.execute_source(EXAMPLE).unwrap();

    assert_eq!(machine.output(), b"7\n");
    assert!(machine.stack().is_empty());
    assert_eq!(machine.register(15), Some(7)); // P: next token address
    for (address, token) in TOKENS.iter().enumerate() {
        assert_eq!(machine.storage(address as u16), *token);
    }
}

#[test]
fn operators_consume_lhs_and_rhs_without_touching_an_older_stack_value() {
    let definitions = EXAMPLE.split("  ; e:").next().unwrap();
    for (operator, expected) in [('a', 17), ('s', 7), ('m', 60)] {
        let mut machine = Machine::new();
        let source = format!("{definitions}  ~=99,12,5\n  {operator}=()\n");
        machine.execute_source(&source).unwrap();
        assert_eq!(machine.stack(), &[99, expected], "operator {operator}");
    }
}

#[test]
fn evaluator_preserves_a_callers_older_stack_value() {
    let source = EXAMPLE.replace("  e=()\n  ?=~", "  ~=99\n  e=()\n  ?=~");
    assert_ne!(source, EXAMPLE);

    let mut machine = Machine::new();
    machine.execute_source(&source).unwrap();

    assert_eq!(machine.output(), b"7\n");
    assert_eq!(machine.stack(), &[99]);
}

#[test]
fn rpn_example_runs_from_cli() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/rpn_evaluator.vtl");
    let output = Command::new(env!("CARGO_BIN_EXE_ecvtl"))
        .arg(path)
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"7\n");
}
