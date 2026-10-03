use crate::{Cell, primitive::Primitive};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ExecutableId(pub(crate) usize);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Executable {
    pub(crate) body: ExecutableBody,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExecutableBody {
    Primitive(Primitive),
    Compiled(Vec<Instruction>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Instruction {
    PushConst(Cell),
    Call(ExecutableId),
    Jump(usize),
    JumpIfZero(usize),
    Return,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeError {
    StackUnderflow,
    DivisionByZero,
    RemainderByZero,
    InvalidExecutable,
    InvalidInstructionTarget,
    MissingReturn,
    InvalidRegister,
}
