use vtlxx_poc_extended_classic_vtl::{CompileError, Machine, RuntimeError, SourceError};

#[test]
fn registers_arithmetic_grouping_and_comparisons() {
    let mut machine = Machine::new();
    machine
        .execute_source(
            "A=2 B=3 C=A+B*4 D=A+(B*4) E=-7/3 F=-7%3\nG=A==2 H=A!=2 I=A<=2 J=A>=3 K=A<B",
        )
        .unwrap();
    for (index, value) in [2, 3, 20, 14, -2, -1, 1, 0, 1, 0, 1]
        .into_iter()
        .enumerate()
    {
        assert_eq!(machine.register(index), Some(value));
    }
    assert!(machine.stack().is_empty());
}

#[test]
fn storage_and_comma_operands_run_left_to_right() {
    let mut machine = Machine::new();
    machine
        .execute_source("I=-1 @=I,41 A=@(I) B=@(I)+1")
        .unwrap();
    assert_eq!(machine.storage(u16::MAX), 41);
    assert_eq!(machine.register(0), Some(41));
    assert_eq!(machine.register(1), Some(42));
}

#[test]
fn labels_work_across_lines_within_one_source_owner() {
    let mut machine = Machine::new();
    machine
        .execute_source("#=10\nA=99\n^=10 A=3\n^=20 A=A-1 %=A [ #=20 ]")
        .unwrap();
    assert_eq!(machine.register(0), Some(0));
    assert_eq!(
        machine.execute_source("#=10"),
        Err(SourceError::Compile(CompileError::Builder))
    );
    assert_eq!(machine.register(0), Some(0));
}

#[test]
fn one_arm_if_leaves_following_statements_outside_the_block() {
    let mut machine = Machine::new();
    machine
        .execute_source("A=0 %=A [ B=1 C=1 ] D=2\n%=1 [ %=0 [ E=3 ] F=4 ]\n%=1 [ %=1 [ G=5 ] ]\n%=0 [ H=99 ] H=6")
        .unwrap();
    for (index, expected) in [(1, 0), (2, 0), (3, 2), (4, 0), (5, 4), (6, 5), (7, 6)] {
        assert_eq!(machine.register(index), Some(expected));
    }
}

#[test]
fn if_executes_one_or_two_bracket_blocks() {
    for (source, expected) in [
        ("%=1 [ A=1 ]", 1),
        ("%=0 [ A=1 ]", 0),
        ("%=1 [ A=1 ] [ A=2 ]", 1),
        ("%=0 [ A=1 ] [ A=2 ]", 2),
        ("%=1 [ A=1 A=A+1 ]", 2),
        ("%=0 [ A=1 A=A+1 ]", 0),
        ("%=1 [ A=1 A=A+1 ] [ A=3 A=A+1 ]", 2),
        ("%=0 [ A=1 A=A+1 ] [ A=3 A=A+1 ]", 4),
    ] {
        let mut machine = Machine::new();
        machine.execute_source(source).unwrap();
        assert_eq!(machine.register(0), Some(expected), "{source}");
        assert!(machine.stack().is_empty(), "{source}");
    }
}

#[test]
fn if_else_crosses_comments_and_blank_lines_and_empty_blocks_are_valid() {
    for (condition, expected) in [(1, 1), (0, 2)] {
        let mut machine = Machine::new();
        let source = format!("%={condition} [ A=1 ]\n; between arms\n\n[ A=2 ]");
        machine.execute_source(&source).unwrap();
        assert_eq!(machine.register(0), Some(expected));
    }

    let mut machine = Machine::new();
    machine
        .execute_source("%=1 [ ] [ A=2 ] %=0 [ A=3 ] [ ]")
        .unwrap();
    assert_eq!(machine.register(0), Some(0));
}

#[test]
fn third_sibling_block_is_outside_the_if_and_is_invalid_as_a_bare_form() {
    let mut machine = Machine::new();
    assert_eq!(
        machine.execute_source("%=1 [ A=1 ] [ A=2 ] [ A=3 ]"),
        Err(SourceError::Compile(CompileError::Syntax))
    );
    assert_eq!(machine.register(0), Some(0));
}

