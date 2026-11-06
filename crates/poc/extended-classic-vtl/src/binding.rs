use std::collections::HashMap;

use crate::executable::ExecutableId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SourceRole {
    Write,
    PrimaryRead,
    AppliedRead,
    BinaryOperator,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BindingError {
    AlreadyBound,
}

#[derive(Default)]
pub(crate) struct Bindings {
    entries: HashMap<(char, SourceRole), ExecutableId>,
}

impl Bindings {
    pub(crate) fn resolve(&self, identity: char, role: SourceRole) -> Option<ExecutableId> {
        self.entries.get(&(identity, role)).copied()
    }

    pub(crate) fn bind_initial(
        &mut self,
        identity: char,
        role: SourceRole,
        id: ExecutableId,
    ) -> Result<(), BindingError> {
        use std::collections::hash_map::Entry;
        match self.entries.entry((identity, role)) {
            Entry::Vacant(slot) => {
                slot.insert(id);
                Ok(())
            }
            Entry::Occupied(_) => Err(BindingError::AlreadyBound),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_are_independent_and_duplicate_binding_keeps_original() {
        let mut bindings = Bindings::default();
        let first = ExecutableId(1);
        let second = ExecutableId(2);
        assert_eq!(bindings.resolve('a', SourceRole::Write), None);
        bindings
            .bind_initial('a', SourceRole::Write, first)
            .unwrap();
        for role in [
            SourceRole::PrimaryRead,
            SourceRole::AppliedRead,
            SourceRole::BinaryOperator,
        ] {
            assert_eq!(bindings.resolve('a', role), None);
            bindings.bind_initial('a', role, second).unwrap();
            assert_eq!(bindings.resolve('a', role), Some(second));
        }
        assert_eq!(
            bindings.bind_initial('a', SourceRole::Write, second),
            Err(BindingError::AlreadyBound)
        );
        assert_eq!(bindings.resolve('a', SourceRole::Write), Some(first));
        assert_eq!(bindings.resolve('b', SourceRole::Write), None);
    }
}
