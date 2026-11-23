use crate::{Cell, primitive::Primitive};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ExecutableId(pub(crate) usize);

/// Private identity of one immutable static byte body owned by the Machine.
/// Assigned when a byte-array declaration completes compilation; compiled
/// instructions embed only this ID, never a source name (ADR #618).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct StaticDataId(pub(crate) usize);

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
    /// Replace the index Cell on top of the stack with the unsigned byte at
    /// that 0-origin position of the identified static byte body.
    ReadStaticByte(StaticDataId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    InputError,
    EntropyError,
    OutputError,
    StackUnderflow,
    DivisionByZero,
    RemainderByZero,
    InvalidExecutable,
    InvalidInstructionTarget,
    MissingReturn,
    InvalidRegister,
    /// A static byte read whose index is negative, at least the body length,
    /// or any index into an empty body (ADR #618). The failed read leaves
    /// the value stack, registers, storage, and static data unchanged.
    StaticDataOutOfBounds,
}