#[test]
fn nested_if_uses_explicit_block_ownership() {
    for (source, expected) in [
        ("%=1 [ %=0 [ A=1 ] [ A=2 ] ] [ A=3 ]", 2),
        ("%=0 [ %=1 [ A=1 ] [ A=2 ] ] [ A=3 ]", 3),
        ("%=1 [ %=0 [ A=1 ] ] [ A=3 ]", 0),
        ("%=0 [ %=0 [ A=1 ] ] [ A=3 ]", 3),
    ] {
        let mut machine = Machine::new();
        machine.execute_source(source).unwrap();
        assert_eq!(machine.register(0), Some(expected), "{source}");
    }
}

#[test]
fn if_else_blocks_cross_lines_and_share_the_owner_labels() {
    let mut machine = Machine::new();
    machine
        .execute_source("%=1 [\n #=10 A=99\n ] [\n A=2\n ] ^=10 B=3")
        .unwrap();
    assert_eq!(machine.register(0), Some(0));
    assert_eq!(machine.register(1), Some(3));

    let mut machine = Machine::new();
    machine
        .execute_source("&=p [ %=0 [ A=1 ] [ #=10 A=99 ] A=3 ^=10 B=3 ] p=()")
        .unwrap();
    assert_eq!(machine.register(0), Some(0));
    assert_eq!(machine.register(1), Some(3));
}

#[test]
fn bracket_blocks_work_for_if_and_while() {
    let mut machine = Machine::new();
    machine.execute_source("%=1 [\n A=1\n]").unwrap();
    assert_eq!(machine.register(0), Some(1));

    let mut machine = Machine::new();
    machine.execute_source("%=1 [ B=1 ] [ B=2 ]").unwrap();
    assert_eq!(machine.register(1), Some(1));
    let mut machine = Machine::new();
    machine.execute_source("%=0 [ B=1 ] [ B=2 ]").unwrap();
    assert_eq!(machine.register(1), Some(2));

    let mut machine = Machine::new();
    machine
        .execute_source("A=0 *=() [ ~=A<3 ] [ A=A+1 ]")
        .unwrap();
    assert_eq!(machine.register(0), Some(3));
    assert!(machine.stack().is_empty());
}

#[test]
fn bracket_blocks_nest_and_allow_same_line_forms() {
    let mut machine = Machine::new();
    machine
        .execute_source("%=1 [ %=1 [ A=1 ] [ A=A+1 ] ]")
        .unwrap();
    assert_eq!(machine.register(0), Some(1));

    let mut machine = Machine::new();
    machine.execute_source("%=0 [ B=1 ] [ B=2 ]").unwrap();
    assert_eq!(machine.register(0), Some(0));
    assert_eq!(machine.register(1), Some(2));
}

#[test]
fn bracket_delimiters_preserve_stack_surface_quotes_and_comments() {
    let mut machine = Machine::new();
    machine.push(10);
    machine.execute_source("~=~+1 B=~").unwrap();
    assert_eq!(machine.register(1), Some(11));

    machine.push(10);
    machine
        .execute_source("A=~+2 %=1 [ ?=\"[]\" ; ] ignored\n C=3 ]")
        .unwrap();
    assert_eq!(machine.register(0), Some(12));
    assert_eq!(machine.register(2), Some(3));
    assert_eq!(machine.output(), b"[]");
}

#[test]
fn bracket_block_errors_are_atomic() {
    for source in [
        "[ A=1 ]",
        "]",
        "%=1 [ A=1",
        "%=1 [ A=1 =|",
        "%=1 |= A=1 ] =|",
        "%=1 [ A=1 =| ] [ B=2 ]",
        "%=1 [ A=1 ] |= B=2 =|",
        "%=1 |= A=1 =| [ B=2 ]",
        "*=() |= ~=0 =| [ A=1 ]",
        "*=() [ ~=0 ] |= A=1 =|",
        "*=() [ ~=0 ] [ A=1",
        "%=1 [ A=1 ] [ B=2",
        "%=1 [ A=1 ] |= B=2 ]",
    ] {
        let mut machine = Machine::new();
        assert!(
            matches!(machine.execute_source(source), Err(SourceError::Compile(_))),
            "{source}"
        );
        assert_eq!(machine.register(0), Some(0), "{source}");
        assert_eq!(machine.register(1), Some(0), "{source}");
    }
}

