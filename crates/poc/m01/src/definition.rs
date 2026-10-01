use crate::primitive::Primitive;

/// Identity of a completed executable definition, independent of its source name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefinitionId(pub(crate) usize);

pub(crate) struct Definition {
    pub(crate) body: DefinitionBody,
}

pub(crate) enum DefinitionBody {
    Primitive(Primitive),
    // The builder and publication path for compiled bodies arrive in #92.
    #[allow(dead_code)]
    Compiled(Vec<Instruction>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum Instruction {
    Call(DefinitionId),
    Jump(usize),
    JumpIfFalse(usize),
    Return,
}
