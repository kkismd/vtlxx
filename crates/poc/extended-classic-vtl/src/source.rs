use crate::{
    binding::SourceRole,
    builder::{CodeBuilder, CompletedBody},
    executable::{Instruction, RuntimeError},
    expression::compile_rhs,
    machine::Machine,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompileError {
    Syntax,
    UndefinedBinding,
    Builder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceError {
    Compile(CompileError),
    Runtime(RuntimeError),
}

pub(crate) fn compile_source(
    machine: &Machine,
    source: &str,
) -> Result<CompletedBody, CompileError> {
    let mut builder = CodeBuilder::new();
    for physical_line in source.split('\n') {
        let statements = scan_line(physical_line)?;
        compile_tail(&statements, 0, machine, &mut builder)?;
    }
    builder.finish().map_err(|_| CompileError::Builder)
}

fn scan_line(line: &str) -> Result<Vec<&str>, CompileError> {
    let mut statements = Vec::new();
    let mut start = None;
    let mut quoted = false;
    for (index, character) in line.char_indices() {
        if character == '"' {
            quoted = !quoted;
        }
        if !quoted && (character == ';' || character == ' ' || character == '\t') {
            if let Some(begin) = start.take() {
                statements.push(&line[begin..index]);
            }
            if character == ';' {
                break;
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if quoted {
        return Err(CompileError::Syntax);
    }
    if let Some(begin) = start {
        statements.push(&line[begin..]);
    }
    Ok(statements)
}

fn compile_tail(
    statements: &[&str],
    mut index: usize,
    machine: &Machine,
    builder: &mut CodeBuilder,
) -> Result<(), CompileError> {
    while let Some(statement) = statements.get(index) {
        let mut chars = statement.chars();
        let target = chars.next().ok_or(CompileError::Syntax)?;
        if chars.next() != Some('=') {
            return Err(CompileError::Syntax);
        }
        let rhs = chars.as_str();
        match target {
            '^' | '#' => {
                if rhs.is_empty() || !rhs.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(CompileError::Syntax);
                }
                let label: i32 = rhs.parse().map_err(|_| CompileError::Syntax)?;
                let result = if target == '^' {
                    builder.define_numeric_label(label)
                } else {
                    builder.emit_numeric_jump(label)
                };
                result.map_err(|_| CompileError::Builder)?;
            }
            '&' => {
                compile_rhs(rhs, machine, builder)?;
                let end = builder.new_target();
                builder
                    .emit_jump_if_zero(end)
                    .map_err(|_| CompileError::Builder)?;
                compile_tail(statements, index + 1, machine, builder)?;
                builder
                    .complete_target(end)
                    .map_err(|_| CompileError::Builder)?;
                return Ok(());
            }
            '?' if rhs.starts_with('"') => {
                let bytes = rhs.as_bytes();
                if bytes.len() < 2 || bytes.last() != Some(&b'"') {
                    return Err(CompileError::Syntax);
                }
                let content = &rhs[1..rhs.len() - 1];
                if !content.is_ascii() || content.contains('"') {
                    return Err(CompileError::Syntax);
                }
                let output = machine
                    .resolve('$', SourceRole::Write)
                    .ok_or(CompileError::UndefinedBinding)?;
                for byte in content.bytes() {
                    builder
                        .emit(Instruction::PushConst(byte.into()))
                        .map_err(|_| CompileError::Builder)?;
                    builder.emit_call(output);
                }
            }
            '?' if rhs.is_empty() => {
                let output = machine
                    .resolve('$', SourceRole::Write)
                    .ok_or(CompileError::UndefinedBinding)?;
                builder
                    .emit(Instruction::PushConst(10))
                    .map_err(|_| CompileError::Builder)?;
                builder.emit_call(output);
            }
            '[' => compile_rhs(rhs, machine, builder)?,
            _ => {
                compile_rhs(rhs, machine, builder)?;
                let id = machine
                    .resolve(target, SourceRole::Write)
                    .ok_or(CompileError::UndefinedBinding)?;
                builder.emit_call(id);
            }
        }
        index += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_keeps_quotes_and_discards_comment() {
        assert_eq!(
            scan_line(" A=1\t?=\"a b;c\" B=2 ; comment").unwrap(),
            vec!["A=1", "?=\"a b;c\"", "B=2"]
        );
        assert_eq!(scan_line("?=\"unfinished"), Err(CompileError::Syntax));
    }

    #[test]
    fn compile_failure_never_produces_completed_body() {
        let machine = Machine::new();
        assert_eq!(
            compile_source(&machine, "A=1 #=9").map(|_| ()),
            Err(CompileError::Builder)
        );
        assert_eq!(
            compile_source(&machine, "A=1 B=2+").map(|_| ()),
            Err(CompileError::Syntax)
        );
    }
}