#[test]
fn if_rejects_legacy_conditionals_and_ordinary_arms() {
    for source in [
        "&=1 A=1",
        "&=1 [ A=1 ]",
        "%=1 A=1 A=2",
        "%=0 A=1 [ A=2 ]",
        "%=1 [ A=1 ] A=2 [ A=3 ]",
    ] {
        let mut machine = Machine::new();
        assert_eq!(
            machine.execute_source(source),
            Err(SourceError::Compile(CompileError::Syntax)),
            "{source}"
        );
        assert_eq!(machine.register(0), Some(0), "{source}");
    }
}

#[test]
fn if_requires_then_block_and_preserves_remainder() {
    for source in ["%=1", "%=1 A=1", "%=1 A=1 A=2", "%=1 A=1 [ A=2 ]"] {
        let mut machine = Machine::new();
        assert!(
            matches!(machine.execute_source(source), Err(SourceError::Compile(_))),
            "{source}"
        );
        assert_eq!(machine.register(0), Some(0), "{source}");
    }
    let mut machine = Machine::new();
    machine.execute_source("%=7%3 [ A=7%3 ] [ A=0 ]").unwrap();
    assert_eq!(machine.register(0), Some(1));
}

#[test]
fn while_rechecks_predicate_and_handles_zero_iterations() {
    let mut machine = Machine::new();
    machine
        .execute_source("A=0 B=0 *=() [ ~=A<3 ] [ A=A+1 B=B+2 ] C=A")
        .unwrap();
    assert_eq!(machine.register(0), Some(3));
    assert_eq!(machine.register(1), Some(6));
    assert_eq!(machine.register(2), Some(3));
    assert!(machine.stack().is_empty());

    let mut machine = Machine::new();
    machine
        .execute_source("A=3 *=() [ ~=A<3 ] [ A=99 ] B=7")
        .unwrap();
    assert_eq!(machine.register(0), Some(3));
    assert_eq!(machine.register(1), Some(7));
}

#[test]
fn nested_while_and_following_source_keep_their_own_arguments() {
    let mut machine = Machine::new();
    machine
        .execute_source("A=0 B=0 *=() [ ~=A<2 ] [ C=0 *=() [ ~=C<3 ] [ B=B+1 C=C+1 ] A=A+1 ] D=9")
        .unwrap();
    for (index, expected) in [(0, 2), (1, 6), (2, 3), (3, 9)] {
        assert_eq!(machine.register(index), Some(expected));
    }
    assert!(machine.stack().is_empty());
}

#[test]
fn while_uses_shared_stack_without_cleanup() {
    let mut machine = Machine::new();
    machine.push(7);
    machine
        .execute_source("A=0 *=() [ ~=99,A<2 ] [ ~=88 A=A+1 ]")
        .unwrap();
    assert_eq!(machine.register(0), Some(2));
    assert_eq!(machine.stack(), &[7, 99, 88, 99, 88, 99]);

    let mut machine = Machine::new();
    machine.push(0);
    machine.execute_source("*=() [ A=1 ] [ A=2 ]").unwrap();
    assert_eq!(machine.register(0), Some(1));
    assert!(machine.stack().is_empty());

    let mut machine = Machine::new();
    assert_eq!(
        machine.execute_source("*=() [ A=1 ] [ A=2 ]"),
        Err(SourceError::Runtime(RuntimeError::StackUnderflow))
    );
    assert_eq!(machine.register(0), Some(1));
}

