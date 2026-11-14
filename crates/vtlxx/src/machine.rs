use std::{
    collections::HashMap,
    io::{self, Read, Write},
};

use crate::{
    Cell,
    binding::{BindingError, Bindings, SourceRole},
    builder::CompletedBody,
    executable::{Executable, ExecutableBody, ExecutableId, Instruction, RuntimeError},
    primitive::Primitive,
    source::{SourceError, execute_source},
};

const REGISTER_COUNT: usize = 26;
const STORAGE_SIZE: usize = 1 << 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Continuation {
    executable: ExecutableId,
    instruction: usize,
}

pub struct Machine {
    pub(crate) registers: [Cell; REGISTER_COUNT],
    pub(crate) storage: Box<[Cell; STORAGE_SIZE]>,
    pub(crate) value_stack: Vec<Cell>,
    executables: Vec<Executable>,
    bindings: Bindings,
    long_writes: HashMap<String, ExecutableId>,
    builtins: Vec<(Primitive, ExecutableId)>,
    pub(crate) output: Vec<u8>,
    input: Box<dyn Read>,
    input_eof: bool,
    entropy_source: Box<dyn FnMut() -> io::Result<Cell>>,
    output_sink: Option<Box<dyn Write>>,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    pub fn execute_source(&mut self, source: &str) -> Result<(), SourceError> {
        execute_source(self, source)
    }

    pub fn new() -> Self {
        let mut machine = Self {
            registers: [0; REGISTER_COUNT],
            storage: Box::new([0; STORAGE_SIZE]),
            value_stack: Vec::new(),
            executables: Vec::new(),
            bindings: Bindings::default(),
            long_writes: HashMap::new(),
            builtins: Vec::new(),
            output: Vec::new(),
            input: Box::new(io::empty()),
            input_eof: false,
            entropy_source: Box::new(default_entropy),
            output_sink: None,
        };
        machine.seed_builtins();
        machine
    }

    /// Replace the runtime input stream for this machine session.
    pub fn set_input(&mut self, input: impl Read + 'static) {
        self.input = Box::new(input);
        self.input_eof = false;
    }

    /// Replace the environment source used by the `'` primary read.
    /// The source is called once for each runtime evaluation.
    pub fn set_entropy_source(&mut self, source: impl FnMut() -> io::Result<Cell> + 'static) {
        self.entropy_source = Box::new(source);
    }

    pub(crate) fn read_entropy(&mut self) -> Result<Cell, RuntimeError> {
        (self.entropy_source)().map_err(|_| RuntimeError::EntropyError)
    }

    /// Send runtime output to a sink as well as retaining it in the output buffer.
    pub fn set_output(&mut self, output: impl Write + 'static) {
        self.output_sink = Some(Box::new(output));
    }

