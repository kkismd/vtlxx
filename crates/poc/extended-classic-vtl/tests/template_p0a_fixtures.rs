use std::path::PathBuf;

use vtlxx_poc_extended_classic_vtl::{CompileError, Machine, SourceError};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/template_p0a");

fn fixture(name: &str) -> String {
    std::fs::read_to_string(PathBuf::from(FIXTURES).join(name)).unwrap()
}

fn expected(name: &str, field: &str) -> String {
    fixture(&format!("{name}.expected"))
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{field}: ")))
        .unwrap_or_else(|| panic!("missing {field} in {name}.expected"))
        .to_owned()
}

fn expected_output(name: &str) -> Vec<u8> {
    expected(name, "stdout").replace("\\n", "\n").into_bytes()
}

fn expected_registers(name: &str) -> Vec<(usize, i16)> {
    expected(name, "registers")
        .split(", ")
        .map(|assignment| {
            let (register, value) = assignment.split_once('=').unwrap();
            (
                usize::from(register.as_bytes()[0] - b'A'),
                value.parse().unwrap(),
            )
        })
        .collect()
}

#[test]
fn minimal_two_argument_fixture_matches_its_expected_state() {
    assert_eq!(expected("minimal_two_arguments", "result"), "success");
    let mut machine = Machine::new();
    machine
        .execute_source(&fixture("minimal_two_arguments.vtl"))
        .unwrap();

    assert_eq!(machine.output(), expected_output("minimal_two_arguments"));
    for (index, value) in expected_registers("minimal_two_arguments") {
        assert_eq!(machine.register(index), Some(value));
    }
    assert!(machine.stack().is_empty());
}

#[test]
fn template_call_inside_if_preserves_block_reader_and_following_statements() {
    assert_eq!(expected("block_then_else", "result"), "success");
    let mut machine = Machine::new();
    machine
        .execute_source(&fixture("block_then_else.vtl"))
        .unwrap();

    assert_eq!(machine.output(), expected_output("block_then_else"));
    for (index, value) in expected_registers("block_then_else") {
        assert_eq!(machine.register(index), Some(value));
    }
    assert!(machine.stack().is_empty());
}

#[test]
fn template_rhs_validation_is_deferred_until_invocation() {
    assert_eq!(expected("deferred_rhs_definition", "result"), "success");
    assert_eq!(expected("deferred_rhs_call", "result"), "compile_error");
    let mut machine = Machine::new();
    machine
        .execute_source(&fixture("deferred_rhs_definition.vtl"))
        .unwrap();

    assert_eq!(
        machine.execute_source(&fixture("deferred_rhs_call.vtl")),
        Err(SourceError::Compile(CompileError::Syntax))
    );
    assert_eq!(machine.register(1), Some(0));
    assert!(machine.stack().is_empty());

    let mut other_machine = Machine::new();
    assert!(matches!(
        other_machine.execute_source(&fixture("deferred_rhs_call.vtl")),
        Err(SourceError::Compile(CompileError::UndefinedBinding))
    ));
}

#[test]
fn malformed_and_undefined_invocations_are_compile_errors() {
    assert!(expected("invalid_call_forms", "result").starts_with("compile_error"));
    let source = fixture("invalid_call_forms.vtl");
    for statement in source.lines() {
        let mut machine = Machine::new();
        machine.execute_source("&=a{} [ A={} ]").unwrap();
        assert!(
            matches!(
                machine.execute_source(statement),
                Err(SourceError::Compile(_))
            ),
            "{statement}"
        );
        assert_eq!(machine.register(0), Some(0));
        assert!(machine.stack().is_empty());
    }
}

#[test]
fn failed_definitions_are_never_published() {
    assert!(expected("invalid_definitions", "result").starts_with("compile_error"));
    for source in fixture("invalid_definitions.vtl").lines() {
        let mut machine = Machine::new();
        assert!(
            matches!(machine.execute_source(source), Err(SourceError::Compile(_))),
            "{source}"
        );
        for identity in 'a'..='g' {
            let invocation = format!("{identity}{{A}}=()");
            assert_eq!(
                machine.execute_source(&invocation),
                Err(SourceError::Compile(CompileError::UndefinedBinding)),
                "{source} published {identity}"
            );
        }
    }
}

#[test]
fn failed_expansion_does_not_run_the_partial_caller_or_mutate_template() {
    assert_eq!(
        expected("partial_expansion_failure", "result"),
        "compile_error"
    );
    let mut machine = Machine::new();
    assert!(matches!(
        machine.execute_source(&fixture("partial_expansion_failure.vtl")),
        Err(SourceError::Compile(_))
    ));
    assert_eq!(machine.register(0), Some(0));
    assert_eq!(machine.register(1), Some(0));
    assert!(machine.stack().is_empty());
    assert!(matches!(
        machine.execute_source("a{B}=()"),
        Err(SourceError::Compile(CompileError::Syntax))
    ));
}

#[test]
fn rust_chunk_failure_profile_retains_prior_flush_and_template() {
    assert!(expected("chunk_failure_profiles", "Rust profile").contains("B=7"));
    let mut machine = Machine::new();
    assert!(matches!(
        machine.execute_source(&fixture("chunk_failure_profiles.vtl")),
        Err(SourceError::Compile(_))
    ));
    assert_eq!(machine.register(1), Some(7));
    machine.execute_source("B=2 a{B}=()").unwrap();
    assert_eq!(machine.register(0), Some(2));
    assert_eq!(machine.register(1), Some(2));
    assert_eq!(
        machine.execute_source("c{A}=()"),
        Err(SourceError::Compile(CompileError::UndefinedBinding))
    );
}
