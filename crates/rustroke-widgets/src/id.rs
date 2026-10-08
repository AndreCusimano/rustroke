use std::hash::{DefaultHasher, Hash, Hasher};

/// Identifies a widget across frames, so state such as focus or an
/// ongoing drag can follow it.
///
/// Ids are hashes. By default each [`crate::Ui`] derives them from its own
/// id and a counter, so they stay stable as long as the same widgets are
/// added in the same order. Use [`crate::Ui::push_id`] around content that
/// appears, disappears or moves, e.g. items of a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id(u64);

impl Id {
    /// An id from any hashable value.
    pub fn new(source: impl Hash) -> Self {
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        Self(hasher.finish())
    }

    /// The hash value (e.g. to derive ids for other systems).
    pub fn value(self) -> u64 {
        self.0
    }

    /// A child id, derived from this one and `source`.
    pub fn with(self, source: impl Hash) -> Self {
        let mut hasher = DefaultHasher::new();
        self.0.hash(&mut hasher);
        source.hash(&mut hasher);
        Self(hasher.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_deterministic_and_distinct() {
        assert_eq!(Id::new("a").with(1), Id::new("a").with(1));
        assert_ne!(Id::new("a").with(1), Id::new("a").with(2));
        assert_ne!(Id::new("a").with("b"), Id::new("b").with("a"));
    }
}
