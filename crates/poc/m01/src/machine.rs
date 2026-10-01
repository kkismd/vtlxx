use crate::{
    Cell, DefinitionId,
    definition::{Definition, DefinitionBody, Instruction},
    dictionary::RuntimeDictionary,
    primitive::RUNTIME_SEED,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionError {
    UnknownRuntimeWord,
    InvalidDefinition,
    StackUnderflow,
    ArithmeticOverflow,
    InvalidInstructionTarget,
    MissingReturn,
}

#[derive(Clone, Copy)]
struct Frame {
    definition: DefinitionId,
    instruction: usize,
}

/// Persistent runtime state. Call frames exist only during an execute call.
#[derive(Default)]
pub struct Machine {
    data_stack: Vec<Cell>,
    definitions: Vec<Definition>,
    runtime_dictionary: RuntimeDictionary,
}

impl Machine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_runtime_seed() -> Self {
        let mut machine = Self::new();
        for (name, primitive) in RUNTIME_SEED {
            let id = machine.store_definition(Definition {
                body: DefinitionBody::Primitive(primitive),
            });
            machine.runtime_dictionary.bind(name, id);
        }
        machine
    }

    pub fn push(&mut self, value: Cell) {
        self.data_stack.push(value);
    }

    pub fn pop(&mut self) -> Option<Cell> {
        self.data_stack.pop()
    }

    pub fn execute_runtime_word(&mut self, name: &str) -> Result<(), ExecutionError> {
        let id = self
            .runtime_dictionary
            .lookup(name)
            .ok_or(ExecutionError::UnknownRuntimeWord)?;
        self.execute(id)
    }

    fn store_definition(&mut self, definition: Definition) -> DefinitionId {
        let id = DefinitionId(self.definitions.len());
        self.definitions.push(definition);
        id
    }

    fn execute(&mut self, id: DefinitionId) -> Result<(), ExecutionError> {
        let body = &self
            .definitions
            .get(id.0)
            .ok_or(ExecutionError::InvalidDefinition)?
            .body;
        if let DefinitionBody::Primitive(primitive) = body {
            return primitive.execute(&mut self.data_stack);
        }

        let mut current = Frame {
            definition: id,
            instruction: 0,
        };
        let mut callers = Vec::new();

        loop {
            let instructions = match &self.definitions[current.definition.0].body {
                DefinitionBody::Compiled(instructions) => instructions,
                DefinitionBody::Primitive(_) => unreachable!(),
            };
            let instruction = *instructions
                .get(current.instruction)
                .ok_or(ExecutionError::MissingReturn)?;

            match instruction {
                Instruction::Call(target) => {
                    let definition = self
                        .definitions
                        .get(target.0)
                        .ok_or(ExecutionError::InvalidDefinition)?;
                    match &definition.body {
                        DefinitionBody::Primitive(primitive) => {
                            primitive.execute(&mut self.data_stack)?;
                            current.instruction += 1;
                        }
                        DefinitionBody::Compiled(_) => {
                            callers.push(Frame {
                                definition: current.definition,
                                instruction: current.instruction + 1,
                            });
                            current = Frame {
                                definition: target,
                                instruction: 0,
                            };
                        }
                    }
                }
                Instruction::Jump(target) => {
                    if target >= instructions.len() {
                        return Err(ExecutionError::InvalidInstructionTarget);
                    }
                    current.instruction = target;
                }
                Instruction::JumpIfFalse(target) => {
                    if target >= instructions.len() {
                        return Err(ExecutionError::InvalidInstructionTarget);
                    }
                    let condition = self
                        .data_stack
                        .pop()
                        .ok_or(ExecutionError::StackUnderflow)?;
                    current.instruction = if condition == 0 {
                        target
                    } else {
                        current.instruction + 1
                    };
                }
                Instruction::Return => match callers.pop() {
                    Some(caller) => current = caller,
                    None => return Ok(()),
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compiled(machine: &mut Machine, name: &str, instructions: Vec<Instruction>) -> DefinitionId {
        let id = machine.store_definition(Definition {
            body: DefinitionBody::Compiled(instructions),
        });
        machine.runtime_dictionary.bind(name, id);
        id
    }

    fn stack(machine: &Machine) -> &[Cell] {
        &machine.data_stack
    }

    #[test]
    fn empty_machine_and_runtime_seed() {
        let mut empty = Machine::new();
        assert_eq!(
            empty.execute_runtime_word("A"),
            Err(ExecutionError::UnknownRuntimeWord)
        );
        let mut machine = Machine::with_runtime_seed();
        for (name, a, b, result) in [
            ("A", 2, 3, 5),
            ("S", 7, 2, 5),
            ("E", 4, 4, 1),
            ("L", 4, 5, 1),
        ] {
            machine.push(a);
            machine.push(b);
            assert_eq!(machine.execute_runtime_word(name), Ok(()));
            assert_eq!(machine.pop(), Some(result));
        }
    }

    #[test]
    fn compiled_calls_primitive_and_returns_from_nested_calls() {
        let mut machine = Machine::with_runtime_seed();
        let add = machine.runtime_dictionary.lookup("A").unwrap();
        let inner = compiled(
            &mut machine,
            "inner",
            vec![Instruction::Call(add), Instruction::Return],
        );
        compiled(
            &mut machine,
            "outer",
            vec![Instruction::Call(inner), Instruction::Return],
        );
        machine.push(2);
        machine.push(3);
        assert_eq!(machine.execute_runtime_word("outer"), Ok(()));
        assert_eq!(stack(&machine), &[5]);
    }

    #[test]
    fn compiled_calls_each_runtime_seed_primitive() {
        let mut machine = Machine::with_runtime_seed();
        for (name, input, expected) in [
            ("A", [2, 3], 5),
            ("S", [2, 7], -5),
            ("E", [4, 5], 0),
            ("L", [4, 5], 1),
        ] {
            let target = machine.runtime_dictionary.lookup(name).unwrap();
            compiled(
                &mut machine,
                "caller",
                vec![Instruction::Call(target), Instruction::Return],
            );
            machine.push(input[0]);
            machine.push(input[1]);
            assert_eq!(machine.execute_runtime_word("caller"), Ok(()));
            assert_eq!(machine.pop(), Some(expected));
        }
    }

    #[test]
    fn missing_return_is_error_and_next_execute_starts_fresh() {
        let mut machine = Machine::with_runtime_seed();
        compiled(&mut machine, "bad", vec![]);
        assert_eq!(
            machine.execute_runtime_word("bad"),
            Err(ExecutionError::MissingReturn)
        );
        machine.push(2);
        machine.push(3);
        assert_eq!(machine.execute_runtime_word("A"), Ok(()));
        assert_eq!(stack(&machine), &[5]);
    }

    #[test]
    fn forward_and_backward_jumps_stay_in_body() {
        let mut machine = Machine::new();
        compiled(
            &mut machine,
            "branch",
            vec![
                Instruction::Jump(2),
                Instruction::Jump(99),
                Instruction::Return,
            ],
        );
        assert_eq!(machine.execute_runtime_word("branch"), Ok(()));

        // A backward jump is taken once; the consumed condition then selects Return.
        compiled(
            &mut machine,
            "loop",
            vec![
                Instruction::Jump(2),
                Instruction::Return,
                Instruction::JumpIfFalse(4),
                Instruction::Jump(1),
                Instruction::Return,
            ],
        );
        machine.push(3);
        assert_eq!(machine.execute_runtime_word("loop"), Ok(()));
        assert_eq!(stack(&machine), &[]);
    }

    #[test]
    fn conditional_jump_consumes_only_after_validation() {
        let mut machine = Machine::new();
        compiled(
            &mut machine,
            "branch",
            vec![
                Instruction::JumpIfFalse(2),
                Instruction::Return,
                Instruction::Return,
            ],
        );
        for condition in [0, -7, 9] {
            machine.push(condition);
            assert_eq!(machine.execute_runtime_word("branch"), Ok(()));
            assert_eq!(stack(&machine), &[]);
        }
        compiled(
            &mut machine,
            "invalid",
            vec![Instruction::JumpIfFalse(2), Instruction::Return],
        );
        machine.push(0);
        assert_eq!(
            machine.execute_runtime_word("invalid"),
            Err(ExecutionError::InvalidInstructionTarget)
        );
        assert_eq!(stack(&machine), &[0]);
    }

    #[test]
    fn invalid_targets_and_definition_do_not_affect_next_execute() {
        let mut machine = Machine::new();
        compiled(
            &mut machine,
            "jump",
            vec![Instruction::Jump(2), Instruction::Return],
        );
        compiled(
            &mut machine,
            "call",
            vec![Instruction::Call(DefinitionId(999)), Instruction::Return],
        );
        compiled(&mut machine, "good", vec![Instruction::Return]);
        assert_eq!(
            machine.execute_runtime_word("jump"),
            Err(ExecutionError::InvalidInstructionTarget)
        );
        assert_eq!(
            machine.execute_runtime_word("call"),
            Err(ExecutionError::InvalidDefinition)
        );
        assert_eq!(machine.execute_runtime_word("good"), Ok(()));
    }

    #[test]
    fn nested_errors_preserve_prior_effects_and_discard_control_state() {
        let mut machine = Machine::with_runtime_seed();
        let add = machine.runtime_dictionary.lookup("A").unwrap();
        let inner = compiled(
            &mut machine,
            "inner",
            vec![
                Instruction::Call(add),
                Instruction::Call(add),
                Instruction::Return,
            ],
        );
        compiled(
            &mut machine,
            "outer",
            vec![Instruction::Call(inner), Instruction::Return],
        );
        machine.push(2);
        machine.push(3);
        assert_eq!(
            machine.execute_runtime_word("outer"),
            Err(ExecutionError::StackUnderflow)
        );
        assert_eq!(stack(&machine), &[5]);
        machine.push(4);
        assert_eq!(machine.execute_runtime_word("A"), Ok(()));
        assert_eq!(stack(&machine), &[9]);

        machine.push(Cell::MAX);
        machine.push(1);
        assert_eq!(
            machine.execute_runtime_word("outer"),
            Err(ExecutionError::ArithmeticOverflow)
        );
        assert_eq!(stack(&machine), &[9, Cell::MAX, 1]);
        assert_eq!(machine.execute_runtime_word("E"), Ok(()));
        assert_eq!(stack(&machine), &[9, 0]);
    }

    #[test]
    fn rebinding_does_not_change_earlier_compiled_call() {
        let mut machine = Machine::with_runtime_seed();
        let old = machine.runtime_dictionary.lookup("A").unwrap();
        compiled(
            &mut machine,
            "caller",
            vec![Instruction::Call(old), Instruction::Return],
        );
        let new = machine.runtime_dictionary.lookup("S").unwrap();
        machine.runtime_dictionary.bind("A", new);
        assert_ne!(old, new);
        machine.push(7);
        machine.push(2);
        assert_eq!(machine.execute_runtime_word("caller"), Ok(()));
        assert_eq!(stack(&machine), &[9]);
        machine.push(2);
        assert_eq!(machine.execute_runtime_word("A"), Ok(()));
        assert_eq!(stack(&machine), &[7]);
    }
}
