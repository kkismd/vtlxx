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
        .execute_source("#=10\nA=99\n^=10 A=3\n^=20 A=A-1 &=A #=20")
        .unwrap();
    assert_eq!(machine.register(0), Some(0));
    assert_eq!(
        machine.execute_source("#=10"),
        Err(SourceError::Compile(CompileError::Builder))
    );
    assert_eq!(machine.register(0), Some(0));
}

#[test]
fn conditionals_own_the_remainder_of_their_line() {
    let mut machine = Machine::new();
    machine
        .execute_source("A=0 &=A B=1 C=1\nD=2\n&=1 &=0 E=3 F=4\n&=1 &=1 G=5\n&=0\nH=6")
        .unwrap();
    for (index, expected) in [(1, 0), (2, 0), (3, 2), (4, 0), (5, 0), (6, 5), (7, 6)] {
        assert_eq!(machine.register(index), Some(expected));
    }
}

#[test]
fn stack_connection_uses_existing_top_without_new_value() {
    let mut machine = Machine::new();
    machine.push(10);
    machine.execute_source("A=[+2").unwrap();
    assert_eq!(machine.register(0), Some(12));
    assert!(machine.stack().is_empty());

    machine.push(10);
    machine.execute_source("[=[+1 B=[").unwrap();
    assert_eq!(machine.register(1), Some(11));
    assert!(machine.stack().is_empty());

    machine.push(0);
    machine.execute_source("C=[==0").unwrap();
    assert_eq!(machine.register(2), Some(1));
    machine.execute_source("[=7,8").unwrap();
    assert_eq!(machine.stack(), &[7, 8]);

    machine.pop();
    machine.pop();
    assert_eq!(
        machine.execute_source("D=["),
        Err(SourceError::Runtime(RuntimeError::StackUnderflow))
    );
    assert_eq!(machine.register(3), Some(0));
}

#[test]
fn output_sugar_preserves_quoted_space_and_semicolon() {
    let mut machine = Machine::new();
    machine
        .execute_source("?=-12 $=32 ?=\"hello; world\" ?= ; comment\n$=65")
        .unwrap();
    assert_eq!(machine.output(), b"-12 hello; world\nA");
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
