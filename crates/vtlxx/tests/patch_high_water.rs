use std::path::Path;
use std::process::Command;

// Fifteen outer IFs and 32 forward references remain pending when ELSE
// registers its new jump before resolving the old one: 15 + 32 + 2 = 49.
// The skipped literal also crosses two 255-byte chunk boundaries.
#[test]
#[ignore = "requires ca65, ld65 and Linux mos6502; run with --ignored"]
fn combined_49_pending_patches_match_the_reference() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/combined49.vtl");
    let reference = Command::new(env!("CARGO_BIN_EXE_vtlxx"))
        .arg(&source)
        .output()
        .unwrap();
    assert_eq!(reference.status.code(), Some(0), "{reference:?}");
    assert_eq!(reference.stdout, b"78");

    let target = Command::new(env!("CARGO_BIN_EXE_vtlxx6502"))
        .arg(&source)
        .args(["--cycle-limit", "1000000000"])
        .output()
        .unwrap();
    assert_eq!(target.status.code(), reference.status.code(), "{target:?}");
    assert_eq!(target.stdout, reference.stdout);
}