#[test]
fn while_requires_two_blocks_and_preserves_multiplication() {
    for source in [
        "*=()",
        "*=() [ ~=1 ]",
        "*=() ~=1 |= A=1 =|",
        "*=() [ ~=1 ] A=1",
        "*=1 [ ~=1 ] [ A=1 ]",
        "*=() [ ~=1 ] [ A=1",
    ] {
        let mut machine = Machine::new();
        assert!(
            matches!(machine.execute_source(source), Err(SourceError::Compile(_))),
            "{source}"
        );
        assert_eq!(machine.register(0), Some(0), "{source}");
    }
    let mut machine = Machine::new();
    machine
        .execute_source("A=2*3 *=() [ ~=0 ] [ A=99 ] B=A*4")
        .unwrap();
    assert_eq!(machine.register(0), Some(6));
    assert_eq!(machine.register(1), Some(24));
}

#[test]
fn anonymous_blocks_cross_lines_and_stop_at_the_matching_close() {
    let mut machine = Machine::new();
    machine
        .execute_source("%=1 [\n A=1\n %=0 [ A=9 ]\n B=2\n] C=3\n%=0 [ D=4 ] E=5")
        .unwrap();
    for (index, expected) in [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5)] {
        assert_eq!(machine.register(index), Some(expected));
    }
}

#[test]
fn block_delimiters_in_strings_and_comments_are_ordinary_text() {
    let mut machine = Machine::new();
    machine
        .execute_source("%=1 [ ?=\"[ ]\" ; [ ]\n A=2 ] B=3")
        .unwrap();
    assert_eq!(machine.output(), b"[ ]");
    assert_eq!(machine.register(0), Some(2));
    assert_eq!(machine.register(1), Some(3));
}

#[test]
fn block_and_surrounding_source_share_numeric_labels() {
    let mut machine = Machine::new();
    machine
        .execute_source("#=10 %=1 [ ^=20 A=99 #=30 ] ^=10 A=3 #=20 ^=30")
        .unwrap();
    assert_eq!(machine.register(0), Some(99));

    let mut machine = Machine::new();
    machine
        .execute_source("A=0 ^=1 A=A+1 %=A<3 [ #=1 ]\n%=1 [ #=2 A=99 ] ^=2 B=7")
        .unwrap();
    assert_eq!(machine.register(0), Some(3));
    assert_eq!(machine.register(1), Some(7));
}

#[test]
fn block_in_named_handler_uses_its_handlers_label_namespace() {
    let mut machine = Machine::new();
    machine
        .execute_source("&=p [ #=10 %=1 [ ^=20 A=99 #=30 ] ^=10 A=3 #=20 ^=30 ] p=()")
        .unwrap();
    assert_eq!(machine.register(0), Some(99));
}

#[test]
fn malformed_or_bare_blocks_and_duplicate_owner_labels_fail() {
    for source in [
        "|= A=1 =|",
        "=|",
        "%=1 |= A=1",
        "%=1 [ [ A=1 ]",
        "%=1 [ A=1 ] ]",
        "%=1 [ A=1+ ]",
        "^=1 %=1 [ ^=1 ]",
        "%=1 [ ^=1 ] ^=1",
    ] {
        let mut machine = Machine::new();
        assert!(
            matches!(machine.execute_source(source), Err(SourceError::Compile(_))),
            "{source}"
        );
        assert_eq!(machine.register(0), Some(0), "{source}");
    }
}

#[test]
fn stack_connection_uses_existing_top_without_new_value() {
    let mut machine = Machine::new();
    machine.execute_source("~=1").unwrap();
    assert_eq!(machine.stack(), &[1]);
    machine.pop();

    machine.push(10);
    machine.execute_source("A=~+2").unwrap();
    assert_eq!(machine.register(0), Some(12));
    assert!(machine.stack().is_empty());

    machine.push(10);
    machine.execute_source("~=~+1 B=~").unwrap();
    assert_eq!(machine.register(1), Some(11));
    assert!(machine.stack().is_empty());

    machine.push(0);
    machine.execute_source("C=~==0").unwrap();
    assert_eq!(machine.register(2), Some(1));
    machine.execute_source("~=7,8").unwrap();
    assert_eq!(machine.stack(), &[7, 8]);

    machine.pop();
    machine.pop();
    assert_eq!(
        machine.execute_source("D=~"),
        Err(SourceError::Runtime(RuntimeError::StackUnderflow))
    );
    assert_eq!(machine.register(3), Some(0));
}

