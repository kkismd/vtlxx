use crate::builder::CompletedBody;
use crate::primitive::Primitive;

/// Identity of a completed executable definition, independent of its source name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefinitionId(pub(crate) usize);

pub(crate) struct Definition {
    pub(crate) body: DefinitionBody,
}

#[allow(dead_code)] // Compiled bodies are published by the later source processor.
pub(crate) enum DefinitionBody {
    Primitive(Primitive),
    Compiled(CompletedBody),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum Instruction {
    Call(DefinitionId),
    Jump(usize),
    JumpIfFalse(usize),
    Return,
}
