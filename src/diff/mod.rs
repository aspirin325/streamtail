#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Unchanged,
    Appended(String),
    Reset(String),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DiffEngine {
    snapshot: String,
}

impl DiffEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, next: &str) -> Change {
        let change = if next == self.snapshot {
            Change::Unchanged
        } else if next.starts_with(&self.snapshot) {
            Change::Appended(next[self.snapshot.len()..].to_owned())
        } else {
            Change::Reset(next.to_owned())
        };

        self.snapshot.clear();
        self.snapshot.push_str(next);
        change
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_initial_content() {
        let mut engine = DiffEngine::new();

        assert_eq!(engine.update("hello"), Change::Appended("hello".to_owned()));
    }

    #[test]
    fn emits_only_appended_content() {
        let mut engine = DiffEngine::new();
        engine.update("hello");

        assert_eq!(
            engine.update("hello world"),
            Change::Appended(" world".to_owned())
        );
    }

    #[test]
    fn emits_unchanged_for_same_snapshot() {
        let mut engine = DiffEngine::new();
        engine.update("hello");

        assert_eq!(engine.update("hello"), Change::Unchanged);
    }

    #[test]
    fn resets_when_content_is_not_append_only() {
        let mut engine = DiffEngine::new();
        engine.update("hello");

        assert_eq!(
            engine.update("goodbye"),
            Change::Reset("goodbye".to_owned())
        );
    }
}
