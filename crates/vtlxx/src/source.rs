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
    let mut has_statements = false;
    let mut reader = SourceReader::new(source);
    // This namespace belongs to one source compilation, not to Machine.
    let mut constants = Constants::default();
    while let Some(token) = reader.next().map_err(SourceError::Compile)? {
        if let Some(definition) = token.text.strip_prefix('.') {
            constants.define(definition).map_err(SourceError::Compile)?;
            continue;
        }
        let (target, rhs) = split_statement(token.text).map_err(SourceError::Compile)?;
        if target == "&" {
            if !valid_write_name(rhs) {
                return Err(SourceError::Compile(CompileError::Syntax));
            }
            flush_top_level(
                machine,
                std::mem::replace(&mut builder, CodeBuilder::new()),
                has_statements,
            )?;
            has_statements = false;
            if machine.resolve_write(rhs).is_some() {
                return Err(SourceError::Compile(CompileError::Syntax));
            }
            let mut definition_builder = CodeBuilder::new();
            compile_required_block(&mut reader, machine, &constants, &mut definition_builder)
                .map_err(SourceError::Compile)?;
            let completed = definition_builder
                .finish()
                .map_err(|_| SourceError::Compile(CompileError::Builder))?;
            machine
                .publish_write(rhs, completed)
                .map_err(|_| SourceError::Compile(CompileError::Syntax))?;
        } else {
            compile_source_statement(token, &mut reader, machine, &constants, &mut builder)
                .map_err(SourceError::Compile)?;
            has_statements = true;
        }
    }
    flush_top_level(machine, builder, has_statements)
}

#[derive(Default)]
struct Constants(Vec<(String, i16)>);

impl Constants {
    fn valid_name(name: &str) -> bool {
        (2..=16).contains(&name.len())
            && name.is_ascii()
            && name.as_bytes()[0].is_ascii_uppercase()
            && name.as_bytes()[1..]
                .iter()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || *byte == b'_')
    }

    fn define(&mut self, definition: &str) -> Result<(), CompileError> {
        let (name, value) = definition.split_once(":=").ok_or(CompileError::Syntax)?;
        let name = name.trim_end_matches([' ', '\t']);
        let value = value.trim_matches([' ', '\t']);
        if !Self::valid_name(name)
            || self.0.iter().any(|(existing, _)| existing == name)
            || value.is_empty()
            || value == "-"
            || !value
                .strip_prefix('-')
                .unwrap_or(value)
                .bytes()
                .all(|byte| byte.is_ascii_digit())
        {
            return Err(CompileError::Syntax);
        }
        let value = value.parse::<i16>().map_err(|_| CompileError::Syntax)?;
        self.0.push((name.to_owned(), value));
        Ok(())
    }

    fn resolve(&self, name: &str) -> Result<i16, CompileError> {
        if !Self::valid_name(name) {
            return Err(CompileError::Syntax);
        }
        self.0
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, value)| *value)
            .ok_or(CompileError::Syntax)
    }

    fn expand_rhs(&self, rhs: &str, machine: &Machine) -> Result<String, CompileError> {
        let mut expanded = String::new();
        let mut offset = 0;
        while offset < rhs.len() {
            let byte = rhs.as_bytes()[offset];
            if byte == b'.' {
                // A completed prefix cannot be followed by another value. Closing
                // open groups lets the existing expression parser check prefixes
                // within nested operands without teaching it constant names.
                let depth = expanded.bytes().fold(0usize, |depth, byte| match byte {
                    b'(' => depth + 1,
                    b')' => depth.saturating_sub(1),
                    _ => depth,
                });
                let mut prefix = expanded.clone();
                prefix.extend(std::iter::repeat_n(')', depth));
                if !prefix.is_empty()
                    && compile_rhs(&prefix, machine, &mut CodeBuilder::new()).is_ok()
                {
                    return Err(CompileError::Syntax);
                }
                // Parenthesized literals must not turn an adjacent primary
                // identity into an applied-read call (e.g. `a.TWO` -> `a(2)`).
                if let Some((index, identity)) = expanded.char_indices().next_back()
                    && machine.resolve(identity, SourceRole::AppliedRead).is_some()
                {
                    let mut before_identity = expanded[..index].to_owned();
                    before_identity.extend(std::iter::repeat_n(')', depth));
                    if before_identity.is_empty()
                        || compile_rhs(&before_identity, machine, &mut CodeBuilder::new()).is_err()
                    {
                        return Err(CompileError::Syntax);
                    }
                }
                offset += 1;
                let start = offset;
                while offset < rhs.len()
                    && (rhs.as_bytes()[offset].is_ascii_uppercase()
                        || rhs.as_bytes()[offset].is_ascii_digit()
                        || rhs.as_bytes()[offset] == b'_')
                {
                    offset += 1;
                }
                let name = &rhs[start..offset];
                expanded.push('(');
                expanded.push_str(&self.resolve(name)?.to_string());
                expanded.push(')');
            } else {
                let ch = rhs[offset..].chars().next().ok_or(CompileError::Syntax)?;
                expanded.push(ch);
                offset += ch.len_utf8();
            }
        }
        Ok(expanded)
    }
}