    pub(crate) fn emit(&mut self, mut bytes: &[u8]) -> Result<(), RuntimeError> {
        if let Some(sink) = &mut self.output_sink {
            while !bytes.is_empty() {
                match sink.write(bytes) {
                    Ok(0) => return Err(RuntimeError::OutputError),
                    Ok(n) => {
                        self.output.extend_from_slice(&bytes[..n]);
                        bytes = &bytes[n..];
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => return Err(RuntimeError::OutputError),
                }
            }
            sink.flush().map_err(|_| RuntimeError::OutputError)?;
        } else {
            self.output.extend_from_slice(bytes);
        }
        Ok(())
    }

    pub(crate) fn read_char(&mut self) -> Result<Cell, RuntimeError> {
        if self.input_eof {
            return Ok(-1);
        }
        let mut byte = [0];
        loop {
            match self.input.read(&mut byte) {
                Ok(0) => {
                    self.input_eof = true;
                    return Ok(-1);
                }
                Ok(_) if byte[0] != 0 => return Ok(byte[0] as Cell),
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(RuntimeError::InputError),
            }
        }
    }

    pub fn push(&mut self, value: Cell) {
        self.value_stack.push(value);
    }

    pub fn pop(&mut self) -> Option<Cell> {
        self.value_stack.pop()
    }

    pub fn register(&self, index: usize) -> Option<Cell> {
        self.registers.get(index).copied()
    }

    pub fn storage(&self, address: u16) -> Cell {
        self.storage[address as usize]
    }

    pub fn stack(&self) -> &[Cell] {
        &self.value_stack
    }

    pub fn output(&self) -> &[u8] {
        &self.output
    }

    pub fn clear_output(&mut self) {
        self.output.clear();
    }

    pub(crate) fn validate_register(&self, index: usize) -> Result<(), RuntimeError> {
        if index < REGISTER_COUNT {
            Ok(())
        } else {
            Err(RuntimeError::InvalidRegister)
        }
    }

    pub(crate) fn read_register(&self, index: usize) -> Result<Cell, RuntimeError> {
        self.validate_register(index)?;
        Ok(self.registers[index])
    }

    fn install_body(&mut self, body: ExecutableBody) -> ExecutableId {
        let id = ExecutableId(self.executables.len());
        self.executables.push(Executable { body });
        id
    }

    pub(crate) fn install_completed(&mut self, body: CompletedBody) -> ExecutableId {
        self.install_body(ExecutableBody::Compiled(body.into_instructions()))
    }

    pub(crate) fn resolve(&self, identity: char, role: SourceRole) -> Option<ExecutableId> {
        self.bindings.resolve(identity, role)
    }

    pub(crate) fn resolve_write(&self, name: &str) -> Option<ExecutableId> {
        if name.chars().count() == 1 {
            self.resolve(name.chars().next().unwrap(), SourceRole::Write)
        } else {
            self.long_writes.get(name).copied()
        }
    }

    pub(crate) fn publish_write(
        &mut self,
        name: &str,
        body: CompletedBody,
    ) -> Result<ExecutableId, BindingError> {
        if name.len() == 1 {
            return self.publish_initial(name.as_bytes()[0] as char, SourceRole::Write, body);
        }
        if self.long_writes.contains_key(name) {
            return Err(BindingError::AlreadyBound);
        }
        let id = self.install_completed(body);
        self.long_writes.insert(name.to_owned(), id);
        Ok(id)
    }

    pub(crate) fn builtin_id(&self, primitive: Primitive) -> Option<ExecutableId> {
        self.builtins
            .iter()
            .find(|(candidate, _)| *candidate == primitive)
            .map(|(_, id)| *id)
    }

    pub(crate) fn publish_initial(
        &mut self,
        identity: char,
        role: SourceRole,
        body: CompletedBody,
    ) -> Result<ExecutableId, BindingError> {
        if self.resolve(identity, role).is_some() {
            return Err(BindingError::AlreadyBound);
        }
        let id = self.install_completed(body);
        self.bindings.bind_initial(identity, role, id)?;
        Ok(id)
    }

    fn seed(&mut self, primitive: Primitive, binding: Option<(char, SourceRole)>) {
        let id = self.install_body(ExecutableBody::Primitive(primitive));
        self.builtins.push((primitive, id));
        if let Some((identity, role)) = binding {
            self.bindings
                .bind_initial(identity, role, id)
                .expect("unique built-in binding");
        }
    }

    fn seed_builtins(&mut self) {
        for index in 0..REGISTER_COUNT {
            let identity = char::from(b'A' + index as u8);
            self.seed(
                Primitive::LoadReg(index),
                Some((identity, SourceRole::PrimaryRead)),
            );
            self.seed(
                Primitive::StoreReg(index),
                Some((identity, SourceRole::Write)),
            );
        }
        for (primitive, binding) in [
            (Primitive::LoadStorage, Some(('@', SourceRole::AppliedRead))),
            (Primitive::StoreStorage, Some(('@', SourceRole::Write))),
            (Primitive::PrintNumber, Some(('?', SourceRole::Write))),
            (Primitive::ReadChar, Some(('$', SourceRole::PrimaryRead))),
            (
                Primitive::ReadEntropy,
                Some(('\'', SourceRole::PrimaryRead)),
            ),
            (Primitive::PrintChar, Some(('$', SourceRole::Write))),
            (Primitive::Add, Some(('+', SourceRole::BinaryOperator))),
            (Primitive::Sub, Some(('-', SourceRole::BinaryOperator))),
            (Primitive::Mul, Some(('*', SourceRole::BinaryOperator))),
            (Primitive::Div, Some(('/', SourceRole::BinaryOperator))),
            (Primitive::Rem, Some(('%', SourceRole::BinaryOperator))),
            (Primitive::Lt, Some(('<', SourceRole::BinaryOperator))),
            (Primitive::Gt, Some(('>', SourceRole::BinaryOperator))),
            (Primitive::Eq, None),
            (Primitive::Ne, None),
            (Primitive::Le, None),
            (Primitive::Ge, None),
        ] {
            self.seed(primitive, binding);
        }
    }

    #[cfg(test)]
    fn install(&mut self, body: ExecutableBody) -> ExecutableId {
        self.install_body(body)
    }

    #[cfg(test)]
    fn execute(&mut self, entry: ExecutableId) -> Result<(), RuntimeError> {
        self.execute_completed(entry)
    }

    pub(crate) fn execute_completed(&mut self, entry: ExecutableId) -> Result<(), RuntimeError> {
        let mut current = entry;
        let mut instruction = 0usize;
        let mut call_stack: Vec<Continuation> = Vec::new();

        loop {
            let body = self
                .executables
                .get(current.0)
                .map(|executable| executable.body.clone())
                .ok_or(RuntimeError::InvalidExecutable)?;

            match body {
                ExecutableBody::Primitive(primitive) => {
                    primitive.execute(self)?;
                    match call_stack.pop() {
                        Some(continuation) => {
                            current = continuation.executable;
                            instruction = continuation.instruction;
                        }
                        None => return Ok(()),
                    }
                }
                ExecutableBody::Compiled(code) => {
                    let next = code
                        .get(instruction)
                        .cloned()
                        .ok_or(RuntimeError::MissingReturn)?;
                    match next {
                        Instruction::PushConst(value) => {
                            self.value_stack.push(value);
                            instruction += 1;
                        }
                        Instruction::Call(target) => {
                            if self.executables.get(target.0).is_none() {
                                return Err(RuntimeError::InvalidExecutable);
                            }
                            call_stack.push(Continuation {
                                executable: current,
                                instruction: instruction + 1,
                            });
                            current = target;
                            instruction = 0;
                        }
                        Instruction::Jump(target) => {
                            validate_target(&code, target)?;
                            instruction = target;
                        }
                        Instruction::JumpIfZero(target) => {
                            validate_target(&code, target)?;
                            let condition = *self
                                .value_stack
                                .last()
                                .ok_or(RuntimeError::StackUnderflow)?;
                            self.value_stack.pop();
                            if condition == 0 {
                                instruction = target;
                            } else {
                                instruction += 1;
                            }
                        }
                        Instruction::Return => match call_stack.pop() {
                            Some(continuation) => {
                                current = continuation.executable;
                                instruction = continuation.instruction;
                            }
                            None => return Ok(()),
                        },
                    }
                }
            }
        }
    }
}

fn default_entropy() -> io::Result<Cell> {
    let mut bytes = [0_u8; 2];
    getrandom::fill(&mut bytes).map_err(|error| io::Error::other(error.to_string()))?;
    Ok(i16::from_le_bytes(bytes))
}

fn validate_target(code: &[Instruction], target: usize) -> Result<(), RuntimeError> {
    if target < code.len() {
        Ok(())
    } else {
        Err(RuntimeError::InvalidInstructionTarget)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{BuildError, CodeBuilder};

    #[test]
    fn builtins_have_their_source_roles_and_composites_have_only_ids() {
        let machine = Machine::new();
        for (index, identity) in ('A'..='Z').enumerate() {
            assert_eq!(
                machine.resolve(identity, SourceRole::PrimaryRead),
                machine.builtin_id(Primitive::LoadReg(index))
            );
            assert_eq!(
                machine.resolve(identity, SourceRole::Write),
                machine.builtin_id(Primitive::StoreReg(index))
            );
        }
        for (identity, role, primitive) in [
            ('@', SourceRole::AppliedRead, Primitive::LoadStorage),
            ('@', SourceRole::Write, Primitive::StoreStorage),
            ('?', SourceRole::Write, Primitive::PrintNumber),
            ('$', SourceRole::Write, Primitive::PrintChar),
            ('$', SourceRole::PrimaryRead, Primitive::ReadChar),
            ('+', SourceRole::BinaryOperator, Primitive::Add),
            ('-', SourceRole::BinaryOperator, Primitive::Sub),
            ('*', SourceRole::BinaryOperator, Primitive::Mul),
            ('/', SourceRole::BinaryOperator, Primitive::Div),
            ('%', SourceRole::BinaryOperator, Primitive::Rem),
            ('<', SourceRole::BinaryOperator, Primitive::Lt),
            ('>', SourceRole::BinaryOperator, Primitive::Gt),
        ] {
            assert_eq!(
                machine.resolve(identity, role),
                machine.builtin_id(primitive)
            );
        }
        for primitive in [Primitive::Eq, Primitive::Ne, Primitive::Le, Primitive::Ge] {
            assert!(machine.builtin_id(primitive).is_some());
        }
        for identity in ['#', '^', '&', '[', '|', '='] {
            for role in [
                SourceRole::Write,
                SourceRole::PrimaryRead,
                SourceRole::AppliedRead,
                SourceRole::BinaryOperator,
            ] {
                assert_eq!(machine.resolve(identity, role), None);
            }
        }
        assert_eq!(machine.resolve('?', SourceRole::PrimaryRead), None);
        assert_eq!(machine.resolve('@', SourceRole::PrimaryRead), None);
    }

    #[test]
    fn install_without_binding_and_publish_only_completed_body() {
        let mut machine = Machine::new();
        let mut unit = CodeBuilder::new();
        unit.emit(Instruction::PushConst(7)).unwrap();
        let id = machine.install_completed(unit.finish().unwrap());
        assert_eq!(machine.resolve('p', SourceRole::Write), None);
        machine.execute_completed(id).unwrap();
        assert_eq!(machine.pop(), Some(7));

        let mut unfinished = CodeBuilder::new();
        let missing = unfinished.new_target();
        unfinished.emit_jump(missing).unwrap();
        assert_eq!(unfinished.finish(), Err(BuildError::UnresolvedTarget));
        assert_eq!(machine.resolve('p', SourceRole::Write), None);

        let mut body = CodeBuilder::new();
        body.emit(Instruction::PushConst(9)).unwrap();
        let published = machine
            .publish_initial('p', SourceRole::Write, body.finish().unwrap())
            .unwrap();
        assert_eq!(machine.resolve('p', SourceRole::Write), Some(published));
        assert_eq!(machine.resolve('p', SourceRole::PrimaryRead), None);

        let before = machine.executables.len();
        let duplicate = CodeBuilder::new().finish().unwrap();
        assert_eq!(
            machine.publish_initial('p', SourceRole::Write, duplicate),
            Err(BindingError::AlreadyBound)
        );
        assert_eq!(machine.executables.len(), before);
        assert_eq!(machine.resolve('p', SourceRole::Write), Some(published));
        assert_eq!(
            machine.publish_initial('A', SourceRole::Write, CodeBuilder::new().finish().unwrap()),
            Err(BindingError::AlreadyBound)
        );

        let mut caller = CodeBuilder::new();
        caller.emit_call(published);
        let caller_id = machine.install_completed(caller.finish().unwrap());
        machine.execute_completed(caller_id).unwrap();
        assert_eq!(machine.stack(), &[9]);
    }

    #[test]
    fn source_definition_preserves_another_role_of_the_same_identity() {
        let mut machine = Machine::new();
        let mut read = CodeBuilder::new();
        read.emit(Instruction::PushConst(11)).unwrap();
        let read_id = machine
            .publish_initial('p', SourceRole::PrimaryRead, read.finish().unwrap())
            .unwrap();

        machine.execute_source("&=p [ A=~ ] A=p p=7").unwrap();
        assert_eq!(machine.resolve('p', SourceRole::PrimaryRead), Some(read_id));
        assert_eq!(machine.register(0), Some(7));
        assert!(machine.stack().is_empty());
    }

    #[test]
    fn completed_call_keeps_resolved_id_without_runtime_binding_lookup() {
        let mut machine = Machine::new();
        let id = machine.resolve('A', SourceRole::PrimaryRead).unwrap();
        let mut caller = CodeBuilder::new();
        caller.emit_call(id);
        let completed = caller.finish().unwrap();
        assert_eq!(
            completed.clone().into_instructions(),
            vec![Instruction::Call(id), Instruction::Return]
        );
        let caller_id = machine.install_completed(completed);
        machine.registers[0] = 17;
        machine.execute_completed(caller_id).unwrap();
        assert_eq!(machine.stack(), &[17]);
    }

    #[test]
    fn branch_to_body_end_executes_implicit_return() {
        let mut machine = Machine::new();
        let mut builder = CodeBuilder::new();
        let end = builder.new_target();
        builder.emit_jump(end).unwrap();
        builder.emit(Instruction::PushConst(99)).unwrap();
        builder.complete_target(end).unwrap();
        let id = machine.install_completed(builder.finish().unwrap());
        machine.execute_completed(id).unwrap();
        assert!(machine.stack().is_empty());
    }

    #[test]
    fn machine_starts_zeroed_and_empty() {
        let machine = Machine::new();
        for index in 0..REGISTER_COUNT {
            assert_eq!(machine.register(index), Some(0));
        }
        for address in [0, 0x7fff, 0x8000, u16::MAX] {
            assert_eq!(machine.storage(address), 0);
        }
        assert!(machine.stack().is_empty());
        assert!(machine.output().is_empty());
    }

    #[test]
    fn top_level_primitive_uses_common_dispatch() {
        let mut machine = Machine::new();
        let add = machine.install(ExecutableBody::Primitive(Primitive::Add));
        machine.push(20);
        machine.push(22);
        machine.execute(add).unwrap();
        assert_eq!(machine.stack(), &[42]);
    }

    #[test]
    fn primitive_and_compiled_share_executable_dispatch() {
        let mut machine = Machine::new();
        let add = machine.install(ExecutableBody::Primitive(Primitive::Add));
        let program = machine.install(ExecutableBody::Compiled(vec![
            Instruction::PushConst(20),
            Instruction::PushConst(22),
            Instruction::Call(add),
            Instruction::Return,
        ]));
        machine.execute(program).unwrap();
        assert_eq!(machine.stack(), &[42]);
    }

    #[test]
    fn nested_compiled_calls_return_without_stack_cleanup() {
        let mut machine = Machine::new();
        let leaf = machine.install(ExecutableBody::Compiled(vec![
            Instruction::PushConst(2),
            Instruction::Return,
        ]));
        let middle = machine.install(ExecutableBody::Compiled(vec![
            Instruction::PushConst(1),
            Instruction::Call(leaf),
            Instruction::Return,
        ]));
        let root = machine.install(ExecutableBody::Compiled(vec![
            Instruction::PushConst(0),
            Instruction::Call(middle),
            Instruction::Return,
        ]));
        machine.execute(root).unwrap();
        assert_eq!(machine.stack(), &[0, 1, 2]);
    }

    #[test]
    fn forward_and_backward_jumps_are_local() {
        let mut machine = Machine::new();
        let program = machine.install(ExecutableBody::Compiled(vec![
            Instruction::Jump(2),
            Instruction::PushConst(99),
            Instruction::PushConst(1),
            Instruction::PushConst(0),
            Instruction::JumpIfZero(6),
            Instruction::Jump(2),
            Instruction::Return,
        ]));
        machine.execute(program).unwrap();
        assert_eq!(machine.stack(), &[1]);
    }

    #[test]
    fn backward_jump_repeats_within_current_body() {
        let mut machine = Machine::new();
        let load = machine.install(ExecutableBody::Primitive(Primitive::LoadReg(0)));
        let store = machine.install(ExecutableBody::Primitive(Primitive::StoreReg(0)));
        let sub = machine.install(ExecutableBody::Primitive(Primitive::Sub));
        let program = machine.install(ExecutableBody::Compiled(vec![
            Instruction::PushConst(2),
            Instruction::Call(store),
            Instruction::Call(load),
            Instruction::PushConst(1),
            Instruction::Call(sub),
            Instruction::Call(store),
            Instruction::Call(load),
            Instruction::JumpIfZero(9),
            Instruction::Jump(2),
            Instruction::Return,
        ]));
        machine.execute(program).unwrap();
        assert_eq!(machine.register(0), Some(0));
        assert!(machine.stack().is_empty());
    }

    #[test]
    fn invalid_jump_if_zero_keeps_condition() {
        let mut machine = Machine::new();
        let program = machine.install(ExecutableBody::Compiled(vec![
            Instruction::PushConst(0),
            Instruction::JumpIfZero(99),
            Instruction::Return,
        ]));
        assert_eq!(
            machine.execute(program),
            Err(RuntimeError::InvalidInstructionTarget)
        );
        assert_eq!(machine.stack(), &[0]);
    }

    #[test]
    fn invalid_call_does_not_change_data_stack() {
        let mut machine = Machine::new();
        let program = machine.install(ExecutableBody::Compiled(vec![
            Instruction::PushConst(7),
            Instruction::Call(ExecutableId(999)),
            Instruction::Return,
        ]));
        assert_eq!(
            machine.execute(program),
            Err(RuntimeError::InvalidExecutable)
        );
        assert_eq!(machine.stack(), &[7]);
    }

    #[test]
    fn missing_return_is_an_error() {
        let mut machine = Machine::new();
        let program = machine.install(ExecutableBody::Compiled(vec![Instruction::PushConst(1)]));
        assert_eq!(machine.execute(program), Err(RuntimeError::MissingReturn));
        assert_eq!(machine.stack(), &[1]);
    }

    #[test]
    fn runtime_error_keeps_persistent_state_and_machine_is_reusable() {
        let mut machine = Machine::new();
        let div = machine.install(ExecutableBody::Primitive(Primitive::Div));
        let bad = machine.install(ExecutableBody::Compiled(vec![
            Instruction::PushConst(6),
            Instruction::PushConst(0),
            Instruction::Call(div),
            Instruction::Return,
        ]));
        assert_eq!(machine.execute(bad), Err(RuntimeError::DivisionByZero));
        assert_eq!(machine.stack(), &[6, 0]);

        machine.value_stack.clear();
        let good = machine.install(ExecutableBody::Compiled(vec![
            Instruction::PushConst(9),
            Instruction::Return,
        ]));
        machine.execute(good).unwrap();
        assert_eq!(machine.stack(), &[9]);
    }

    #[test]
    fn nested_underflow_preserves_completed_effects_and_clears_control() {
        let mut machine = Machine::new();
        let store_reg = machine.install(ExecutableBody::Primitive(Primitive::StoreReg(0)));
        let store_storage = machine.install(ExecutableBody::Primitive(Primitive::StoreStorage));
        let print_char = machine.install(ExecutableBody::Primitive(Primitive::PrintChar));
        let add = machine.install(ExecutableBody::Primitive(Primitive::Add));
        let leaf = machine.install(ExecutableBody::Compiled(vec![
            Instruction::PushConst(7),
            Instruction::Call(add),
            Instruction::Return,
        ]));
        let middle = machine.install(ExecutableBody::Compiled(vec![
            Instruction::Call(leaf),
            Instruction::PushConst(99),
            Instruction::Return,
        ]));
        let root = machine.install(ExecutableBody::Compiled(vec![
            Instruction::PushConst(41),
            Instruction::Call(store_reg),
            Instruction::PushConst(Cell::MIN),
            Instruction::PushConst(23),
            Instruction::Call(store_storage),
            Instruction::PushConst(65),
            Instruction::Call(print_char),
            Instruction::Call(middle),
            Instruction::PushConst(88),
            Instruction::Return,
        ]));

        assert_eq!(machine.execute(root), Err(RuntimeError::StackUnderflow));
        assert_eq!(machine.stack(), &[7]);
        assert_eq!(machine.register(0), Some(41));
        assert_eq!(machine.storage(0x8000), 23);
        assert_eq!(machine.output(), b"A");

        let good = machine.install(ExecutableBody::Compiled(vec![
            Instruction::PushConst(9),
            Instruction::Return,
        ]));
        machine.execute(good).unwrap();
        assert_eq!(machine.stack(), &[7, 9]);
        assert_eq!(machine.register(0), Some(41));
        assert_eq!(machine.storage(0x8000), 23);
        assert_eq!(machine.output(), b"A");
    }

    #[test]
    fn conditional_branch_consumes_condition_only_after_target_validation() {
        let mut machine = Machine::new();
        let program = machine.install(ExecutableBody::Compiled(vec![
            Instruction::PushConst(1),
            Instruction::JumpIfZero(3),
            Instruction::Return,
            Instruction::Return,
        ]));
        machine.execute(program).unwrap();
        assert!(machine.stack().is_empty());
    }

    #[test]
    fn invalid_register_is_reported_without_consuming_value() {
        let mut machine = Machine::new();
        machine.push(5);
        assert_eq!(
            Primitive::StoreReg(REGISTER_COUNT).execute(&mut machine),
            Err(RuntimeError::InvalidRegister)
        );
        assert_eq!(machine.stack(), &[5]);
    }
}
