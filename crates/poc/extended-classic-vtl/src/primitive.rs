use crate::{Cell, cell, executable::RuntimeError, machine::Machine};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Primitive {
    LoadReg(usize),
    StoreReg(usize),
    LoadStorage,
    StoreStorage,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    PrintNumber,
    PrintChar,
}

impl Primitive {
    pub(crate) fn execute(self, machine: &mut Machine) -> Result<(), RuntimeError> {
        match self {
            Self::LoadReg(index) => {
                let value = machine.read_register(index)?;
                machine.value_stack.push(value);
            }
            Self::StoreReg(index) => {
                machine.validate_register(index)?;
                let value = *machine
                    .value_stack
                    .last()
                    .ok_or(RuntimeError::StackUnderflow)?;
                machine.registers[index] = value;
                machine.value_stack.pop();
            }
            Self::LoadStorage => {
                let index = *machine
                    .value_stack
                    .last()
                    .ok_or(RuntimeError::StackUnderflow)?;
                let value = machine.storage[index as u16 as usize];
                let last = machine.value_stack.len() - 1;
                machine.value_stack[last] = value;
            }
            Self::StoreStorage => {
                let len = machine.value_stack.len();
                if len < 2 {
                    return Err(RuntimeError::StackUnderflow);
                }
                let index = machine.value_stack[len - 2];
                let value = machine.value_stack[len - 1];
                machine.storage[index as u16 as usize] = value;
                machine.value_stack.truncate(len - 2);
            }
            Self::Add => binary(machine, |lhs, rhs| Ok(cell::add(lhs, rhs)))?,
            Self::Sub => binary(machine, |lhs, rhs| Ok(cell::sub(lhs, rhs)))?,
            Self::Mul => binary(machine, |lhs, rhs| Ok(cell::mul(lhs, rhs)))?,
            Self::Div => binary(machine, cell::div)?,
            Self::Rem => binary(machine, cell::rem)?,
            Self::Eq => binary(machine, |lhs, rhs| Ok(cell::eq(lhs, rhs)))?,
            Self::Ne => binary(machine, |lhs, rhs| Ok(cell::ne(lhs, rhs)))?,
            Self::Lt => binary(machine, |lhs, rhs| Ok(cell::lt(lhs, rhs)))?,
            Self::Le => binary(machine, |lhs, rhs| Ok(cell::le(lhs, rhs)))?,
            Self::Gt => binary(machine, |lhs, rhs| Ok(cell::gt(lhs, rhs)))?,
            Self::Ge => binary(machine, |lhs, rhs| Ok(cell::ge(lhs, rhs)))?,
            Self::PrintNumber => {
                let value = *machine
                    .value_stack
                    .last()
                    .ok_or(RuntimeError::StackUnderflow)?;
                machine
                    .output
                    .extend_from_slice(value.to_string().as_bytes());
                machine.value_stack.pop();
            }
            Self::PrintChar => {
                let value = *machine
                    .value_stack
                    .last()
                    .ok_or(RuntimeError::StackUnderflow)?;
                machine.output.push(value as u8);
                machine.value_stack.pop();
            }
        }
        Ok(())
    }
}

fn binary(
    machine: &mut Machine,
    operation: impl FnOnce(Cell, Cell) -> Result<Cell, RuntimeError>,
) -> Result<(), RuntimeError> {
    let len = machine.value_stack.len();
    if len < 2 {
        return Err(RuntimeError::StackUnderflow);
    }
    let lhs = machine.value_stack[len - 2];
    let rhs = machine.value_stack[len - 1];
    let result = operation(lhs, rhs)?;
    machine.value_stack.truncate(len - 2);
    machine.value_stack.push(result);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_storage_stack_effects() {
        let mut machine = Machine::new();
        machine.push(41);
        Primitive::StoreReg(0).execute(&mut machine).unwrap();
        assert_eq!(machine.stack(), &[]);
        Primitive::LoadReg(0).execute(&mut machine).unwrap();
        assert_eq!(machine.stack(), &[41]);

        machine.push(-1);
        machine.push(77);
        Primitive::StoreStorage.execute(&mut machine).unwrap();
        assert_eq!(machine.storage(0xffff), 77);

        machine.push(-1);
        Primitive::LoadStorage.execute(&mut machine).unwrap();
        assert_eq!(machine.pop(), Some(77));
    }

    #[test]
    fn failing_binary_primitive_preserves_operands() {
        let mut machine = Machine::new();
        machine.push(7);
        machine.push(0);
        assert_eq!(
            Primitive::Div.execute(&mut machine),
            Err(RuntimeError::DivisionByZero)
        );
        assert_eq!(machine.stack(), &[7, 0]);
    }

    #[test]
    fn underflow_does_not_partially_mutate() {
        let mut machine = Machine::new();
        machine.push(9);
        assert_eq!(
            Primitive::Add.execute(&mut machine),
            Err(RuntimeError::StackUnderflow)
        );
        assert_eq!(machine.stack(), &[9]);
    }

    #[test]
    fn output_primitives_consume_only_on_success() {
        let mut machine = Machine::new();
        machine.push(-12);
        Primitive::PrintNumber.execute(&mut machine).unwrap();
        machine.push(10);
        Primitive::PrintChar.execute(&mut machine).unwrap();
        assert_eq!(machine.output(), b"-12\n");
        assert!(machine.stack().is_empty());
    }
}
