use crate::{
    binding::SourceRole,
    builder::CodeBuilder,
    cell,
    executable::{Instruction, RuntimeError, StaticDataId},
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
    let mut names = NameTable::default();
    while let Some(token) = reader.next().map_err(SourceError::Compile)? {
        let text = match token {
            Token::ByteData { name, elements } => {
                // The reader has fully validated the declaration; only a
                // complete, valid declaration installs its body and
                // registers the name (ADR #618).
                names
                    .validate_new_name(name)
                    .map_err(SourceError::Compile)?;
                let id = machine.install_static_data(elements);
                names.register_byte_data(name, id);
                continue;
            }
            Token::Text(text) => text,
        };
        if let Some(definition) = text.strip_prefix('.') {
            names
                .define_integer(definition)
                .map_err(SourceError::Compile)?;
            continue;
        }
        let (target, rhs) = split_statement(text).map_err(SourceError::Compile)?;
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
            compile_required_block(&mut reader, machine, &names, &mut definition_builder)
                .map_err(SourceError::Compile)?;
            let completed = definition_builder
                .finish()
                .map_err(|_| SourceError::Compile(CompileError::Builder))?;
            machine
                .publish_write(rhs, completed)
                .map_err(|_| SourceError::Compile(CompileError::Syntax))?;
        } else {
            compile_source_statement(
                Token::Text(text),
                &mut reader,
                machine,
                &names,
                &mut builder,
            )
            .map_err(SourceError::Compile)?;
            has_statements = true;
        }
    }
    flush_top_level(machine, builder, has_statements)
}

/// The value a compile-local name is bound to. Source names and kinds are
/// compile-time only: expressions resolve them directly to a constant or to
/// a private static-data ID, and nothing here reaches runtime objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NamedValue {
    Integer(i16),
    ByteData(StaticDataId),
}

/// The single compile-local namespace shared by named integers and named
/// static byte arrays (ADR #618, successor of the integer-only table).
/// One lookup serves both kinds, a name may be defined once per
/// `execute_source` call regardless of kind, and only names defined
/// earlier in the same source are visible.
#[derive(Default)]
pub(crate) struct NameTable(Vec<(String, NamedValue)>);

impl NameTable {
    fn valid_name(name: &str) -> bool {
        (2..=16).contains(&name.len())
            && name.is_ascii()
            && name.as_bytes()[0].is_ascii_uppercase()
            && name.as_bytes()[1..]
                .iter()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || *byte == b'_')
    }

    fn validate_new_name(&self, name: &str) -> Result<(), CompileError> {
        if !Self::valid_name(name) || self.0.iter().any(|(existing, _)| existing == name) {
            return Err(CompileError::Syntax);
        }
        Ok(())
    }

    fn define_integer(&mut self, definition: &str) -> Result<(), CompileError> {
        let (name, value) = definition.split_once(":=").ok_or(CompileError::Syntax)?;
        let name = name.trim_end_matches([' ', '\t']);
        let value = value.trim_start_matches([' ', '\t']);
        self.validate_new_name(name)?;
        let value = self.evaluate_integer(value)?;
        self.0.push((name.to_owned(), NamedValue::Integer(value)));
        Ok(())
    }

    fn evaluate_integer(&self, expression: &str) -> Result<i16, CompileError> {
        if !expression.is_ascii() {
            return Err(CompileError::Syntax);
        }
        let bytes = expression.as_bytes();
        let mut pos = 0;
        let mut value = self.integer_value(expression, &mut pos)?;
        while pos < bytes.len() {
            let operator = bytes[pos];
            pos += 1;
            if !matches!(operator, b'+' | b'-' | b'*') {
                return Err(CompileError::Syntax);
            }
            let rhs = self.integer_value(expression, &mut pos)?;
            value = match operator {
                b'+' => cell::add(value, rhs),
                b'-' => cell::sub(value, rhs),
                b'*' => cell::mul(value, rhs),
                _ => unreachable!(),
            };
        }
        Ok(value)
    }

    fn integer_value(&self, expression: &str, pos: &mut usize) -> Result<i16, CompileError> {
        let bytes = expression.as_bytes();
        if bytes.get(*pos) == Some(&b'.') {
            *pos += 1;
            let start = *pos;
            while bytes
                .get(*pos)
                .is_some_and(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || *b == b'_')
            {
                *pos += 1;
            }
            return self.resolve_integer(&expression[start..*pos]);
        }
        let start = *pos;
        if bytes.get(*pos) == Some(&b'-') {
            *pos += 1;
        }
        let digits = *pos;
        while bytes.get(*pos).is_some_and(u8::is_ascii_digit) {
            *pos += 1;
        }
        if *pos == digits {
            return Err(CompileError::Syntax);
        }
        expression[start..*pos]
            .parse()
            .map_err(|_| CompileError::Syntax)
    }

    /// Register a byte-array name whose body the Machine has already taken
    /// ownership of. Callers validate with `validate_new_name` first, so a
    /// failed declaration never installs a body or registers the name.
    fn register_byte_data(&mut self, name: &str, id: StaticDataId) {
        debug_assert!(self.validate_new_name(name).is_ok());
        self.0.push((name.to_owned(), NamedValue::ByteData(id)));
    }

    /// The one name lookup used by expression resolution. Undefined and
    /// not-yet-defined names are indistinguishable here, which is what makes
    /// forward references compile errors.
    pub(crate) fn lookup(&self, name: &str) -> Option<NamedValue> {
        self.0
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, value)| *value)
    }

    /// Integer-only resolution kept for the `^=`/`#=` numeric-label operand.
    /// A byte-data name is not a label and is rejected here; this path never
    /// expands into an expression string.
    fn resolve_integer(&self, name: &str) -> Result<i16, CompileError> {
        if !Self::valid_name(name) {
            return Err(CompileError::Syntax);
        }
        match self.lookup(name) {
            Some(NamedValue::Integer(value)) => Ok(value),
            _ => Err(CompileError::Syntax),
        }
    }
}