fn compile_resolved_rhs(
    rhs: &str,
    machine: &Machine,
    constants: &Constants,
    builder: &mut CodeBuilder,
) -> Result<(), CompileError> {
    compile_rhs(&constants.expand_rhs(rhs, machine)?, machine, builder)
}

#[derive(Clone, Copy)]
struct Token<'a> {
    text: &'a str,
}

struct SourceReader<'a> {
    lines: Vec<&'a str>,
    line: usize,
    offset: usize,
}

impl<'a> SourceReader<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            lines: source
                .split('\n')
                .map(|line| line.strip_suffix('\r').unwrap_or(line))
                .collect(),
            line: 0,
            offset: 0,
        }
    }

    fn next(&mut self) -> Result<Option<Token<'a>>, CompileError> {
        while let Some(line) = self.lines.get(self.line) {
            if let Some(text) = next_source_token(line, &mut self.offset)? {
                return Ok(Some(Token { text }));
            }
            self.line += 1;
            self.offset = 0;
        }
        Ok(None)
    }

    fn peek(&self) -> Result<Option<Token<'a>>, CompileError> {
        let mut line_index = self.line;
        let mut offset = self.offset;
        while let Some(line) = self.lines.get(line_index) {
            if let Some(text) = next_source_token(line, &mut offset)? {
                return Ok(Some(Token { text }));
            }
            line_index += 1;
            offset = 0;
        }
        Ok(None)
    }
}

fn next_source_token<'a>(
    line: &'a str,
    offset: &mut usize,
) -> Result<Option<&'a str>, CompileError> {
    let rest = &line[*offset..];
    let indent = rest.len() - rest.trim_start_matches([' ', '\t']).len();
    if rest.as_bytes().get(indent) == Some(&b'.') {
        let start = *offset + indent;
        let bytes = line.as_bytes();
        let mut end = start + 1;
        while end < bytes.len()
            && (bytes[end].is_ascii_uppercase()
                || bytes[end].is_ascii_digit()
                || bytes[end] == b'_')
        {
            end += 1;
        }
        while end < bytes.len() && matches!(bytes[end], b' ' | b'\t') {
            end += 1;
        }
        if bytes.get(end..end + 2) != Some(b":=") {
            return Err(CompileError::Syntax);
        }
        end += 2;
        while end < bytes.len() && matches!(bytes[end], b' ' | b'\t') {
            end += 1;
        }
        if bytes.get(end) == Some(&b'-') {
            end += 1;
        }
        let digits = end;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end == digits || (end < bytes.len() && !matches!(bytes[end], b' ' | b'\t' | b';')) {
            return Err(CompileError::Syntax);
        }
        *offset = end;
        return Ok(Some(&line[start..end]));
    }
    next_statement(line, offset)
}

