use std::{collections::HashSet, path::Path, process::Command};
use vtlxx_poc_extended_classic_vtl::Machine;

const EXAMPLE: &str = include_str!("../examples/eight_queens.vtl");
const EXPECTED_OUTPUT: &[u8] = include_bytes!("fixtures/eight_queens_example_output.txt");

fn assert_solution_output(output: &[u8]) {
    assert_eq!(output, EXPECTED_OUTPUT, "solution output changed");
    assert!(output.ends_with(b"\n"), "output must end with LF");

    let lines: Vec<_> = output[..output.len() - 1]
        .split(|byte| *byte == b'\n')
        .collect();
    assert_eq!(lines.len(), 93, "92 solutions followed by the count");
    assert_eq!(lines[92], b"92");

    let solutions = &lines[..92];
    for solution in solutions {
        assert_eq!(solution.len(), 8);
        assert!(solution.iter().all(|byte| (b'1'..=b'8').contains(byte)));
    }
    assert_eq!(solutions.iter().copied().collect::<HashSet<_>>().len(), 92);
}

#[test]
fn example_lists_all_solutions_using_public_api() {
    let mut machine = Machine::new();
    machine.execute_source(EXAMPLE).unwrap();
    assert_solution_output(machine.output());
}

#[test]
fn example_runs_from_cli() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/eight_queens.vtl");
    let output = Command::new(env!("CARGO_BIN_EXE_ecvtl"))
        .arg(path)
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(output.stderr.is_empty());
    assert_solution_output(&output.stdout);
}