/// One lexical unit of a source. Ordinary statements and integer
/// declarations stay borrowed line text; a byte-array declaration is
/// validated and materialized by the reader because its body may span
/// several physical lines (ADR #618).
enum Token<'a> {
    Text(&'a str),
    ByteData { name: &'a str, elements: Vec<u8> },
}

impl<'a> Token<'a> {
    /// The borrowed text for ordinary tokens. A byte-array declaration has
    /// no single-line text; consumers that only compare against `[` / `]`
    /// or split a statement see an empty string and reject it, which keeps
    /// declarations out of blocks and other non-top-level positions.
    fn text(&self) -> &'a str {
        match self {
            Token::Text(text) => text,
            Token::ByteData { .. } => "",
        }
    }
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
        while let Some(line) = self.lines.get(self.line).copied() {
            if let Some(token) = self.next_byte_declaration()? {
                return Ok(Some(token));
            }
            if let Some(text) = next_source_token(line, &mut self.offset)? {
                return Ok(Some(Token::Text(text)));
            }
            self.line += 1;
            self.offset = 0;
        }
        Ok(None)
    }

    fn peek(&self) -> Result<Option<Token<'a>>, CompileError> {
        let mut scratch = SourceReader {
            lines: self.lines.clone(),
            line: self.line,
            offset: self.offset,
        };
        scratch.next()
    }

    /// If the next token on the current line is a byte-array declaration
    /// header (`.NAME := [` with `[` on the same physical line as `:=`),
    /// consume the whole declaration, validating its elements across as
    /// many lines as the body spans. Anything else is left for the ordinary
    /// per-line tokenizer, including malformed `.` forms whose errors it
    /// owns.
    fn next_byte_declaration(&mut self) -> Result<Option<Token<'a>>, CompileError> {
        let line = self.lines[self.line];
        let bytes = line.as_bytes();
        let mut index = self.offset;
        while index < bytes.len() && matches!(bytes[index], b' ' | b'\t') {
            index += 1;
        }
        if bytes.get(index) != Some(&b'.') {
            return Ok(None);
        }
        let name_start = index + 1;
        let mut end = name_start;
        while end < bytes.len()
            && (bytes[end].is_ascii_uppercase()
                || bytes[end].is_ascii_digit()
                || bytes[end] == b'_')
        {
            end += 1;
        }
        if end == name_start {
            return Ok(None);
        }
        let mut after_name = end;
        while after_name < bytes.len() && matches!(bytes[after_name], b' ' | b'\t') {
            after_name += 1;
        }
        if bytes.get(after_name..after_name + 2) != Some(b":=") {
            return Ok(None);
        }
        let mut open = after_name + 2;
        while open < bytes.len() && matches!(bytes[open], b' ' | b'\t') {
            open += 1;
        }
        if bytes.get(open) != Some(&b'[') {
            return Ok(None);
        }
        let name = &line[name_start..end];
        self.offset = open + 1;
        let elements = self.read_byte_array()?;
        Ok(Some(Token::ByteData { name, elements }))
    }

    /// The byte under the array cursor. Crossing a physical line boundary
    /// reads as one `\n`, so whitespace, comments, and the ban on newlines
    /// inside quoted elements all follow from the ordinary byte rules.
    fn array_byte(&self) -> Option<u8> {
        let line = self.lines.get(self.line)?;
        match line.as_bytes().get(self.offset) {
            Some(byte) => Some(*byte),
            None if self.line + 1 < self.lines.len() => Some(b'\n'),
            None => None,
        }
    }

    fn bump_array_byte(&mut self) {
        let line = self.lines[self.line];
        if self.offset >= line.len() {
            self.line += 1;
            self.offset = 0;
        } else {
            self.offset += 1;
        }
    }

    /// Consume array whitespace (space, tab, CR, line crossings) and
    /// `;`-to-end-of-line comments outside quotes. Returns whether anything
    /// was consumed, which is what separates two adjacent elements.
    fn skip_array_space(&mut self) -> bool {
        let mut skipped = false;
        loop {
            match self.array_byte() {
                Some(b' ' | b'\t' | b'\r' | b'\n') => {
                    self.bump_array_byte();
                    skipped = true;
                }
                Some(b';') => {
                    self.offset = self.lines[self.line].len();
                    skipped = true;
                }
                _ => return skipped,
            }
        }
    }

    fn read_byte_array(&mut self) -> Result<Vec<u8>, CompileError> {
        #[derive(PartialEq)]
        enum Last {
            Start,
            Element,
            Comma,
        }
        let mut elements = Vec::new();
        let mut last = Last::Start;
        loop {
            let separated = self.skip_array_space();
            match self.array_byte() {
                None => return Err(CompileError::Syntax),
                Some(b']') => {
                    if last == Last::Comma {
                        return Err(CompileError::Syntax);
                    }
                    self.bump_array_byte();
                    // After `]` only an existing statement separator or the
                    // end of the line may follow (ADR #618).
                    match self
                        .lines
                        .get(self.line)
                        .and_then(|line| line.as_bytes().get(self.offset))
                    {
                        None | Some(b' ' | b'\t' | b';') => return Ok(elements),
                        Some(_) => return Err(CompileError::Syntax),
                    }
                }
                Some(b',') => {
                    if last != Last::Element {
                        return Err(CompileError::Syntax);
                    }
                    self.bump_array_byte();
                    last = Last::Comma;
                }
                Some(b'"') => {
                    if last == Last::Element && !separated {
                        return Err(CompileError::Syntax);
                    }
                    self.read_array_string(&mut elements)?;
                    last = Last::Element;
                }
                Some(byte) if byte.is_ascii_digit() => {
                    if last == Last::Element && !separated {
                        return Err(CompileError::Syntax);
                    }
                    self.read_array_number(&mut elements)?;
                    last = Last::Element;
                }
                Some(_) => return Err(CompileError::Syntax),
            }
        }
    }

    fn read_array_string(&mut self, elements: &mut Vec<u8>) -> Result<(), CompileError> {
        self.bump_array_byte();
        loop {
            match self.array_byte() {
                Some(b'"') => {
                    self.bump_array_byte();
                    return Ok(());
                }
                Some(byte) if (0x20..=0x7e).contains(&byte) => {
                    elements.push(byte);
                    if elements.len() > MAX_STATIC_DATA_LEN {
                        return Err(CompileError::Syntax);
                    }
                    self.bump_array_byte();
                }
                _ => return Err(CompileError::Syntax),
            }
        }
    }

    fn read_array_number(&mut self, elements: &mut Vec<u8>) -> Result<(), CompileError> {
        let mut value: u32 = 0;
        while let Some(byte) = self.array_byte() {
            if !byte.is_ascii_digit() {
                break;
            }
            value = value * 10 + u32::from(byte - b'0');
            if value > 255 {
                return Err(CompileError::Syntax);
            }
            self.bump_array_byte();
        }
        elements.push(value as u8);
        if elements.len() > MAX_STATIC_DATA_LEN {
            return Err(CompileError::Syntax);
        }
        Ok(())
    }
}

