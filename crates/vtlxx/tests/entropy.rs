use std::{cell::RefCell, io, rc::Rc};
use vtlxx::{Machine, RuntimeError, SourceError};

#[test]
fn apostrophe_is_a_runtime_primary_read_for_register_and_storage_expressions() {
    let calls = Rc::new(RefCell::new(0));
    let observed = calls.clone();
    let mut machine = Machine::new();
    machine.set_entropy_source(move || {
        let value = [-32768, -1, 0, 1, 32767][*observed.borrow()];
        *observed.borrow_mut() += 1;
        Ok(value)
    });

    machine
        .execute_source("A=' B='+1 C=' D=' E=' @=192,7")
        .unwrap();
    assert_eq!(*calls.borrow(), 5);
    assert_eq!(machine.register(0), Some(-32768));
    assert_eq!(machine.register(1), Some(0));
    assert_eq!(machine.storage(192), 7);
    assert_eq!(machine.register(2), Some(0));
    assert_eq!(machine.register(3), Some(1));
    assert_eq!(machine.register(4), Some(32767));
    assert_eq!(machine.stack(), &[]);

    let repeats = Rc::new(RefCell::new(0));
    let observed = repeats.clone();
    machine.set_entropy_source(move || {
        *observed.borrow_mut() += 1;
        Ok(32767)
    });
    machine.execute_source("A=' B=' C=' D='").unwrap();
    assert_eq!(*repeats.borrow(), 4);
    assert_eq!(machine.register(0), Some(32767));
}

#[test]
fn entropy_provider_is_lazy_and_failure_preserves_machine_state() {
    let calls = Rc::new(RefCell::new(0));
    let observed = calls.clone();
    let mut machine = Machine::new();
    machine.set_entropy_source(move || {
        *observed.borrow_mut() += 1;
        Err(io::Error::other("unavailable"))
    });
    machine.push(77);
    machine.execute_source("A=2").unwrap();
    assert_eq!(*calls.borrow(), 0);

    machine.execute_source("?=\"before\"").unwrap();
    assert_eq!(
        machine.execute_source("B='"),
        Err(SourceError::Runtime(RuntimeError::EntropyError))
    );
    assert_eq!(*calls.borrow(), 1);
    assert_eq!(machine.stack(), &[77]);
    assert_eq!(machine.register(1), Some(0));
    assert_eq!(machine.output(), b"before");
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
