use crate::{
    Cell,
    executable::{Executable, ExecutableBody, ExecutableId, Instruction, RuntimeError},
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
    pub(crate) output: Vec<u8>,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    pub fn new() -> Self {
        Self {
            registers: [0; REGISTER_COUNT],
            storage: Box::new([0; STORAGE_SIZE]),
            value_stack: Vec::new(),
            executables: Vec::new(),
            output: Vec::new(),
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

    #[cfg(test)]
    fn install(&mut self, body: ExecutableBody) -> ExecutableId {
        let id = ExecutableId(self.executables.len());
        self.executables.push(Executable { body });
        id
    }

    #[cfg(test)]
    fn execute(&mut self, entry: ExecutableId) -> Result<(), RuntimeError> {
        self.execute_completed(entry)
    }

    fn execute_completed(&mut self, entry: ExecutableId) -> Result<(), RuntimeError> {
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
    use crate::primitive::Primitive;

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
