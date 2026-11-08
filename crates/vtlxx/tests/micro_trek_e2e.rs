//! Rust CLI E2E: canonical complete source, with seed 0 supplied as the first runtime line.
//!
//! CLI exposes output and exit status, not final storage or PRNG. The
//! corresponding owner-state checks remain in `micro_trek_lifecycle`
//! (replay PRNG continuity), `micro_trek_turn_end` (K=0 victory priority),
//! `micro_trek_warp`, `micro_trek_departure`, `micro_trek_torpedo`,
//! `micro_trek_phaser`, and the scan/map/status direct tests. These tests do
//! not claim to observe final storage or a CLI victory.
//!
//! Direct-test evidence, independent of the CLI transcripts below:
//! - `micro_trek_lifecycle::replay_initialization_continues_the_random_stream`
//!   verifies two initializations preserve and advance storage[192], reproduce
//!   the same states on repetition, and restore the caller's stack prefix.
//! - `micro_trek_turn_end::win_precedes_dock_and_attack_even_when_energy_and_days_are_exhausted`
//!   verifies K=0/E=0 or -1/D=0 returns WIN=1, without output or state changes.
//! - `micro_trek_departure::departure_crosses_multiple_quadrants_before_rebuilding_final_sector`
//!   verifies its independent seed 1 fixture reaches (2,4,7,4), D=18,
//!   E=3950 and PRNG=3882 after storm, damage, and final reconstruction.
//! - `micro_trek_warp::crossing_rebuilds_only_after_storm_and_outside_relocates`
//!   verifies its independent seed 0 outside fixture ends with PRNG=17180
//!   and valid relocated coordinates, without a storm draw.
//! - `micro_trek_torpedo::star_hit_and_reprompt`,
//!   `micro_trek_phaser::retry_then_eof_preserves_game_state`,
//!   `micro_trek_long_scan::scan_marks_only_visible_cells_and_is_idempotent`,
//!   and `micro_trek_status::reports_condition_priority_and_latest_owned_state_without_side_effects`
//!   directly check the corresponding owner state and stack contracts.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const SOURCE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/micro_trek.vtl");
// Each run has its own wall-clock deadline, including input and pipe reads.
const LIMIT: Duration = Duration::from_secs(30);

fn run(input: &[u8]) -> (i32, Vec<u8>, Vec<u8>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_vtlxx"))
        .arg(SOURCE)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let bytes = input.to_vec();
    let writer = thread::spawn(move || stdin.write_all(&bytes));
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let errors = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let deadline = Instant::now() + LIMIT;
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            child.kill().unwrap();
            break child.wait().unwrap();
        }
        thread::sleep(Duration::from_millis(10));
    };
    let written = writer.join().unwrap();
    let output = reader.join().unwrap();
    let error = errors.join().unwrap();
    assert!(!timed_out, "CLI exceeded {LIMIT:?}");
    written.unwrap();
    (status.code().unwrap_or(-1), output, error)
}

fn run_seeded(seed: &[u8], input: &[u8]) -> (i32, Vec<u8>, Vec<u8>) {
    let mut all = seed.to_vec();
    all.extend_from_slice(input);
    run(&all)
}

fn reproducible(input: &[u8]) -> String {
    let first = run_seeded(b"0\n", input);
    assert_eq!(
        first,
        run_seeded(b"0\n", input),
        "seed 0 and input {input:?}"
    );
    assert_eq!(first.0, 0, "stderr: {:?}", first.2);
    assert!(first.2.is_empty(), "stderr: {:?}", first.2);
    String::from_utf8(first.1).unwrap()
}

fn ordered(output: &str, parts: &[&str]) {
    let mut rest = output;
    for part in parts {
        let position = rest
            .find(part)
            .unwrap_or_else(|| panic!("missing {part:?} in {rest:?}"));
        rest = &rest[position + part.len()..];
    }
}

#[test]
fn displays_then_x_without_turn_end() {
    // Input ends in X; all five display commands return to COMMAND.
    let output = reproducible(b"G\nL\nS\nR\n?\nX\n");
    ordered(
        &output,
        &[
            "DESTROY 26 KLINGON CRUISERS\n",
            "COMMAND :\n########## GARAXY MAP ##########\n",
            "COMMAND :NOW: QUADRANT(78) SECTOR(87)\n*LONG RANGE SENSOR*\n",
            "6 002 001 ***\n7 402 001 ***\n8 002 001 ***\n ...7...8...9\n",
            "COMMAND :NOW: QUADRANT(78) SECTOR(87)\n*SHORT RANGE SENSOR*\n",
            "8 . . . . . . E .\n  1 2 3 4 5 6 7 8\n",
            "COMMAND :*STATUS REPORT*\n\n*STARDATE : 3200\n",
            "COMMAND :*HELP*\nS : SHORT RANGE SENSOR\n",
            "? : HELP\nCOMMAND :",
        ],
    );
    assert_eq!(output.matches("COMMAND :").count(), 6);
    assert!(!output.contains("*KLINGON ATTACK"));
    assert!(!output.contains("RETRY?(Y/N)"));
    assert!(output.ends_with("COMMAND :"));
}

