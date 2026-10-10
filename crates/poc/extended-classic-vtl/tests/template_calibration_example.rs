use std::{path::Path, process::Command};
use vtlxx_poc_extended_classic_vtl::Machine;

const EXAMPLE: &str = include_str!("../examples/p0a_template_calibration.vtl");
const EXPECTED_OUTPUT: &[u8] = b"58\n20\n40\n";
const EXPECTED_REGISTERS: [i16; 3] = [58, 20, 40];

fn assert_calibration(machine: &Machine) {
    for (index, expected) in EXPECTED_REGISTERS.into_iter().enumerate() {
        assert_eq!(machine.register(index), Some(expected));
    }
    assert!(machine.stack().is_empty());
    assert_eq!(machine.output(), EXPECTED_OUTPUT);
}

#[test]
fn template_calibration_example_matches_ordinary_assignments() {
    let mut template_machine = Machine::new();
    template_machine.execute_source(EXAMPLE).unwrap();
    assert_calibration(&template_machine);

    let mut ordinary_machine = Machine::new();
    ordinary_machine
        .execute_source(
            "A=3 B=4 C=5 \
             A=A*2+1 A=A*2 \
             B=B*3-2 B=B*2 \
             C=C*4+0 C=C*2 \
             A=A*2+1 A=A*2 \
             ?=A ?=() ?=B ?=() ?=C ?=()",
        )
        .unwrap();
    assert_calibration(&ordinary_machine);
}

#[test]
fn template_calibration_example_runs_from_cli() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/p0a_template_calibration.vtl");
    let output = Command::new(env!("CARGO_BIN_EXE_ecvtl"))
        .arg(path)
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, EXPECTED_OUTPUT);
}