fn compile_source_statement(
    token: Token<'_>,
    reader: &mut SourceReader<'_>,
    machine: &Machine,
    constants: &Constants,
    builder: &mut CodeBuilder,
) -> Result<(), CompileError> {
    let (target, rhs) = split_statement(token.text)?;
    if target == "*" {
        if rhs.is_empty() || rhs == "()" {
            return Err(CompileError::Syntax);
        }
        // Stage the condition and body together in the surrounding owner.
        // Failed compilation must not leave partial code, labels, or targets.
        let mut staged = builder.clone();
        let loop_target = staged.new_target();
        let end_target = staged.new_target();
        staged
            .complete_target(loop_target)
            .map_err(|_| CompileError::Builder)?;
        compile_resolved_rhs(rhs, machine, constants, &mut staged)?;
        staged
            .emit_jump_if_zero(end_target)
            .map_err(|_| CompileError::Builder)?;
        compile_required_block(reader, machine, constants, &mut staged)?;
        staged
            .emit_jump(loop_target)
            .map_err(|_| CompileError::Builder)?;
        staged
            .complete_target(end_target)
            .map_err(|_| CompileError::Builder)?;
        *builder = staged;
        return Ok(());
    }
    if target == "%" {
        // The complete form belongs to the surrounding code and label owner.
        let mut staged = builder.clone();
        compile_resolved_rhs(rhs, machine, constants, &mut staged)?;
        let false_target = staged.new_target();
        staged
            .emit_jump_if_zero(false_target)
            .map_err(|_| CompileError::Builder)?;
        compile_required_block(reader, machine, constants, &mut staged)?;
        if reader.peek()?.is_some_and(|next| next.text == "[") {
            let end_target = staged.new_target();
            staged
                .emit_jump(end_target)
                .map_err(|_| CompileError::Builder)?;
            staged
                .complete_target(false_target)
                .map_err(|_| CompileError::Builder)?;
            compile_required_block(reader, machine, constants, &mut staged)?;
            staged
                .complete_target(end_target)
                .map_err(|_| CompileError::Builder)?;
        } else {
            staged
                .complete_target(false_target)
                .map_err(|_| CompileError::Builder)?;
        }
        *builder = staged;
        return Ok(());
    }
    compile_statement(token.text, machine, constants, builder)
}

fn compile_required_block(
    reader: &mut SourceReader<'_>,
    machine: &Machine,
    constants: &Constants,
    builder: &mut CodeBuilder,
) -> Result<(), CompileError> {
    if reader.next()?.ok_or(CompileError::Syntax)?.text != "[" {
        return Err(CompileError::Syntax);
    }
    compile_block(reader, machine, constants, builder)
}

