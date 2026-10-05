use std::{path::Path, process::Command};
use vtlxx_poc_extended_classic_vtl::Machine;

const EXAMPLE: &str = include_str!("../examples/maze_dfs.vtl");
const GOAL_PATH: [i16; 7] = [0, 1, 5, 9, 13, 14, 15];

#[test]
fn dfs_backtracks_from_two_and_keeps_only_the_goal_path_live() {
    let mut machine = Machine::new();
    machine.execute_source(EXAMPLE).unwrap();

    assert_eq!(machine.output(), b"7\n");
    assert!(machine.stack().is_empty());
    assert_eq!(machine.register(2), Some(15)); // C: current cell
    assert_eq!(machine.register(3), Some(6)); // D: zero-based path top

    let open = [0, 1, 2, 5, 9, 13, 14, 15];
    for cell in 0..16 {
        assert_eq!(machine.storage(cell), i16::from(open.contains(&cell)));
        assert_eq!(machine.storage(16 + cell), i16::from(open.contains(&cell)));
    }

    let path: Vec<_> = (0..GOAL_PATH.len())
        .map(|depth| machine.storage(32 + depth as u16))
        .collect();
    assert_eq!(path, GOAL_PATH);
    // The right-first walk visited dead end 2, then popped it before reaching 5.
    assert_eq!(machine.storage(18), 1);
    assert!(!path.contains(&2));
    assert_eq!(machine.storage(49), 2); // cell 1 resumed after trying right
}

#[test]
fn dfs_example_runs_from_cli() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/maze_dfs.vtl");
    let output = Command::new(env!("CARGO_BIN_EXE_ecvtl"))
        .arg(path)
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {:?}", output.stderr);
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"7\n");
}