#[test]
fn stack_marker_is_restricted_to_output_and_rhs_root() {
    for source in [
        "[=1", "A=[", "A=[+1", "A=1+~", "A=(~+1)", "A=X,~+1", "A=1~2", "A=~+~", "~=1+~", "~=()",
    ] {
        let mut machine = Machine::new();
        machine.push(7);
        assert_eq!(
            machine.execute_source(source),
            Err(SourceError::Compile(CompileError::Syntax)),
            "{source}"
        );
        assert_eq!(machine.stack(), &[7], "{source}");
        assert_eq!(machine.register(0), Some(0), "{source}");
    }
}

#[test]
fn stack_connection_runs_inside_nested_blocks() {
    let mut machine = Machine::new();
    machine
        .execute_source("%=1 [ %=1 [ ~=4 A=~+2 ~=A ] ]")
        .unwrap();
    assert_eq!(machine.register(0), Some(6));
    assert_eq!(machine.stack(), &[6]);
}

#[test]
fn stack_input_calls_a_user_defined_write_handler() {
    let mut machine = Machine::new();
    machine.execute_source("&=p [ B=~ ] ~=9 p=~").unwrap();
    assert_eq!(machine.register(1), Some(9));
    assert!(machine.stack().is_empty());
}

#[test]
fn output_sugar_preserves_quoted_space_and_semicolon() {
    let mut machine = Machine::new();
    machine
        .execute_source("?=-12 $=32 ?=\"hello; world\" ?=() ; comment\n$=65")
        .unwrap();
    assert_eq!(machine.output(), b"-12 hello; world\nA");
}

#[test]
fn zero_operand_call_preserves_the_caller_stack_for_the_handler() {
    let mut machine = Machine::new();
    machine.execute_source("&=q [ ?=\"called\" ]").unwrap();
    machine.push(17);
    machine.execute_source("q=()").unwrap();
    assert_eq!(machine.output(), b"called");
    assert_eq!(machine.stack(), &[17]);

    machine.execute_source("&=p [ A=~ ] p=()").unwrap();
    assert_eq!(machine.register(0), Some(17));
    assert!(machine.stack().is_empty());
}

#[test]
fn zero_operand_call_uses_the_existing_callee_stack_contract() {
    let mut machine = Machine::new();
    machine.execute_source("&=q [ A=~ ]").unwrap();
    assert_eq!(
        machine.execute_source("q=()"),
        Err(SourceError::Runtime(RuntimeError::StackUnderflow))
    );
    assert!(machine.stack().is_empty());

    assert_eq!(
        machine.execute_source("X=()"),
        Err(SourceError::Runtime(RuntimeError::StackUnderflow))
    );
    machine.push(23);
    machine.execute_source("X=()").unwrap();
    assert_eq!(machine.register(23), Some(23));
    assert!(machine.stack().is_empty());
}

#[test]
fn newline_requires_explicit_zero_operand_marker() {
    let mut machine = Machine::new();
    machine.execute_source("?=()").unwrap();
    assert_eq!(machine.output(), b"\n");
    assert_eq!(
        machine.execute_source("?="),
        Err(SourceError::Compile(CompileError::Syntax))
    );
    assert_eq!(machine.output(), b"\n");
}

#[test]
fn compile_failure_keeps_all_machine_data_and_output() {
    let mut machine = Machine::new();
    machine.push(17);
    machine.execute_source("A=5 @=1,9 ?=\"old\"").unwrap();
    let before = machine.output().to_vec();
    for bad in ["A=99 ?=\"new\" @=2,3 B=1+", "A=99 #=9", "A=99 ?=\"open"] {
        assert!(matches!(
            machine.execute_source(bad),
            Err(SourceError::Compile(_))
        ));
        assert_eq!(machine.register(0), Some(5));
        assert_eq!(machine.storage(1), 9);
        assert_eq!(machine.storage(2), 0);
        assert_eq!(machine.stack(), &[17]);
        assert_eq!(machine.output(), before);
    }
}