fn compile_block(
    reader: &mut SourceReader<'_>,
    machine: &Machine,
    constants: &Constants,
    builder: &mut CodeBuilder,
) -> Result<(), CompileError> {
    while let Some(token) = reader.next()? {
        if token.text == "]" {
            return Ok(());
        }
        compile_source_statement(token, reader, machine, constants, builder)?;
    }
    Err(CompileError::Syntax)
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

fn valid_write_name(name: &str) -> bool {
    (1..=8).contains(&name.len()) && name.bytes().all(|byte| byte.is_ascii_lowercase())
}

fn split_statement(statement: &str) -> Result<(&str, &str), CompileError> {
    let (target, rhs) = statement.split_once('=').ok_or(CompileError::Syntax)?;
    if target.chars().count() != 1 && !valid_write_name(target) {
        return Err(CompileError::Syntax);
    }
    Ok((target, rhs))
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

fn compile_statement(
    statement: &str,
    machine: &Machine,
    constants: &Constants,
    builder: &mut CodeBuilder,
) -> Result<(), CompileError> {
    let (target, rhs) = split_statement(statement)?;
    if matches!(target, "&" | "[" | "=") {
        return Err(CompileError::Syntax);
    }
    match target {
        "^" | "#" => {
            if target == "#" && rhs == "()" {
                builder
                    .emit(Instruction::Return)
                    .map_err(|_| CompileError::Builder)?;
                return Ok(());
            }
            let label: i32 = if rhs.bytes().all(|byte| byte.is_ascii_digit()) && !rhs.is_empty() {
                rhs.parse().map_err(|_| CompileError::Syntax)?
            } else {
                i32::from(constants.resolve(rhs.strip_prefix('.').ok_or(CompileError::Syntax)?)?)
            };
            let result = if target == "^" {
                builder.define_numeric_label(label)
            } else {
                builder.emit_numeric_jump(label)
            };
            result.map_err(|_| CompileError::Builder)?;
        }
        "?" if rhs.starts_with('"') => {
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
        "?" if rhs == "()" => {
            let output = machine
                .resolve('$', SourceRole::Write)
                .ok_or(CompileError::UndefinedBinding)?;
            builder
                .emit(Instruction::PushConst(10))
                .map_err(|_| CompileError::Builder)?;
            builder.emit_call(output);
        }
        "~" => compile_resolved_rhs(rhs, machine, constants, builder)?,
        _ => {
            if rhs != "()" {
                compile_resolved_rhs(rhs, machine, constants, builder)?;
            }
            let id = machine
                .resolve_write(target)
                .ok_or(CompileError::UndefinedBinding)?;
            builder.emit_call(id);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::Primitive;

    #[test]
    fn lowercase_operator_ends_constant_name() {
        let mut machine = Machine::new();
        let mut operator = CodeBuilder::new();
        operator
            .emit(Instruction::Call(
                machine.builtin_id(Primitive::Add).unwrap(),
            ))
            .unwrap();
        machine
            .publish_initial('a', SourceRole::BinaryOperator, operator.finish().unwrap())
            .unwrap();

        machine.execute_source(".MAX:=2\nB=3 A=.MAXaB").unwrap();
        assert_eq!(machine.register(0), Some(5));
    }

    #[test]
    fn adjacent_constant_cannot_create_an_applied_read() {
        let mut machine = Machine::new();
        machine
            .publish_initial(
                'a',
                SourceRole::AppliedRead,
                CodeBuilder::new().finish().unwrap(),
            )
            .unwrap();
        machine
            .publish_initial(
                'a',
                SourceRole::BinaryOperator,
                CodeBuilder::new().finish().unwrap(),
            )
            .unwrap();
        assert!(matches!(
            machine.execute_source(".TWO:=2 A=a.TWO"),
            Err(SourceError::Compile(_))
        ));
        assert!(matches!(
            machine.execute_source(".TWO:=2 A=1+(a.TWO)"),
            Err(SourceError::Compile(_))
        ));
    }

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

    #[test]
    fn failed_block_keeps_parent_code_labels_and_targets_unchanged() {
        let machine = Machine::new();
        let mut builder = CodeBuilder::new();
        builder.emit(Instruction::PushConst(7)).unwrap();
        let original = builder.clone();
        let before = builder.clone().finish().unwrap().into_instructions();
        let mut reader = SourceReader::new("%=1 [ ^=8 A=1+ ]");
        let token = reader.next().unwrap().unwrap();
        assert_eq!(
            compile_source_statement(
                token,
                &mut reader,
                &machine,
                &Constants::default(),
                &mut builder
            ),
            Err(CompileError::Syntax)
        );
        assert_eq!(builder, original);
        assert_eq!(
            builder.clone().finish().unwrap().into_instructions(),
            before
        );
        builder.define_numeric_label(8).unwrap();
        builder.emit_numeric_jump(8).unwrap();
        assert_eq!(
            builder.finish().unwrap().into_instructions(),
            vec![
                Instruction::PushConst(7),
                Instruction::Jump(1),
                Instruction::Return
            ]
        );
    }

    #[test]
    fn failed_if_keeps_parent_code_labels_and_targets_unchanged() {
        let machine = Machine::new();
        for source in [
            "%=1+ [ A=1 ]",
            "%=1 [ ^=8 A=1+ ]",
            "%=1 [ ^=8 A=1 ] [ A=2+ ]",
            "%=1 [ %=0 [ A=1+ ] ]",
            "%=1 [ ^=8 A=1+ ]",
            "%=1 [ ^=8 A=1 ] [ B=2+ ]",
            "%=1 [ ^=8 A=1 ] [ B=2",
            "%=1 [ ^=8 A=1 =| ] [ B=2 ]",
        ] {
            let mut builder = CodeBuilder::new();
            builder.emit(Instruction::PushConst(7)).unwrap();
            builder.define_numeric_label(3).unwrap();
            let existing_target = builder.new_target();
            builder.complete_target(existing_target).unwrap();
            let original = builder.clone();
            let before = builder.clone().finish().unwrap().into_instructions();
            let mut reader = SourceReader::new(source);
            let token = reader.next().unwrap().unwrap();
            assert!(
                compile_source_statement(
                    token,
                    &mut reader,
                    &machine,
                    &Constants::default(),
                    &mut builder
                )
                .is_err(),
                "{source}"
            );
            assert_eq!(builder, original, "{source}");
            assert_eq!(
                builder.clone().finish().unwrap().into_instructions(),
                before,
                "{source}"
            );
            builder.define_numeric_label(8).unwrap();
            builder.emit_numeric_jump(8).unwrap();
            assert_eq!(
                builder.finish().unwrap().into_instructions(),
                vec![
                    Instruction::PushConst(7),
                    Instruction::Jump(1),
                    Instruction::Return
                ],
                "{source}"
            );
        }
    }

    #[test]
    fn failed_while_keeps_parent_code_labels_and_targets_unchanged() {
        let machine = Machine::new();
        for source in [
            "*=A+ [ A=1 ]",
            "*=1 [ ^=8 A=1+ ]",
            "*=1 [ ^=8 ^=8 ]",
            "*=1 [ ^=9 ]",
            "*=1 [ ^=8 A=1",
            "*=MISSING [ ]",
        ] {
            let mut builder = CodeBuilder::new();
            builder.emit(Instruction::PushConst(7)).unwrap();
            builder.define_numeric_label(9).unwrap();
            let target = builder.new_target();
            builder.emit_jump(target).unwrap();
            let before = builder.clone();
            let mut reader = SourceReader::new(source);
            let token = reader.next().unwrap().unwrap();
            assert!(
                compile_source_statement(
                    token,
                    &mut reader,
                    &machine,
                    &Constants::default(),
                    &mut builder
                )
                .is_err(),
                "{source}"
            );
            assert_eq!(builder, before, "{source}");
            builder.complete_target(target).unwrap();
            builder.emit_numeric_jump(9).unwrap();
            builder.define_numeric_label(8).unwrap();
            builder.emit_numeric_jump(8).unwrap();
            assert_eq!(
                builder.finish().unwrap().into_instructions(),
                vec![
                    Instruction::PushConst(7),
                    Instruction::Jump(2),
                    Instruction::Jump(1),
                    Instruction::Jump(3),
                    Instruction::Return
                ],
                "{source}"
            );
        }
    }

    #[test]
    fn while_consumes_only_one_body_block() {
        let machine = Machine::new();
        let mut builder = CodeBuilder::new();
        let mut reader = SourceReader::new("*=0 [ A=1 ] B=2");
        let token = reader.next().unwrap().unwrap();
        compile_source_statement(
            token,
            &mut reader,
            &machine,
            &Constants::default(),
            &mut builder,
        )
        .unwrap();
        assert_eq!(reader.peek().unwrap().unwrap().text, "B=2");
    }
}
