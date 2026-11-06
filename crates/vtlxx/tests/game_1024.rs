use std::io::Cursor;
use std::process::{Command, Stdio};
use vtlxx::Machine;

const SOURCE: &str = include_str!("../examples/1024.vtl");
const MARKER: &str = "; GAME1024_TEST_DEFINITIONS_END";
const PREFIX: &[i16] = &[-32768, 1234, -7, 32767];

fn machine() -> Machine {
    let mut markers = SOURCE.match_indices(MARKER).filter(|(i, _)| {
        (*i == 0 || SOURCE.as_bytes()[i - 1] == b'\n')
            && SOURCE
                .as_bytes()
                .get(i + MARKER.len())
                .is_none_or(|b| *b == b'\n')
    });
    let (end, _) = markers.next().expect("missing definitions marker");
    assert!(markers.next().is_none(), "duplicate definitions marker");
    let mut m = Machine::new();
    m.execute_source(&SOURCE[..end]).unwrap();
    for &v in PREFIX {
        m.push(v);
    }
    m
}

fn set(m: &mut Machine, index: usize, value: i16) {
    m.execute_source(&format!("@={index},{value}")).unwrap();
}
fn board(m: &Machine) -> Vec<i16> {
    (0..16).map(|i| m.storage(i)).collect()
}
fn call(m: &mut Machine, source: &str) {
    m.execute_source(source).unwrap();
    assert_eq!(m.stack(), PREFIX, "stack after {source}");
}