#[test]
fn runtime_error_keeps_prior_effects_and_pending_operands() {
    let mut machine = Machine::new();
    assert_eq!(
        machine.execute_source("A=4 ?=\"ok\" B=6/0"),
        Err(SourceError::Runtime(RuntimeError::DivisionByZero))
    );
    assert_eq!(machine.register(0), Some(4));
    assert_eq!(machine.output(), b"ok");
    assert_eq!(machine.stack(), &[6, 0]);
    machine.execute_source("C=9").unwrap();
    assert_eq!(machine.register(2), Some(9));
    assert_eq!(machine.stack(), &[6, 0]);
}

#[test]
fn invalid_source_forms_are_compile_errors() {
    let mut machine = Machine::new();
    for bad in [
        "A =1",
        "A= 1",
        "A=1 +2",
        "A=",
        "A=-B",
        "A=32768",
        "A=-32769",
        "A=1=1",
        "A=1+",
        "A=(1+2",
        "A=1+2)",
        "A=1,",
        "A=1+()",
        "A=(())",
        "A=@()",
        "A=@(1)(2)",
        "A=1+[",
        "A=([+1)",
        "A=X,[+1",
        "A=@([+1)",
        "^=-1",
        "#=32768",
        "^=1 ^=1",
        "A=@",
        "A=a",
        "a=1",
        "?=\"a\"tail",
    ] {
        assert!(
            matches!(machine.execute_source(bad), Err(SourceError::Compile(_))),
            "{bad}"
        );
    }
}

#[test]
fn definition_flushes_prior_statements_and_is_callable_in_same_and_later_inputs() {
    let mut machine = Machine::new();
    machine.execute_source("A=7 &=p [ B=~ ?=B ] p=42").unwrap();
    assert_eq!(machine.register(0), Some(7));
    assert_eq!(machine.output(), b"42");
    machine.execute_source("p=A").unwrap();
    assert_eq!(machine.output(), b"427");
    assert!(machine.stack().is_empty());
}

#[test]
fn empty_write_definition_is_completed_and_callable() {
    let mut machine = Machine::new();
    machine.execute_source("&=p [ ] p=()").unwrap();
    assert_eq!(machine.stack(), &[]);
    assert_eq!(machine.register(0), Some(0));
}

#[test]
fn multi_value_handler_uses_shared_stack_in_operand_order() {
    let mut machine = Machine::new();
    machine
        .execute_source("&=p [ B=~ A=~ ?=A ?=B ] p=10,20")
        .unwrap();
    assert_eq!(machine.output(), b"1020");
    assert!(machine.stack().is_empty());

    machine
        .execute_source("&=q [ B=~+1 A=~ ?=A ?=B ] q=5,8")
        .unwrap();
    assert_eq!(machine.output(), b"102059");
}

#[test]
fn handler_stack_connection_and_return_leave_unconsumed_values() {
    let mut machine = Machine::new();
    machine
        .execute_source("&=p [ A=~+2 ~=A,9 B=~ ] p=3")
        .unwrap();
    assert_eq!(machine.register(0), Some(5));
    assert_eq!(machine.register(1), Some(9));
    assert_eq!(machine.stack(), &[5]);
}

#[test]
fn published_handlers_call_each_other_with_owner_local_labels() {
    let mut machine = Machine::new();
    machine
        .execute_source("&=p [ #=2 ?=99 ^=2 A=~ ?=A ] &=q [ ^=1 p=4 %=0 [ #=1 ]\n] q=8")
        .unwrap();
    assert_eq!(machine.output(), b"4");
    assert_eq!(machine.stack(), &[8]);
}

