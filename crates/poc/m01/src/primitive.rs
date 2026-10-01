use crate::{Cell, machine::ExecutionError};

#[derive(Clone, Copy)]
pub(crate) enum Primitive {
    Add,
    Subtract,
    Equal,
    Less,
}

pub(crate) const RUNTIME_SEED: [(&str, Primitive); 4] = [
    ("A", Primitive::Add),
    ("S", Primitive::Subtract),
    ("E", Primitive::Equal),
    ("L", Primitive::Less),
];

impl Primitive {
    pub(crate) fn execute(self, stack: &mut Vec<Cell>) -> Result<(), ExecutionError> {
        let len = stack.len();
        if len < 2 {
            return Err(ExecutionError::StackUnderflow);
        }
        let a = stack[len - 2];
        let b = stack[len - 1];
        let result = match self {
            Self::Add => a.checked_add(b).ok_or(ExecutionError::ArithmeticOverflow)?,
            Self::Subtract => a.checked_sub(b).ok_or(ExecutionError::ArithmeticOverflow)?,
            Self::Equal => Cell::from(a == b),
            Self::Less => Cell::from(a < b),
        };
        stack.truncate(len - 2);
        stack.push(result);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_and_operand_order() {
        for (primitive, input, expected) in [
            (Primitive::Add, [2, 3], 5),
            (Primitive::Subtract, [7, 2], 5),
            (Primitive::Subtract, [2, 7], -5),
        ] {
            let mut stack = input.to_vec();
            assert_eq!(primitive.execute(&mut stack), Ok(()));
            assert_eq!(stack, [expected]);
        }
    }

    #[test]
    fn comparisons_return_zero_or_one() {
        for (primitive, input, expected) in [
            (Primitive::Equal, [4, 4], 1),
            (Primitive::Equal, [4, 5], 0),
            (Primitive::Less, [4, 5], 1),
            (Primitive::Less, [5, 4], 0),
        ] {
            let mut stack = input.to_vec();
            assert_eq!(primitive.execute(&mut stack), Ok(()));
            assert_eq!(stack, [expected]);
        }
    }

    #[test]
    fn underflow_preserves_operands() {
        for primitive in [
            Primitive::Add,
            Primitive::Subtract,
            Primitive::Equal,
            Primitive::Less,
        ] {
            for input in [vec![], vec![7]] {
                let mut stack = input.clone();
                assert_eq!(
                    primitive.execute(&mut stack),
                    Err(ExecutionError::StackUnderflow)
                );
                assert_eq!(stack, input);
            }
        }
    }

    #[test]
    fn overflow_preserves_operands() {
        for (primitive, input) in [
            (Primitive::Add, [Cell::MAX, 1]),
            (Primitive::Subtract, [Cell::MIN, 1]),
        ] {
            let mut stack = input.to_vec();
            assert_eq!(
                primitive.execute(&mut stack),
                Err(ExecutionError::ArithmeticOverflow)
            );
            assert_eq!(stack, input);
        }
    }
}