#[test]
fn initial_draws_and_initialization_are_independent_expectations() {
    let mut m = machine();
    set(&mut m, 20, 12345);
    for (bound, value, state) in [
        (16, 7, 2822),
        (10, 2, 11031),
        (15, 1, 21180),
        (10, 4, -22907),
    ] {
        m.execute_source(&format!("random={bound}")).unwrap();
        assert_eq!(m.pop().unwrap(), value);
        assert_eq!(m.stack(), PREFIX);
        assert_eq!(m.storage(20), state);
    }
    set(&mut m, 20, 12345);
    set(&mut m, 26, 1);
    call(&mut m, "newgame=()");
    assert_eq!(board(&m), [1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(
        (
            m.storage(20),
            m.storage(21),
            m.storage(22),
            m.storage(23),
            m.storage(26)
        ),
        (-22907, 0, 0, 0, 1)
    );
    call(&mut m, "newgame=()");
    assert_ne!(m.storage(20), -22907);
    assert_eq!(m.storage(26), 1);
}

#[test]
fn movement_merges_once_in_each_direction_and_scores() {
    for (direction, indices) in [
        (0, [0, 1, 2, 3]),
        (1, [3, 2, 1, 0]),
        (2, [0, 4, 8, 12]),
        (3, [12, 8, 4, 0]),
    ] {
        for (input, expected, score) in [
            ([2, 2, 2, 2], [4, 4, 0, 0], 8),
            ([2, 0, 2, 4], [4, 4, 0, 0], 4),
            ([1, 1, 2, 2], [2, 4, 0, 0], 6),
            ([2, 2, 4, 0], [4, 4, 0, 0], 4),
            ([4, 2, 0, 0], [4, 2, 0, 0], 0),
        ] {
            let mut m = machine();
            for (idx, value) in indices.into_iter().zip(input) {
                set(&mut m, idx, value);
            }
            call(&mut m, &format!("movegrid={direction}"));
            let actual: Vec<_> = indices
                .into_iter()
                .map(|idx| m.storage(idx as u16))
                .collect();
            assert_eq!(actual, expected, "direction {direction}, input {input:?}");
            assert_eq!(m.storage(22), score);
            assert_eq!(m.storage(24), i16::from(input != expected));
            assert_eq!(m.storage(23), 0);
            assert_eq!(m.storage(20), 0);
        }
    }
}

#[test]
fn score_carry_and_saturation() {
    for (high, low, expected) in [(0, 9999, (1, 1)), (32767, 9999, (32767, 9999))] {
        let mut m = machine();
        set(&mut m, 0, 1);
        set(&mut m, 1, 1);
        set(&mut m, 21, high);
        set(&mut m, 22, low);
        call(&mut m, "movegrid=0");
        assert_eq!((m.storage(21), m.storage(22)), expected);
    }
}

#[test]
fn availability_checks_empty_and_equal_neighbors() {
    for (cells, expected) in [
        ([1, 2, 1, 2, 2, 1, 2, 1, 1, 2, 1, 2, 2, 1, 2, 1], 0),
        ([1, 1, 1, 2, 2, 1, 2, 1, 1, 2, 1, 2, 2, 1, 2, 1], 1),
        ([0, 2, 1, 2, 2, 1, 2, 1, 1, 2, 1, 2, 2, 1, 2, 1], 1),
    ] {
        let mut m = machine();
        for (i, value) in cells.into_iter().enumerate() {
            set(&mut m, i, value);
        }
        m.execute_source("canmove=()").unwrap();
        assert_eq!(m.pop().unwrap(), expected);
        assert_eq!(m.stack(), PREFIX);
        assert_eq!(board(&m), cells);
    }
}

#[test]
fn input_lines_handle_crlf_cr_empty_and_incomplete_eof() {
    for (input, expected) in [
        (&b"W\r\nA\n"[..], vec![87, 65, -1]),
        (&b"W\rA\n"[..], vec![87, 65, -1]),
        (&b"\n\r\n\r\n"[..], vec![0, 0, 0, -1]),
        (&b"R\r\nQ\n"[..], vec![82, 81, -1]),
        (&b"wxyz\n?bad\na\nD"[..], vec![87, 0, 65, -1]),
    ] {
        let mut m = machine();
        m.set_input(Cursor::new(input.to_vec()));
        for value in expected {
            m.execute_source("readcmd=()").unwrap();
            assert_eq!(m.pop().unwrap(), value, "input {input:?}");
            assert_eq!(m.stack(), PREFIX);
        }
    }
}

#[test]
fn cell_centering_and_score_rendering() {
    let mut m = machine();
    for value in [0, 1, 12, 1024] {
        call(&mut m, &format!("tileout={value}"));
    }
    assert_eq!(m.output(), b"   .      1      12    1024 ");
    m.clear_output();
    set(&mut m, 21, 1);
    set(&mut m, 22, 1);
    call(&mut m, "drawgame=()");
    assert!(m.output().starts_with(b"1024\nSCORE: 10001  MOVES: 0\n\n"));
}

#[test]
fn initial_cli_stdout_is_exact() {
    let output = Command::new(env!("CARGO_BIN_EXE_ecvtl"))
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/1024.vtl"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = concat!(
        "1024\nSCORE: 0  MOVES: 0\n\n",
        "+-------+-------+-------+-------+\n|       |       |       |       |\n|   1   |   .   |   .   |   .   |\n|       |       |       |       |\n",
        "+-------+-------+-------+-------+\n|       |       |       |       |\n|   .   |   .   |   1   |   .   |\n|       |       |       |       |\n",
        "+-------+-------+-------+-------+\n|       |       |       |       |\n|   .   |   .   |   .   |   .   |\n|       |       |       |       |\n",
        "+-------+-------+-------+-------+\n|       |       |       |       |\n|   .   |   .   |   .   |   .   |\n|       |       |       |       |\n",
        "+-------+-------+-------+-------+\n\nWASD MOVE  R RESTART  Q QUIT\n> "
    );
    assert_eq!(output.stdout, expected.as_bytes());
}

fn run_game(input: &[u8]) -> Machine {
    let mut m = machine();
    m.set_input(Cursor::new(input.to_vec()));
    set(&mut m, 20, 12345);
    set(&mut m, 26, 0);
    call(&mut m, "gameloop=()");
    m
}

#[test]
fn invalid_lines_and_incomplete_eof_leave_game_state_unchanged() {
    let initial = run_game(b"Q\n");
    for input in [&b"?\n\nQ\n"[..], &b"A"[..]] {
        let actual = run_game(input);
        for i in 0..=25 {
            assert_eq!(
                actual.storage(i),
                initial.storage(i),
                "storage {i}, input {input:?}"
            );
        }
        if input.starts_with(b"?") {
            let mut expected = initial.output().to_vec();
            expected.extend_from_slice(b"INVALID COMMAND\n> INVALID COMMAND\n> ");
            assert_eq!(actual.output(), expected);
        } else {
            assert_eq!(actual.output(), initial.output());
        }
    }
}

#[test]
fn successful_move_draws_twice_and_restart_continues_stream() {
    let moved = run_game(b"A\nQ\n");
    assert_eq!(
        board(&moved),
        [1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        (
            moved.storage(20),
            moved.storage(21),
            moved.storage(22),
            moved.storage(23)
        ),
        (-15869, 0, 0, 1)
    );
    let restarted = run_game(b"A\nR\nQ\n");
    assert_eq!(restarted.storage(20), -21713);
    assert_eq!(
        (
            restarted.storage(21),
            restarted.storage(22),
            restarted.storage(23),
            restarted.storage(24),
            restarted.storage(25)
        ),
        (0, 0, 0, 0, 0)
    );
    assert_eq!(board(&restarted).iter().filter(|&&x| x != 0).count(), 2);
    assert_eq!(
        restarted
            .output()
            .windows(b"1024\nSCORE:".len())
            .filter(|w| *w == b"1024\nSCORE:")
            .count(),
        3
    );
}

#[test]
fn repeated_no_op_does_not_redraw_or_change_state() {
    let before = run_game(b"A\nA\nQ\n");
    let after = run_game(b"A\nA\nA\nQ\n");
    for i in (0..=15).chain(20..=23).chain(25..=26) {
        assert_eq!(after.storage(i), before.storage(i), "storage {i}");
    }
    let mut expected = before.output().to_vec();
    expected.extend_from_slice(b"NO MOVE\n> ");
    assert_eq!(after.output(), expected);
}

#[test]
fn procedures_preserve_registers_outside_transitive_clobbers() {
    for (source, clobbers, returns) in [
        ("random=7", "AB", true),
        ("newtile=()", "ABCDEF", false),
        ("movegrid=0", "GHIJKLMN", false),
        ("tileout=12", "VW", false),
        ("drawgame=()", "IJVW", false),
        ("readcmd=()", "XY", true),
        ("canmove=()", "IJ", true),
        ("newgame=()", "ABCDEF", false),
        ("gameloop=()", "ABCDEFGHIJKLMNOPVWXY", false),
    ] {
        let mut m = machine();
        m.set_input(Cursor::new(b"Q\n".to_vec()));
        for index in 0..26 {
            m.execute_source(&format!(
                "{}={}",
                char::from(b'A' + index),
                1000 + i16::from(index)
            ))
            .unwrap();
        }
        m.execute_source(source).unwrap();
        if returns {
            m.pop().unwrap();
        }
        assert_eq!(m.stack(), PREFIX, "{source}");
        for index in 0..26 {
            let register = char::from(b'A' + index);
            if !clobbers.contains(register) {
                assert_eq!(
                    m.register(index as usize),
                    Some(1000 + i16::from(index)),
                    "{source}: {register}"
                );
            }
        }
    }
}

// Inject only fixture setup after g's initial i call; all game decisions and
// procedures remain the source under test.
fn game_with_position(cells: [i16; 16], moves: i16, input: &[u8]) -> Machine {
    let end = SOURCE.find(MARKER).unwrap();
    let mut setup = String::new();
    for (index, value) in cells.into_iter().enumerate() {
        setup.push_str(&format!(" @={index},{value}"));
    }
    setup.push_str(&format!(" @=23,{moves}"));
    let definitions = SOURCE[..end].replacen(
        "&=gameloop [\n    newgame=()",
        &format!("&=gameloop [\n    newgame=() {setup}"),
        1,
    );
    assert_ne!(definitions, SOURCE[..end]);
    let mut m = Machine::new();
    m.execute_source(&definitions).unwrap();
    for &value in PREFIX {
        m.push(value);
    }
    m.set_input(Cursor::new(input.to_vec()));
    set(&mut m, 20, 12345);
    call(&mut m, "gameloop=()");
    m
}

#[test]
fn winning_move_spawns_before_display_saturates_moves_and_locks_input() {
    let cells = [512, 512, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let won = game_with_position(cells, 32767, b"A\nQ\n");
    assert_eq!(won.storage(0), 1024);
    assert_eq!(won.storage(23), 32767);
    assert_eq!(won.storage(25), 1);
    assert_eq!(won.storage(20), -15869);
    assert_eq!(board(&won).iter().filter(|&&v| v != 0).count(), 2);
    let output = String::from_utf8_lossy(won.output());
    let last_screen = output.rfind("1024\nSCORE:").unwrap();
    let victory = output.rfind("YOU WIN!\n").unwrap();
    assert!(last_screen < victory);
    assert!(output[last_screen..victory].contains("1024"));
    let locked = game_with_position(cells, 32767, b"A\nW\n?\nQ\n");
    for i in (0..=15).chain(20..=26) {
        assert_eq!(locked.storage(i), won.storage(i), "storage {i}");
    }
    let mut expected = won.output().to_vec();
    expected.extend_from_slice(b"R OR Q ONLY\n> R OR Q ONLY\n> ");
    assert_eq!(locked.output(), expected);
    let restarted = game_with_position(cells, 32767, b"A\nR\nQ\n");
    assert_eq!(restarted.storage(20), -21713);
    assert_eq!(
        (
            restarted.storage(21),
            restarted.storage(22),
            restarted.storage(23),
            restarted.storage(25)
        ),
        (0, 0, 0, 0)
    );
}

#[test]
fn final_spawn_causes_game_over_and_terminal_input_is_locked() {
    let cells = [2, 1, 2, 0, 2, 1, 2, 1, 1, 2, 1, 2, 2, 1, 2, 1];
    let lost = game_with_position(cells, 0, b"D\nQ\n");
    assert_eq!(
        board(&lost),
        [1, 2, 1, 2, 2, 1, 2, 1, 1, 2, 1, 2, 2, 1, 2, 1]
    );
    assert_eq!(lost.storage(20), -15869);
    assert_eq!(lost.storage(23), 1);
    assert!(
        lost.output()
            .windows(b"GAME OVER\n".len())
            .any(|w| w == b"GAME OVER\n")
    );
    let locked = game_with_position(cells, 0, b"D\nA\nQ\n");
    for i in (0..=15).chain(20..=26) {
        assert_eq!(locked.storage(i), lost.storage(i), "storage {i}");
    }
    let mut expected = lost.output().to_vec();
    expected.extend_from_slice(b"R OR Q ONLY\n> ");
    assert_eq!(locked.output(), expected);
}

#[test]
fn victory_takes_priority_when_final_board_has_no_moves() {
    let cells = [512, 512, 1, 2, 1, 2, 1, 2, 2, 1, 2, 1, 1, 2, 1, 2];
    let won = game_with_position(cells, 0, b"A\nQ\n");
    assert_eq!(
        board(&won),
        [1024, 1, 2, 1, 1, 2, 1, 2, 2, 1, 2, 1, 1, 2, 1, 2]
    );
    assert!(won.output().windows(9).any(|w| w == b"YOU WIN!\n"));
    assert!(!won.output().windows(10).any(|w| w == b"GAME OVER\n"));
}