#[test]
fn invalid_numeric_retry_cancel_and_eof() {
    let output = reproducible(b"P\nbad\n0\nX\n");
    assert!(output.ends_with("COMMAND :*PHASER*\nENERGY ? ENERGY ? COMMAND :"));
    assert_eq!(output.matches("ENERGY ? ").count(), 2);
    assert!(!output.contains("*KLINGON ATTACK"));
    let eof = reproducible(b"P\nbad\n");
    assert!(eof.ends_with("COMMAND :*PHASER*\nENERGY ? ENERGY ? "));
    assert!(!eof.contains("RETRY?(Y/N)"));
}

#[test]
fn actions_and_outside_warp_follow_fixed_tracks() {
    let output = reproducible(b"P\n10\nT\n0\nW\n1\n0\nR\nX\n");
    ordered(
        &output,
        &[
            "COMMAND :*PHASER*\nENERGY ? MISSED!!\n",
            "COMMAND :*TORPEDOES*\nCOURSE: ENTERPRISE FIRING TORPEDOES.\nTRACK-77-67-57-47-37-27-17-MISS\n",
            "COMMAND :*WARP*\n FACTOR ? COURSE: * ION ENGINE START\n78-87\nNOW: QUADRANT(78) SECTOR(77)\n",
            "*STARDATE : 3201\n",
            "*ENERGY...: 3989\n",
            "*TORPEDOES: 9\n",
            "COMMAND :",
        ],
    );
    assert_eq!(output.matches("COMMAND :").count(), 5);
    let outside = reproducible(b"W\n89\n0\nX\n");
    ordered(
        &outside,
        &[
            "* ION ENGINE START\n78-87\n",
            "*WARP ENGINE START\n68-87\n",
            "18-17\n OUTSIDE THE GARAXY !!\nON BOARD COMPUTER TAKES OVER,\n",
            "*COMPUTER*DAMAGED!\n3\n YEARS ESTIMATED FOR REPAIR.\n",
            "COMMAND :",
        ],
    );
    assert_eq!(outside.matches("*WARP ENGINE START\n").count(), 1);
    assert!(!outside.contains("## SPACE STORM ##"));
    assert!(outside.ends_with("COMMAND :"));
}

#[test]
fn two_defeats_invalid_retry_yes_then_no() {
    // Seed 0: 30 committed W=1/course=0 turns per mission exhaust time.
    let mut input = b"W\n1\n0\n".repeat(30);
    input.extend_from_slice(b"bad\nY\n");
    input.extend_from_slice(&b"W\n1\n0\n".repeat(30));
    input.extend_from_slice(b"N\n");
    let output = reproducible(&input);
    ordered(
        &output,
        &[
            "OBJECTIVE :\nDESTROY 26 KLINGON CRUISERS\n",
            "IT IS STARDATE : 3230\nPLANETS UNION WAS CONQUERED \nRETRY?(Y/N)RETRY?(Y/N)",
            "OBJECTIVE :\n",
            "IT IS STARDATE : 3230\nPLANETS UNION WAS CONQUERED \nRETRY?(Y/N)",
        ],
    );
    assert_eq!(output.matches("OBJECTIVE :\n").count(), 2);
    assert_eq!(output.matches("RETRY?(Y/N)").count(), 3);
    assert_eq!(output.matches("* ION ENGINE START\n").count(), 60);
    assert!(output.ends_with("RETRY?(Y/N)"));
}

#[test]
fn initial_eof_is_normal_exit() {
    let output = reproducible(b"");
    assert!(output.ends_with("THE NUMBER OF BASE IS 3\nCOMMAND :"));
    assert!(!output.contains("RETRY?(Y/N)"));
}

#[test]
fn seed_boundaries_are_accepted_and_deterministic() {
    for seed in [b"0\n".as_slice(), b"1\n", b"-1\n", b"-32768\n", b"32767\n"] {
        let first = run_seeded(seed, b"X\n");
        assert_eq!(first, run_seeded(seed, b"X\n"), "seed {seed:?}");
        assert_eq!(first.0, 0, "stderr: {:?}", first.2);
        assert!(first.2.is_empty());
        assert!(first.1.ends_with(b"COMMAND :"));
    }
}

#[test]
fn seed_eof_and_invalid_lines_do_not_start_the_game() {
    for (input, expected) in [
        (b"".as_slice(), b"".as_slice()),
        (b"\n", b"INVALID SEED\n"),
        (b"nope\n", b"INVALID SEED\n"),
        (b"32768\n", b"INVALID SEED\n"),
        (b"-32769\n", b"INVALID SEED\n"),
    ] {
        let result = run(input);
        assert_eq!(result.0, 0, "input {input:?}, stderr: {:?}", result.2);
        assert_eq!(result.1, expected, "input {input:?}");
        assert!(result.2.is_empty());
    }
}