#[test]
fn handler_backward_branch_repeats_within_its_own_body() {
    let mut machine = Machine::new();
    machine
        .execute_source("&=p [ A=~ ^=1 ?=A A=A-1 %=A [ #=1 ]\n] p=3")
        .unwrap();
    assert_eq!(machine.output(), b"321");
}

#[test]
fn malformed_definitions_do_not_publish_or_process_later_source() {
    for bad in [
        "|=p A=1 p=|",
        "&=P [ A=1 ]",
        "&=pp [ A=1 ]",
        "&=p? [ A=1 ]",
        "&= A=1",
        "&=p A=1",
        "&=p [ A=1",
        "&=p [ &=q [ ] ]",
        "%=1 [ &=q [ ] ]",
        "&=1 [ ]",
        "&=p [ A=1+ ]",
        "&=p [ #=9 ]",
        "&=p [ p=1 ]",
        "&=p [ q=1 ] &=q [ ]",
    ] {
        let mut machine = Machine::new();
        let source = format!("{bad} B=9");
        assert!(
            matches!(
                machine.execute_source(&source),
                Err(SourceError::Compile(_))
            ),
            "{bad}"
        );
        assert_eq!(machine.register(0), Some(0), "{bad}");
        assert_eq!(machine.register(1), Some(0), "{bad}");
        assert_eq!(
            machine.execute_source("p=3"),
            Err(SourceError::Compile(CompileError::UndefinedBinding)),
            "{bad}"
        );
    }
}

#[test]
fn committed_publication_survives_later_failure_and_duplicate_is_rejected() {
    let mut machine = Machine::new();
    assert_eq!(
        machine.execute_source("&=p [ A=~ ] &=q [ B=1+ ] C=9"),
        Err(SourceError::Compile(CompileError::Syntax))
    );
    machine.execute_source("p=12").unwrap();
    assert_eq!(machine.register(0), Some(12));
    assert_eq!(machine.register(2), Some(0));
    assert!(matches!(
        machine.execute_source("&=p [ ]"),
        Err(SourceError::Compile(_))
    ));
    machine.execute_source("p=13").unwrap();
    assert_eq!(machine.register(0), Some(13));
}

#[test]
fn definition_failure_keeps_prior_top_level_effects() {
    let mut machine = Machine::new();
    assert_eq!(
        machine.execute_source("A=7 ?=\"ok\" &=p [ B=1+ ] C=9"),
        Err(SourceError::Compile(CompileError::Syntax))
    );
    assert_eq!(machine.register(0), Some(7));
    assert_eq!(machine.register(2), Some(0));
    assert_eq!(machine.output(), b"ok");
    assert_eq!(
        machine.execute_source("p=1"),
        Err(SourceError::Compile(CompileError::UndefinedBinding))
    );
}

#[test]
fn definition_boundary_isolates_top_level_labels_and_runtime_failure_stops_processing() {
    let mut machine = Machine::new();
    assert_eq!(
        machine.execute_source("^=1 &=p [ ] #=1"),
        Err(SourceError::Compile(CompileError::Builder))
    );
    machine.execute_source("p=4").unwrap();
    assert_eq!(machine.stack(), &[4]);
    machine.pop();

    assert_eq!(
        machine.execute_source("A=5 &=q [ B=~ C=~ ] q=6 C=9"),
        Err(SourceError::Runtime(RuntimeError::StackUnderflow))
    );
    assert_eq!(machine.register(0), Some(5));
    assert_eq!(machine.register(2), Some(0));
    machine.execute_source("D=7").unwrap();
    assert_eq!(machine.register(3), Some(7));
}

#[test]
fn runtime_failure_before_header_prevents_publication() {
    let mut machine = Machine::new();
    assert_eq!(
        machine.execute_source("A=5 B=~ &=p [ ] p=1"),
        Err(SourceError::Runtime(RuntimeError::StackUnderflow))
    );
    assert_eq!(machine.register(0), Some(5));
    assert_eq!(
        machine.execute_source("p=1"),
        Err(SourceError::Compile(CompileError::UndefinedBinding))
    );
}
