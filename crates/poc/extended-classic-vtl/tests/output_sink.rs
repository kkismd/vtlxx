use std::{
    cell::RefCell,
    io::{self, Write},
    rc::Rc,
};
use vtlxx_poc_extended_classic_vtl::{Machine, RuntimeError, SourceError};

struct Sink(Rc<RefCell<Vec<u8>>>);
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.0.borrow().is_empty() {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.0.borrow_mut().push(bytes[0]);
        Ok(1)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn output_failure_stops_source_and_preserves_prefix_and_operand() {
    let bytes = Rc::new(RefCell::new(Vec::new()));
    let mut machine = Machine::new();
    machine.set_output(Sink(bytes.clone()));
    assert_eq!(
        machine.execute_source("?=123 A=7"),
        Err(SourceError::Runtime(RuntimeError::OutputError))
    );
    assert_eq!(machine.stack(), &[123]);
    assert_eq!(machine.output(), b"1");
    assert_eq!(&*bytes.borrow(), b"1");
    assert_eq!(machine.register(0), Some(0));
}
