use vtlxx_poc_extended_classic_vtl::{CompileError, Machine, SourceError};

#[test]
fn names_are_complete_and_calls_share_stack_across_nested_blocks() {
    let mut machine = Machine::new();
    machine
        .execute_source(
            "&=a [ ?=\"a\" ] &=ab [ ?=\"b\" ] &=read [ A=~ ] \
         &=reads [ read=~ ] &=reader [ reads=~+1 ] &=eightchr [ %=1 [ reader=~ ] ] \
         a=() ab=() read=2+3 eightchr=7",
        )
        .unwrap();
    assert_eq!(machine.output(), b"ab");
    assert_eq!(machine.register(0), Some(8));
    assert!(machine.stack().is_empty());
}

#[test]
fn rust_accepts_more_than_sixty_four_completed_names() {
    let mut source = String::new();
    for n in 0..65 {
        let name = format!("p{}{}", (b'a' + n / 26) as char, (b'a' + n % 26) as char);
        source.push_str(&format!("&={name} [ A=A+1 ] "));
    }
    for n in 0..65 {
        let name = format!("p{}{}", (b'a' + n / 26) as char, (b'a' + n % 26) as char);
        source.push_str(&format!("{name}=() "));
    }
    let mut machine = Machine::new();
    machine.execute_source(&source).unwrap();
    assert_eq!(machine.register(0), Some(65));
}

#[test]
fn invalid_names_and_unpublished_calls_are_rejected() {
    for source in [
        "&= [ ]",
        "&=abcdefghi [ ]",
        "&=abC [ ]",
        "&=ab1 [ ]",
        "&=ab_cd [ ]",
        "abcdefghi=()",
        "abC=()",
        "ab1=()",
        "ab_cd=()",
        "missing=()",
        "later=() &=later [ ]",
        "&=open [ A=1",
        "&=self [ self=() ]",
    ] {
        let mut machine = Machine::new();
        assert!(
            matches!(machine.execute_source(source), Err(SourceError::Compile(_))),
            "{source}"
        );
        assert_eq!(machine.register(0), Some(0), "{source}");
        assert!(machine.output().is_empty(), "{source}");
    }
}

#[test]
fn completed_long_names_survive_failures_and_stay_machine_local() {
    let mut machine = Machine::new();
    assert!(matches!(
        machine.execute_source("A=7 ?=\"ok\" &=persist [ B=~ ] &=broken [ C=1+ ]"),
        Err(SourceError::Compile(_))
    ));
    assert_eq!(machine.register(0), Some(7));
    assert_eq!(machine.output(), b"ok");
    assert!(matches!(
        machine.execute_source("broken=()"),
        Err(SourceError::Compile(_))
    ));
    assert!(matches!(
        machine.execute_source("&=persist [ ]"),
        Err(SourceError::Compile(_))
    ));
    machine.execute_source("persist=42").unwrap();
    assert_eq!(machine.register(1), Some(42));
    assert_eq!(
        Machine::new().execute_source("persist=()"),
        Err(SourceError::Compile(CompileError::UndefinedBinding))
    );
}

#[test]
fn normal_names_use_complete_name_for_collisions() {
    for source in [
        "&=a [ A=1 ] &=ab [ B=2 ] a=() ab=()",
        "&=ab [ B=2 ] &=a [ A=1 ] a=() ab=()",
    ] {
        let mut machine = Machine::new();
        machine.execute_source(source).unwrap();
        assert_eq!(machine.register(0), Some(1));
        assert_eq!(machine.register(1), Some(2));
    }
    for source in ["&=a [ ] &=a [ ]", "&=ab [ ] &=ab [ ]"] {
        assert!(
            matches!(
                Machine::new().execute_source(source),
                Err(SourceError::Compile(_))
            ),
            "{source}"
        );
    }
}

#[test]
fn existing_builtins_labels_and_named_integers_keep_their_meaning() {
    let mut machine = Machine::new();
    machine
        .execute_source(
            ".VALUE:=3 &=value [ A=.VALUE+2 ] value=() @=4,A B=@(4) %=B==5 [ #=9 ] B=99 ^=9 ?=B",
        )
        .unwrap();
    assert_eq!(machine.register(0), Some(5));
    assert_eq!(machine.register(1), Some(5));
    assert_eq!(machine.output(), b"5");
}
