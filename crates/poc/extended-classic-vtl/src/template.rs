use crate::source::CompileError;

pub(crate) struct CompletedTemplate {
    statements: Vec<String>,
}

impl CompletedTemplate {
    pub(crate) fn statements(&self) -> &[String] {
        &self.statements
    }
}

pub(crate) fn bind_statement(statement: &str, identity: char) -> String {
    statement.replace("{}", &identity.to_string())
}

pub(crate) fn validate_template_body(
    statements: Vec<String>,
) -> Result<CompletedTemplate, CompileError> {
    if statements.is_empty() {
        return Err(CompileError::Syntax);
    }

    let mut has_hole = false;
    for statement in &statements {
        has_hole |= validate_template_statement(statement)?;
    }
    if !has_hole {
        return Err(CompileError::Syntax);
    }
    Ok(CompletedTemplate { statements })
}

pub(crate) fn validate_template_statement(statement: &str) -> Result<bool, CompileError> {
    let Some((lhs, _rhs)) = statement.split_once('=') else {
        return Err(CompileError::Syntax);
    };
    if lhs != "{}" && !(lhs.len() == 1 && lhs.as_bytes()[0].is_ascii_uppercase()) {
        return Err(CompileError::Syntax);
    }

    let bytes = statement.as_bytes();
    let mut offset = 0;
    let mut has_hole = false;
    while offset < bytes.len() {
        if bytes[offset] == b'{' {
            if bytes.get(offset + 1) != Some(&b'}') {
                return Err(CompileError::Syntax);
            }
            has_hole = true;
            offset += 2;
        } else {
            offset += 1;
        }
    }
    Ok(has_hole)
}
