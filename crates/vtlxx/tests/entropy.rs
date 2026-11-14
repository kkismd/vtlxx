use std::{cell::RefCell, io, rc::Rc};
use vtlxx::{Machine, RuntimeError, SourceError};

#[test]
fn apostrophe_is_a_runtime_primary_read_for_register_and_storage_expressions() {
    let calls = Rc::new(RefCell::new(0));
    let observed = calls.clone();
    let mut machine = Machine::new();
    machine.set_entropy_source(move || {
        *observed.borrow_mut() += 1;
        Ok(1)
    });

    machine
        .execute_source("A=' B='+1 C=' D=' E=' @=192,7")
        .unwrap();
    assert_eq!(*calls.borrow(), 1);
    assert_eq!(machine.register(0), Some(16576));
    assert_eq!(machine.register(1), Some(12305));
    assert_eq!(machine.storage(192), 7);
    assert_eq!(machine.register(2), Some(29900));
    assert_eq!(machine.register(3), Some(5893));
    assert_eq!(machine.register(4), Some(23247));
    assert_eq!(machine.stack(), &[]);

    machine.execute_source("A=' B=' C=' D='").unwrap();
    assert_eq!(*calls.borrow(), 1);
    assert_eq!(machine.register(0), Some(27857));
}

#[test]
fn entropy_provider_is_lazy_and_failure_preserves_machine_state() {
    let mut machine = Machine::new();
    let attempts = Rc::new(RefCell::new(0));
    let observed_attempts = attempts.clone();
    machine.set_entropy_source(move || {
        *observed_attempts.borrow_mut() += 1;
        if *observed_attempts.borrow() == 1 {
            return Err(io::Error::other("unavailable"));
        }
        Ok(1)
    });
    machine.push(77);
    machine.execute_source("A=2").unwrap();
    assert_eq!(*attempts.borrow(), 0);

    machine.execute_source("?=\"before\"").unwrap();
    assert_eq!(
        machine.execute_source("B='"),
        Err(SourceError::Runtime(RuntimeError::EntropyError))
    );
    assert_eq!(*attempts.borrow(), 1);
    assert_eq!(machine.stack(), &[77]);
    assert_eq!(machine.register(1), Some(0));
    assert_eq!(machine.output(), b"before");
    machine.execute_source("B='").unwrap();
    assert_eq!(*attempts.borrow(), 2);
    assert_eq!(machine.register(1), Some(16576));
}

#[test]
fn two_entropy_reads_in_one_nested_expression_consume_steps_left_to_right() {
    let mut machine = Machine::new();
    machine.set_entropy_source(|| Ok(1));

    machine.execute_source("A=('+'*2)").unwrap();

    assert_eq!(machine.register(0), Some(-7776));
    assert_eq!(machine.stack(), &[]);
}

#[test]
fn zero_seed_and_ace1_seed_produce_the_same_sequence() {
    let mut zero = Machine::new();
    zero.set_entropy_source(|| Ok(0));
    zero.execute_source("A=' B=' C=' D=' E=' F='").unwrap();

    let mut ace1 = Machine::new();
    ace1.set_entropy_source(|| Ok(-21279));
    ace1.execute_source("A=' B=' C=' D=' E=' F='").unwrap();

    let zero_sequence: Vec<_> = (0..6).map(|index| zero.register(index)).collect();
    let ace1_sequence: Vec<_> = (0..6).map(|index| ace1.register(index)).collect();
    assert_eq!(
        zero_sequence,
        [
            Some(27015),
            Some(30930),
            Some(2970),
            Some(32697),
            Some(2984),
            Some(6343)
        ]
    );
    assert_eq!(ace1_sequence, zero_sequence);
}

#[test]
fn apostrophe_can_supply_a_storage_write_without_changing_prefix_or_other_storage() {
    let calls = Rc::new(RefCell::new(0));
    let observed = calls.clone();
    let mut machine = Machine::new();
    machine.execute_source("@=193,8").unwrap();
    machine.push(91);
    machine.set_entropy_source(move || {
        *observed.borrow_mut() += 1;
        Ok(-1234)
    });

    machine.execute_source("@=192,'").unwrap();

    assert_eq!(*calls.borrow(), 1);
    assert_eq!(machine.storage(192), 14860);
    assert_eq!(machine.storage(193), 8);
    assert_eq!(machine.stack(), &[91]);
}

#[test]
fn entropy_state_is_machine_local_and_provider_replacement_does_not_reseed() {
    let calls = Rc::new(RefCell::new(0));
    let observed = calls.clone();
    let mut first = Machine::new();
    first.set_entropy_source(move || {
        *observed.borrow_mut() += 1;
        Ok(1)
    });
    first.execute_source("A='").unwrap();

    let replacement_calls = Rc::new(RefCell::new(0));
    let replacement_observed = replacement_calls.clone();
    first.set_entropy_source(move || {
        *replacement_observed.borrow_mut() += 1;
        Ok(2)
    });
    first.execute_source("B='").unwrap();
    assert_eq!(first.register(1), Some(12304));
    assert_eq!(*calls.borrow(), 1);
    assert_eq!(*replacement_calls.borrow(), 0);

    let mut second = Machine::new();
    second.set_entropy_source(|| Ok(1));
    second.execute_source("A='").unwrap();
    assert_eq!(second.register(0), Some(16576));
}

#[test]
fn compiling_uninvoked_procedure_does_not_call_entropy_provider() {
    let calls = Rc::new(RefCell::new(0));
    let observed = calls.clone();
    let mut machine = Machine::new();
    machine.set_entropy_source(move || {
        *observed.borrow_mut() += 1;
        Err(io::Error::other("unavailable"))
    });

    machine.execute_source("&=p [ A=' ]").unwrap();
    assert_eq!(*calls.borrow(), 0);
    assert_eq!(machine.register(0), Some(0));

    assert_eq!(
        machine.execute_source("p=()"),
        Err(SourceError::Runtime(RuntimeError::EntropyError))
    );
    assert_eq!(*calls.borrow(), 1);
    assert_eq!(machine.register(0), Some(0));
}

#[test]
fn compile_failure_does_not_call_entropy_provider() {
    let calls = Rc::new(RefCell::new(0));
    let observed = calls.clone();
    let mut machine = Machine::new();
    machine.set_entropy_source(move || {
        *observed.borrow_mut() += 1;
        Ok(1)
    });

    assert!(matches!(
        machine.execute_source("A=' invalid"),
        Err(SourceError::Compile(_))
    ));
    assert_eq!(*calls.borrow(), 0);
}

#[test]
fn apostrophe_cannot_be_written_or_applied() {
    for source in ["'=1", "A='(1)"] {
        assert!(matches!(
            Machine::new().execute_source(source),
            Err(SourceError::Compile(_))
        ));
    }
}
