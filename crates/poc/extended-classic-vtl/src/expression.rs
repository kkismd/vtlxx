use crate::{
    Cell, binding::SourceRole, builder::CodeBuilder, executable::Instruction, machine::Machine,
    primitive::Primitive, source::CompileError,
};

pub(crate) fn compile_rhs(
    source: &str,
    machine: &Machine,
    builder: &mut CodeBuilder,
) -> Result<(), CompileError> {
    let mut parser = Parser {
        chars: source.chars().collect(),
        position: 0,
        machine,
        builder,
    };
    let seeded = parser.consume('[');
    parser.operand_list(None, seeded)
}

struct Parser<'a> {
    chars: Vec<char>,
    position: usize,
    machine: &'a Machine,
    builder: &'a mut CodeBuilder,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn consume(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn operand_list(
        &mut self,
        close: Option<char>,
        first_seeded: bool,
    ) -> Result<(), CompileError> {
        self.operand(close, first_seeded)?;
        while self.consume(',') {
            self.operand(close, false)?;
        }
        match close {
            Some(delimiter) if self.consume(delimiter) => Ok(()),
            None if self.peek().is_none() => Ok(()),
            _ => Err(CompileError::Syntax),
        }
    }

    fn operand(&mut self, close: Option<char>, seeded: bool) -> Result<(), CompileError> {
        if !seeded {
            self.value()?;
        }
        loop {
            match self.peek() {
                None if close.is_none() => return Ok(()),
                Some(',') | Some(')') => return Ok(()),
                None => return Err(CompileError::Syntax),
                _ => {}
            }
            let operator = self.operator()?;
            self.value()?;
            self.builder.emit_call(operator);
        }
    }

    fn value(&mut self) -> Result<(), CompileError> {
        let current = self.peek().ok_or(CompileError::Syntax)?;
        if current == '(' {
            self.position += 1;
            self.operand(Some(')'), false)?;
            if !self.consume(')') {
                return Err(CompileError::Syntax);
            }
            return Ok(());
        }
        if current.is_ascii_digit()
            || (current == '-'
                && self
                    .chars
                    .get(self.position + 1)
                    .is_some_and(char::is_ascii_digit))
        {
            return self.literal();
        }
        if matches!(current, '[' | ')' | ',' | '=' | '"') {
            return Err(CompileError::Syntax);
        }
        self.position += 1;
        let role = if self.consume('(') {
            self.operand_list(Some(')'), false)?;
            SourceRole::AppliedRead
        } else {
            SourceRole::PrimaryRead
        };
        let id = self
            .machine
            .resolve(current, role)
            .ok_or(CompileError::UndefinedBinding)?;
        self.builder.emit_call(id);
        Ok(())
    }

    fn literal(&mut self) -> Result<(), CompileError> {
        let start = self.position;
        if self.peek() == Some('-') {
            self.position += 1;
        }
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.position += 1;
        }
        let text: String = self.chars[start..self.position].iter().collect();
        let value: Cell = text.parse().map_err(|_| CompileError::Syntax)?;
        self.builder
            .emit(Instruction::PushConst(value))
            .map_err(|_| CompileError::Builder)
    }

    fn operator(&mut self) -> Result<crate::executable::ExecutableId, CompileError> {
        let current = self.peek().ok_or(CompileError::Syntax)?;
        let next = self.chars.get(self.position + 1).copied();
        let composite = match (current, next) {
            ('=', Some('=')) => Some(Primitive::Eq),
            ('!', Some('=')) => Some(Primitive::Ne),
            ('<', Some('=')) => Some(Primitive::Le),
            ('>', Some('=')) => Some(Primitive::Ge),
            _ => None,
        };
        if let Some(primitive) = composite {
            self.position += 2;
            return self
                .machine
                .builtin_id(primitive)
                .ok_or(CompileError::UndefinedBinding);
        }
        if matches!(current, '=' | '(' | ')' | '[' | '"' | ';') {
            return Err(CompileError::Syntax);
        }
        self.position += 1;
        self.machine
            .resolve(current, SourceRole::BinaryOperator)
            .ok_or(CompileError::UndefinedBinding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instructions(machine: &Machine, source: &str) -> Vec<Instruction> {
        let mut builder = CodeBuilder::new();
        compile_rhs(source, machine, &mut builder).unwrap();
        builder.finish().unwrap().into_instructions()
    }

    #[test]
    fn operators_are_left_associative_and_grouping_changes_order() {
        let machine = Machine::new();
        let load_a = machine.resolve('A', SourceRole::PrimaryRead).unwrap();
        let load_b = machine.resolve('B', SourceRole::PrimaryRead).unwrap();
        let mul = machine.resolve('*', SourceRole::BinaryOperator).unwrap();
        let add = machine.resolve('+', SourceRole::BinaryOperator).unwrap();
        assert_eq!(
            instructions(&machine, "A+B*2"),
            vec![
                Instruction::Call(load_a),
                Instruction::Call(load_b),
                Instruction::Call(add),
                Instruction::PushConst(2),
                Instruction::Call(mul),
                Instruction::Return,
            ]
        );
        assert_eq!(
            instructions(&machine, "A+(B*2)"),
            vec![
                Instruction::Call(load_a),
                Instruction::Call(load_b),
                Instruction::PushConst(2),
                Instruction::Call(mul),
                Instruction::Call(add),
                Instruction::Return,
            ]
        );
    }

    #[test]
    fn roles_and_operand_lists_resolve_before_runtime() {
        let mut machine = Machine::new();
        let primary = machine
            .publish_initial(
                'a',
                SourceRole::PrimaryRead,
                CodeBuilder::new().finish().unwrap(),
            )
            .unwrap();
        let applied = machine
            .publish_initial(
                'a',
                SourceRole::AppliedRead,
                CodeBuilder::new().finish().unwrap(),
            )
            .unwrap();
        let load_b = machine.resolve('B', SourceRole::PrimaryRead).unwrap();
        assert_eq!(
            instructions(&machine, "a"),
            vec![Instruction::Call(primary), Instruction::Return]
        );
        assert_eq!(
            instructions(&machine, "a(B,2)"),
            vec![
                Instruction::Call(load_b),
                Instruction::PushConst(2),
                Instruction::Call(applied),
                Instruction::Return,
            ]
        );
        assert_eq!(
            instructions(&machine, "B,2"),
            vec![
                Instruction::Call(load_b),
                Instruction::PushConst(2),
                Instruction::Return,
            ]
        );
        let mut builder = CodeBuilder::new();
        assert_eq!(
            compile_rhs("a(B)(C)", &machine, &mut builder),
            Err(CompileError::Syntax)
        );
    }

    #[test]
    fn composite_operators_and_seeded_stack_emit_only_resolved_calls() {
        let machine = Machine::new();
        let eq = machine.builtin_id(Primitive::Eq).unwrap();
        assert_eq!(
            instructions(&machine, "[==0"),
            vec![
                Instruction::PushConst(0),
                Instruction::Call(eq),
                Instruction::Return,
            ]
        );
        assert_eq!(instructions(&machine, "["), vec![Instruction::Return]);
        for (source, expected) in [
            ("1!=2", Primitive::Ne),
            ("1<=2", Primitive::Le),
            ("1>=2", Primitive::Ge),
        ] {
            let code = instructions(&machine, source);
            assert_eq!(
                code[2],
                Instruction::Call(machine.builtin_id(expected).unwrap())
            );
        }
    }
}