/// Logical length limit of one static byte body (ADR #618). Target arena
/// capacity is a separate, target-owned resource limit.
const MAX_STATIC_DATA_LEN: usize = 32768;

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
        let value_start = end;
        while end < bytes.len() && !matches!(bytes[end], b' ' | b'\t' | b';') {
            end += 1;
        }
        if end == value_start {
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
    names: &NameTable,
    builder: &mut CodeBuilder,
) -> Result<(), CompileError> {
    // A byte-array declaration is a top-level form only; anywhere a
    // statement is expected it is a compile error (ADR #618).
    let Token::Text(text) = token else {
        return Err(CompileError::Syntax);
    };
    let (target, rhs) = split_statement(text)?;
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
        compile_rhs(rhs, machine, names, &mut staged)?;
        staged
            .emit_jump_if_zero(end_target)
            .map_err(|_| CompileError::Builder)?;
        compile_required_block(reader, machine, names, &mut staged)?;
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
        compile_rhs(rhs, machine, names, &mut staged)?;
        let false_target = staged.new_target();
        staged
            .emit_jump_if_zero(false_target)
            .map_err(|_| CompileError::Builder)?;
        compile_required_block(reader, machine, names, &mut staged)?;
        if reader.peek()?.is_some_and(|next| next.text() == "[") {
            let end_target = staged.new_target();
            staged
                .emit_jump(end_target)
                .map_err(|_| CompileError::Builder)?;
            staged
                .complete_target(false_target)
                .map_err(|_| CompileError::Builder)?;
            compile_required_block(reader, machine, names, &mut staged)?;
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
    compile_statement(text, machine, names, builder)
}

fn compile_required_block(
    reader: &mut SourceReader<'_>,
    machine: &Machine,
    names: &NameTable,
    builder: &mut CodeBuilder,
) -> Result<(), CompileError> {
    if reader.next()?.ok_or(CompileError::Syntax)?.text() != "[" {
        return Err(CompileError::Syntax);
    }
    compile_block(reader, machine, names, builder)
}

fn compile_block(
    reader: &mut SourceReader<'_>,
    machine: &Machine,
    names: &NameTable,
    builder: &mut CodeBuilder,
) -> Result<(), CompileError> {
    while let Some(token) = reader.next()? {
        if token.text() == "]" {
            return Ok(());
        }
        compile_source_statement(token, reader, machine, names, builder)?;
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
    // The first quote character seen decides the quoted region: a `'` inside
    // a `?="..."` output string is string content, and a `"` inside an RHS
    // character literal such as `'"'` is literal content. Bytes inside a
    // character literal (`' '`, `';'`) are neither separators nor comments.
    let mut quote: Option<u8> = None;
    while *offset < bytes.len() {
        let byte = bytes[*offset];
        match quote {
            Some(open) => {
                if byte == open {
                    quote = None;
                }
            }
            None => {
                if matches!(byte, b'"' | b'\'') {
                    quote = Some(byte);
                } else if matches!(byte, b' ' | b'\t' | b';') {
                    break;
                }
            }
        }
        *offset += 1;
    }
    if quote.is_some() {
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
    names: &NameTable,
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
                i32::from(
                    names.resolve_integer(rhs.strip_prefix('.').ok_or(CompileError::Syntax)?)?,
                )
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
        "~" => compile_rhs(rhs, machine, names, builder)?,
        _ => {
            if rhs != "()" {
                compile_rhs(rhs, machine, names, builder)?;
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
    fn failed_integer_expression_does_not_publish_or_modify_names() {
        let mut names = NameTable::default();
        names.define_integer("AA:=1").unwrap();
        for expression in [".MISSING+1", ".AA+.MISSING", ".AA+2+.MISSING", "32768-1"] {
            let before = names.0.clone();
            assert_eq!(
                names.define_integer(&format!("BAD:={expression}")),
                Err(CompileError::Syntax)
            );
            assert_eq!(names.0, before);
            assert_eq!(names.lookup("BAD"), None);
        }
    }

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
    fn scan_protects_character_literal_bytes() {
        // Spaces, comment starts, and string quotes inside a character
        // literal do not split or comment out the statement, while a `'`
        // inside a double-quoted string stays string content.
        assert_eq!(
            scan_line("A=' ' B=';' C='\"' ?=\"'\" D=1 ; comment").unwrap(),
            vec!["A=' '", "B=';'", "C='\"'", "?=\"'\"", "D=1"]
        );
        assert_eq!(scan_line("A='"), Err(CompileError::Syntax));
        assert_eq!(scan_line("A='A"), Err(CompileError::Syntax));
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
                &NameTable::default(),
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
                    &NameTable::default(),
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
                    &NameTable::default(),
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
            &NameTable::default(),
            &mut builder,
        )
        .unwrap();
        assert_eq!(reader.peek().unwrap().unwrap().text(), "B=2");
    }
}
