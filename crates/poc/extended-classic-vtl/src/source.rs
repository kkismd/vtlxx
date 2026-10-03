use crate::{
    binding::SourceRole,
    builder::CodeBuilder,
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

pub(crate) fn execute_source(machine: &mut Machine, source: &str) -> Result<(), SourceError> {
    let mut builder = CodeBuilder::new();
    let mut definition = None;
    let mut has_statements = false;

    for line in source.split('\n') {
        let mut offset = 0;
        while let Some(statement) =
            next_statement(line, &mut offset).map_err(SourceError::Compile)?
        {
            let (target, rhs) = split_statement(statement).map_err(SourceError::Compile)?;
            if target == '|' {
                if definition.is_some() || rhs.len() != 1 || !rhs.as_bytes()[0].is_ascii_lowercase()
                {
                    return Err(SourceError::Compile(CompileError::Syntax));
                }
                let identity = rhs.as_bytes()[0] as char;
                flush_top_level(
                    machine,
                    std::mem::replace(&mut builder, CodeBuilder::new()),
                    has_statements,
                )?;
                has_statements = false;
                if machine.resolve(identity, SourceRole::Write).is_some() {
                    return Err(SourceError::Compile(CompileError::Syntax));
                }
                definition = Some(identity);
            } else if rhs.starts_with('|') {
                if rhs != "|" || definition != Some(target) {
                    return Err(SourceError::Compile(CompileError::Syntax));
                }
                let completed = std::mem::replace(&mut builder, CodeBuilder::new())
                    .finish()
                    .map_err(|_| SourceError::Compile(CompileError::Builder))?;
                machine
                    .publish_initial(target, SourceRole::Write, completed)
                    .map_err(|_| SourceError::Compile(CompileError::Syntax))?;
                definition = None;
            } else if target == '&' {
                let mut tail = vec![statement];
                while let Some(next) =
                    next_statement(line, &mut offset).map_err(SourceError::Compile)?
                {
                    tail.push(next);
                }
                compile_tail(&tail, 0, machine, &mut builder).map_err(SourceError::Compile)?;
                has_statements = true;
            } else {
                compile_tail(&[statement], 0, machine, &mut builder)
                    .map_err(SourceError::Compile)?;
                has_statements = true;
            }
        }
    }
    if definition.is_some() {
        return Err(SourceError::Compile(CompileError::Syntax));
    }
    flush_top_level(machine, builder, has_statements)
}

fn flush_top_level(
    machine: &mut Machine,
    builder: CodeBuilder,
    nonempty: bool,
) -> Result<(), SourceError> {
    if !nonempty {
        return Ok(());
    }
    let completed = builder
        .finish()
        .map_err(|_| SourceError::Compile(CompileError::Builder))?;
    let entry = machine.install_completed(completed);
    machine
        .execute_completed(entry)
        .map_err(SourceError::Runtime)
}

fn split_statement(statement: &str) -> Result<(char, &str), CompileError> {
    let mut chars = statement.chars();
    let target = chars.next().ok_or(CompileError::Syntax)?;
    if chars.next() != Some('=') {
        return Err(CompileError::Syntax);
    }
    Ok((target, chars.as_str()))
}

fn next_statement<'a>(line: &'a str, offset: &mut usize) -> Result<Option<&'a str>, CompileError> {
    let bytes = line.as_bytes();
    while *offset < bytes.len() && matches!(bytes[*offset], b' ' | b'\t') {
        *offset += 1;
    }
    if *offset == bytes.len() || bytes[*offset] == b';' {
        *offset = bytes.len();
        return Ok(None);
    }
    let start = *offset;
    let mut quoted = false;
    while *offset < bytes.len() {
        let byte = bytes[*offset];
        if byte == b'"' {
            quoted = !quoted;
        }
        if !quoted && matches!(byte, b' ' | b'\t' | b';') {
            break;
        }
        *offset += 1;
    }
    if quoted {
        return Err(CompileError::Syntax);
    }
    Ok(Some(&line[start..*offset]))
}

#[cfg(test)]
fn scan_line(line: &str) -> Result<Vec<&str>, CompileError> {
    let mut statements = Vec::new();
    let mut offset = 0;
    while let Some(statement) = next_statement(line, &mut offset)? {
        statements.push(statement);
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
        let (target, rhs) = split_statement(statement)?;
        if target == '|' || rhs.starts_with('|') {
            return Err(CompileError::Syntax);
        }
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
            '?' if rhs == "()" => {
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
                if rhs != "()" {
                    compile_rhs(rhs, machine, builder)?;
                }
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
        let mut machine = Machine::new();
        assert_eq!(
            machine.execute_source("A=1 #=9"),
            Err(SourceError::Compile(CompileError::Builder))
        );
        assert_eq!(
            machine.execute_source("A=1 B=2+"),
            Err(SourceError::Compile(CompileError::Syntax))
        );
        assert_eq!(machine.register(0), Some(0));
    }
}
