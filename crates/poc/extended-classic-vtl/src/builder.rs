use std::{
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::executable::{ExecutableId, Instruction};

static NEXT_OWNER: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BranchTarget {
    owner: usize,
    index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BuildError {
    ForeignTarget,
    DuplicateTarget,
    UnresolvedTarget,
    DuplicateLabel,
    UnresolvedLabel,
    LabelOutOfRange,
    RawBranch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PendingInstruction {
    Ready(Instruction),
    TargetJump(BranchTarget, bool),
    NumericJump(u16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompletedBody {
    instructions: Vec<Instruction>,
}

impl CompletedBody {
    pub(crate) fn into_instructions(self) -> Vec<Instruction> {
        self.instructions
    }
}

#[derive(Clone)]
pub(crate) struct CodeBuilder {
    owner: usize,
    code: Vec<PendingInstruction>,
    targets: Vec<Option<usize>>,
    labels: HashMap<u16, usize>,
}

impl CodeBuilder {
    pub(crate) fn new() -> Self {
        Self {
            owner: NEXT_OWNER.fetch_add(1, Ordering::Relaxed),
            code: Vec::new(),
            targets: Vec::new(),
            labels: HashMap::new(),
        }
    }

    pub(crate) fn emit(&mut self, instruction: Instruction) -> Result<(), BuildError> {
        if matches!(
            instruction,
            Instruction::Jump(_) | Instruction::JumpIfZero(_)
        ) {
            return Err(BuildError::RawBranch);
        }
        self.code.push(PendingInstruction::Ready(instruction));
        Ok(())
    }

    pub(crate) fn emit_call(&mut self, id: ExecutableId) {
        self.code
            .push(PendingInstruction::Ready(Instruction::Call(id)));
    }

    pub(crate) fn append_fragment(
        &mut self,
        instructions: Vec<Instruction>,
    ) -> Result<(), BuildError> {
        if instructions.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::Jump(_) | Instruction::JumpIfZero(_)
            )
        }) {
            return Err(BuildError::RawBranch);
        }
        self.code
            .extend(instructions.into_iter().map(PendingInstruction::Ready));
        Ok(())
    }

    pub(crate) fn new_target(&mut self) -> BranchTarget {
        let target = BranchTarget {
            owner: self.owner,
            index: self.targets.len(),
        };
        self.targets.push(None);
        target
    }

    fn target_index(&self, target: BranchTarget) -> Result<usize, BuildError> {
        if target.owner != self.owner || target.index >= self.targets.len() {
            Err(BuildError::ForeignTarget)
        } else {
            Ok(target.index)
        }
    }

    pub(crate) fn complete_target(&mut self, target: BranchTarget) -> Result<(), BuildError> {
        let index = self.target_index(target)?;
        if self.targets[index].is_some() {
            return Err(BuildError::DuplicateTarget);
        }
        self.targets[index] = Some(self.code.len());
        Ok(())
    }

    pub(crate) fn emit_jump(&mut self, target: BranchTarget) -> Result<(), BuildError> {
        self.target_index(target)?;
        self.code
            .push(PendingInstruction::TargetJump(target, false));
        Ok(())
    }

    pub(crate) fn emit_jump_if_zero(&mut self, target: BranchTarget) -> Result<(), BuildError> {
        self.target_index(target)?;
        self.code.push(PendingInstruction::TargetJump(target, true));
        Ok(())
    }

    fn checked_label(label: i32) -> Result<u16, BuildError> {
        u16::try_from(label)
            .ok()
            .filter(|value| *value <= 32767)
            .ok_or(BuildError::LabelOutOfRange)
    }

    pub(crate) fn define_numeric_label(&mut self, label: i32) -> Result<(), BuildError> {
        let label = Self::checked_label(label)?;
        if self.labels.contains_key(&label) {
            return Err(BuildError::DuplicateLabel);
        }
        self.labels.insert(label, self.code.len());
        Ok(())
    }

    pub(crate) fn emit_numeric_jump(&mut self, label: i32) -> Result<(), BuildError> {
        let label = Self::checked_label(label)?;
        self.code.push(PendingInstruction::NumericJump(label));
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<CompletedBody, BuildError> {
        if self.targets.iter().any(Option::is_none) {
            return Err(BuildError::UnresolvedTarget);
        }
        self.code
            .push(PendingInstruction::Ready(Instruction::Return));
        let mut instructions = Vec::with_capacity(self.code.len());
        for pending in self.code {
            let instruction = match pending {
                PendingInstruction::Ready(instruction) => instruction,
                PendingInstruction::TargetJump(target, conditional) => {
                    let position =
                        self.targets[target.index].ok_or(BuildError::UnresolvedTarget)?;
                    if conditional {
                        Instruction::JumpIfZero(position)
                    } else {
                        Instruction::Jump(position)
                    }
                }
                PendingInstruction::NumericJump(label) => {
                    Instruction::Jump(*self.labels.get(&label).ok_or(BuildError::UnresolvedLabel)?)
                }
            };
            instructions.push(instruction);
        }
        Ok(CompletedBody { instructions })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_backward_and_end_targets_resolve_to_local_instructions() {
        let mut builder = CodeBuilder::new();
        let start = builder.new_target();
        let end = builder.new_target();
        builder.complete_target(start).unwrap();
        builder.emit_jump(end).unwrap();
        builder.emit_jump_if_zero(start).unwrap();
        builder.complete_target(end).unwrap();
        assert_eq!(
            builder.finish().unwrap().into_instructions(),
            vec![
                Instruction::Jump(2),
                Instruction::JumpIfZero(0),
                Instruction::Return
            ]
        );
    }

    #[test]
    fn foreign_target_and_duplicate_completion_are_atomic() {
        let mut builder = CodeBuilder::new();
        let mut other = CodeBuilder::new();
        let own = builder.new_target();
        let foreign = other.new_target();
        assert_eq!(builder.emit_jump(foreign), Err(BuildError::ForeignTarget));
        assert_eq!(
            builder.emit_jump_if_zero(foreign),
            Err(BuildError::ForeignTarget)
        );
        assert_eq!(
            builder.complete_target(foreign),
            Err(BuildError::ForeignTarget)
        );
        builder.complete_target(own).unwrap();
        builder.emit(Instruction::PushConst(7)).unwrap();
        assert_eq!(
            builder.complete_target(own),
            Err(BuildError::DuplicateTarget)
        );
        builder.emit_jump(own).unwrap();
        assert_eq!(
            builder.finish().unwrap().into_instructions(),
            vec![
                Instruction::PushConst(7),
                Instruction::Jump(0),
                Instruction::Return
            ]
        );
        other.complete_target(foreign).unwrap();
    }

    #[test]
    fn unresolved_target_and_raw_branch_do_not_complete() {
        let mut builder = CodeBuilder::new();
        let target = builder.new_target();
        builder.emit_jump(target).unwrap();
        assert_eq!(
            builder.emit(Instruction::Jump(99)),
            Err(BuildError::RawBranch)
        );
        assert_eq!(builder.finish(), Err(BuildError::UnresolvedTarget));

        let mut unused = CodeBuilder::new();
        unused.new_target();
        assert_eq!(unused.finish(), Err(BuildError::UnresolvedTarget));
    }

    #[test]
    fn numeric_labels_support_forward_backward_and_end() {
        let mut builder = CodeBuilder::new();
        builder.define_numeric_label(0).unwrap();
        builder.emit_numeric_jump(32767).unwrap();
        builder.emit_numeric_jump(0).unwrap();
        builder.define_numeric_label(32767).unwrap();
        assert_eq!(
            builder.finish().unwrap().into_instructions(),
            vec![
                Instruction::Jump(2),
                Instruction::Jump(0),
                Instruction::Return
            ]
        );
    }

    #[test]
    fn numeric_label_failures_leave_code_and_first_definition_intact() {
        let mut builder = CodeBuilder::new();
        builder.define_numeric_label(12).unwrap();
        builder.emit(Instruction::PushConst(1)).unwrap();
        assert_eq!(
            builder.define_numeric_label(12),
            Err(BuildError::DuplicateLabel)
        );
        for value in [-1, 32768, 65536] {
            assert_eq!(
                builder.define_numeric_label(value),
                Err(BuildError::LabelOutOfRange)
            );
            assert_eq!(
                builder.emit_numeric_jump(value),
                Err(BuildError::LabelOutOfRange)
            );
        }
        builder.emit_numeric_jump(12).unwrap();
        assert_eq!(
            builder.finish().unwrap().into_instructions(),
            vec![
                Instruction::PushConst(1),
                Instruction::Jump(0),
                Instruction::Return
            ]
        );

        let mut unresolved = CodeBuilder::new();
        unresolved.emit_numeric_jump(1).unwrap();
        assert_eq!(unresolved.finish(), Err(BuildError::UnresolvedLabel));
    }

    #[test]
    fn completed_call_contains_only_executable_id_and_fallthrough_returns() {
        let mut builder = CodeBuilder::new();
        builder.emit_call(ExecutableId(42));
        assert_eq!(
            builder.finish().unwrap().into_instructions(),
            vec![Instruction::Call(ExecutableId(42)), Instruction::Return]
        );
    }

    #[test]
    fn fragment_append_rejects_raw_branch_without_partial_change() {
        let mut builder = CodeBuilder::new();
        builder.emit(Instruction::PushConst(99)).unwrap();
        assert_eq!(
            builder.append_fragment(vec![Instruction::PushConst(1), Instruction::Jump(0)]),
            Err(BuildError::RawBranch)
        );
        assert_eq!(
            builder.finish().unwrap().into_instructions(),
            vec![Instruction::PushConst(99), Instruction::Return]
        );
    }
}
