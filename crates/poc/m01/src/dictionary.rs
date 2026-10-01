use crate::DefinitionId;

#[derive(Default)]
pub(crate) struct RuntimeDictionary {
    bindings: Vec<(String, DefinitionId)>,
}

impl RuntimeDictionary {
    pub(crate) fn lookup(&self, name: &str) -> Option<DefinitionId> {
        self.bindings
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, id)| *id)
    }

    pub(crate) fn bind(&mut self, name: &str, id: DefinitionId) {
        if let Some((_, bound_id)) = self.bindings.iter_mut().find(|(key, _)| key == name) {
            *bound_id = id;
        } else {
            self.bindings.push((name.to_owned(), id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_bind_and_rebind() {
        let mut dictionary = RuntimeDictionary::default();
        assert_eq!(dictionary.lookup("A"), None);
        dictionary.bind("A", DefinitionId(0));
        dictionary.bind("S", DefinitionId(1));
        assert_eq!(dictionary.lookup("A"), Some(DefinitionId(0)));
        dictionary.bind("A", DefinitionId(2));
        assert_eq!(dictionary.lookup("A"), Some(DefinitionId(2)));
        assert_eq!(dictionary.lookup("S"), Some(DefinitionId(1)));
    }
}
