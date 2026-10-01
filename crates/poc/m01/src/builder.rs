use std::sync::atomic::{AtomicUsize, Ordering};

use crate::definition::{DefinitionId, Instruction};

static NEXT_BUILDER_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BuildError {
    UnknownRuntimeWord,
    ForeignBranchTarget,
    BranchTargetAlreadyCompleted,
    UnresolvedBranchTarget,
    InvalidBranchTarget,
}

/// A target is meaningful only to the builder that created it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BranchTarget {
    owner: usize,
    index: usize,
}

enum PendingInstruction {
    Call(DefinitionId),
    Jump(BranchTarget),
    JumpIfFalse(BranchTarget),
}

/// A compiled body whose local targets have been resolved and whose final
/// instruction is the ordinary Return inserted by completion.
pub(crate) struct CompletedBody {
    instructions: Vec<Instruction>,
}

impl CompletedBody {
    pub(crate) fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }

    #[cfg(test)]
    pub(crate) fn from_instructions_unchecked(instructions: Vec<Instruction>) -> Self {
        Self { instructions }
    }
}

pub(crate) struct CodeBuilder {
    owner: usize,
    instructions: Vec<PendingInstruction>,
    targets: Vec<Option<usize>>,
}

impl CodeBuilder {
    pub(crate) fn new() -> Self {
        let owner = NEXT_BUILDER_ID
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("builder identity exhausted");
        Self {
            owner,
            instructions: Vec::new(),
            targets: Vec::new(),
        }
    }

    pub(crate) fn emit_call(&mut self, id: DefinitionId) {
        self.instructions.push(PendingInstruction::Call(id));
    }

    pub(crate) fn new_target(&mut self) -> BranchTarget {
        let target = BranchTarget {
            owner: self.owner,
            index: self.targets.len(),
        };
        self.targets.push(None);
        target
    }

    fn check_target(&self, target: BranchTarget) -> Result<(), BuildError> {
        if target.owner != self.owner {
            return Err(BuildError::ForeignBranchTarget);
        }
        if target.index >= self.targets.len() {
            return Err(BuildError::InvalidBranchTarget);
        }
        Ok(())
    }

    pub(crate) fn emit_jump(&mut self, target: BranchTarget) -> Result<(), BuildError> {
        self.check_target(target)?;
        self.instructions.push(PendingInstruction::Jump(target));
        Ok(())
    }

    pub(crate) fn emit_jump_if_false(&mut self, target: BranchTarget) -> Result<(), BuildError> {
        self.check_target(target)?;
        self.instructions
            .push(PendingInstruction::JumpIfFalse(target));
        Ok(())
    }

    pub(crate) fn complete_target(&mut self, target: BranchTarget) -> Result<(), BuildError> {
        self.check_target(target)?;
        let position = &mut self.targets[target.index];
        if position.is_some() {
            return Err(BuildError::BranchTargetAlreadyCompleted);
        }
        *position = Some(self.instructions.len());
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<CompletedBody, BuildError> {
        if self.targets.iter().any(Option::is_none) {
            return Err(BuildError::UnresolvedBranchTarget);
        }

        let final_len = self.instructions.len() + 1; // includes the appended Return
        let mut instructions = Vec::with_capacity(final_len);
        for pending in self.instructions {
            let instruction = match pending {
                PendingInstruction::Call(id) => Instruction::Call(id),
                PendingInstruction::Jump(target) => Instruction::Jump(Self::resolve_target(
                    self.owner,
                    &self.targets,
                    target,
                    final_len,
                )?),
                PendingInstruction::JumpIfFalse(target) => Instruction::JumpIfFalse(
                    Self::resolve_target(self.owner, &self.targets, target, final_len)?,
                ),
            };
            instructions.push(instruction);
        }
        instructions.push(Instruction::Return);
        Ok(CompletedBody { instructions })
    }

    fn resolve_target(
        owner: usize,
        targets: &[Option<usize>],
        target: BranchTarget,
        final_len: usize,
    ) -> Result<usize, BuildError> {
        if target.owner != owner {
            return Err(BuildError::ForeignBranchTarget);
        }
        let position = targets
            .get(target.index)
            .ok_or(BuildError::InvalidBranchTarget)?
            .ok_or(BuildError::UnresolvedBranchTarget)?;
        if position >= final_len {
            return Err(BuildError::InvalidBranchTarget);
        }
        Ok(position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_and_backward_jumps_resolve_locally() {
        let mut builder = CodeBuilder::new();
        let forward = builder.new_target();
        let backward = builder.new_target();
        builder.emit_jump(forward).unwrap();
        builder.complete_target(backward).unwrap();
        builder.emit_jump(backward).unwrap();
        builder.complete_target(forward).unwrap();
        let body = builder.finish().unwrap();
        assert_eq!(
            body.instructions(),
            &[
                Instruction::Jump(2),
                Instruction::Jump(1),
                Instruction::Return
            ]
        );
    }

    #[test]
    fn conditional_end_target_points_to_appended_return() {
        let mut builder = CodeBuilder::new();
        let end = builder.new_target();
        builder.emit_jump_if_false(end).unwrap();
        builder.complete_target(end).unwrap();
        let body = builder.finish().unwrap();
        assert_eq!(
            body.instructions(),
            &[Instruction::JumpIfFalse(1), Instruction::Return]
        );
    }

    #[test]
    fn empty_body_gets_exactly_one_return() {
        let body = CodeBuilder::new().finish().unwrap();
        assert_eq!(body.instructions(), &[Instruction::Return]);
    }

    #[test]
    fn foreign_targets_are_rejected_at_emit_and_completion() {
        let mut first = CodeBuilder::new();
        let mut second = CodeBuilder::new();
        let target = first.new_target();
        let same_index = second.new_target();
        assert_eq!(target.index, same_index.index);
        assert_eq!(
            second.emit_jump(target),
            Err(BuildError::ForeignBranchTarget)
        );
        assert_eq!(
            second.emit_jump_if_false(target),
            Err(BuildError::ForeignBranchTarget)
        );
        assert_eq!(
            second.complete_target(target),
            Err(BuildError::ForeignBranchTarget)
        );
    }

    #[test]
    fn duplicate_and_unresolved_targets_are_rejected() {
        let mut builder = CodeBuilder::new();
        let completed = builder.new_target();
        builder.complete_target(completed).unwrap();
        assert_eq!(
            builder.complete_target(completed),
            Err(BuildError::BranchTargetAlreadyCompleted)
        );
        builder.new_target();
        assert!(matches!(
            builder.finish(),
            Err(BuildError::UnresolvedBranchTarget)
        ));
    }

    #[test]
    fn invalid_target_is_rejected() {
        let mut builder = CodeBuilder::new();
        let target = BranchTarget {
            owner: builder.owner,
            index: 0,
        };
        assert_eq!(
            builder.emit_jump(target),
            Err(BuildError::InvalidBranchTarget)
        );
        assert_eq!(
            builder.complete_target(target),
            Err(BuildError::InvalidBranchTarget)
        );
    }
}
