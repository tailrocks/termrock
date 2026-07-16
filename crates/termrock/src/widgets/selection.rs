#[derive(Debug, Clone, PartialEq, Eq)]
/// Data carried by `Selection`.
pub struct Selection<Id> {
    checked: Vec<Id>,
}

impl<Id> Default for Selection<Id> {
    fn default() -> Self {
        Self {
            checked: Vec::new(),
        }
    }
}

impl<Id> Selection<Id> {
    #[must_use]
    /// Creates a new value with canonical defaults.
    pub const fn new() -> Self {
        Self {
            checked: Vec::new(),
        }
    }

    #[must_use]
    /// Performs the `checked` operation.
    pub fn checked(&self) -> &[Id] {
        &self.checked
    }

    /// Performs the `clear` operation.
    pub fn clear(&mut self) {
        self.checked.clear();
    }
}

impl<Id: Clone + PartialEq> Selection<Id> {
    /// Toggle a stable identity, preserving check order.
    ///
    /// Returns whether the identity is checked after the toggle.
    pub fn toggle(&mut self, id: &Id) -> bool {
        if let Some(index) = self.checked.iter().position(|checked| checked == id) {
            self.checked.remove(index);
            false
        } else {
            self.checked.push(id.clone());
            true
        }
    }

    #[must_use]
    /// Returns whether `checked`.
    pub fn is_checked(&self, id: &Id) -> bool {
        self.checked.iter().any(|checked| checked == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_preserves_check_order_and_clear_resets() {
        let mut selection = Selection::new();

        assert!(selection.toggle(&"beta"));
        assert!(selection.toggle(&"alpha"));
        assert_eq!(selection.checked(), ["beta", "alpha"]);
        assert!(!selection.toggle(&"beta"));
        assert_eq!(selection.checked(), ["alpha"]);
        assert!(selection.is_checked(&"alpha"));

        selection.clear();
        assert!(selection.checked().is_empty());
    }
}
