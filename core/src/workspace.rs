//! Workspace model: named desktops, each with its own layout.
//!
//! ```
//! use slate_core::workspace::Workspaces;
//! let mut ws = Workspaces::builtins();
//! assert!(ws.switch("coding"));
//! assert_eq!(ws.current_name(), "coding");
//! ```

use crate::layout::{LayoutSpec, LAYOUT_PRESETS};

/// A named workspace with a parsed layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workspace {
    pub name: String,
    pub layout: LayoutSpec,
}

impl Workspace {
    pub fn new(name: impl Into<String>, layout: LayoutSpec) -> Self {
        Workspace { name: name.into(), layout }
    }
}

/// The set of workspaces and which one is active.
#[derive(Clone, Debug)]
pub struct Workspaces {
    list: Vec<Workspace>,
    current: usize,
}

impl Default for Workspaces {
    fn default() -> Self {
        Self::builtins()
    }
}

impl Workspaces {
    /// The built-in preset desktops.
    pub fn builtins() -> Self {
        let list = LAYOUT_PRESETS
            .iter()
            .map(|(name, spec)| {
                let layout = LayoutSpec::parse(spec)
                    .unwrap_or_else(|_| LayoutSpec::panel("prompt"));
                Workspace::new(*name, layout)
            })
            .collect();
        Workspaces { list, current: 0 }
    }

    pub fn from_workspaces(list: Vec<Workspace>) -> Self {
        let list = if list.is_empty() { vec![Workspace::new("main", LayoutSpec::panel("prompt"))] } else { list };
        Workspaces { list, current: 0 }
    }

    pub fn current(&self) -> &Workspace {
        &self.list[self.current]
    }

    pub fn current_name(&self) -> &str {
        &self.list[self.current].name
    }

    pub fn names(&self) -> Vec<&str> {
        self.list.iter().map(|w| w.name.as_str()).collect()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.list.iter().any(|w| w.name == name)
    }

    pub fn get(&self, name: &str) -> Option<&Workspace> {
        self.list.iter().find(|w| w.name == name)
    }

    /// Switch to a workspace by name. Returns `false` when it does not exist.
    pub fn switch(&mut self, name: &str) -> bool {
        if let Some(i) = self.list.iter().position(|w| w.name == name) {
            self.current = i;
            true
        } else {
            false
        }
    }

    /// Cycle to the next workspace (wrapping).
    pub fn next(&mut self) {
        self.current = (self.current + 1) % self.list.len();
    }

    /// Cycle to the previous workspace (wrapping).
    pub fn prev(&mut self) {
        self.current = (self.current + self.list.len() - 1) % self.list.len();
    }

    /// Replace (or insert) the layout of a workspace.
    pub fn set_layout(&mut self, name: &str, layout: LayoutSpec) {
        if let Some(w) = self.list.iter_mut().find(|w| w.name == name) {
            w.layout = layout;
        } else {
            self.list.push(Workspace::new(name, layout));
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Workspace> {
        self.list.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_loaded() {
        let ws = Workspaces::builtins();
        assert_eq!(ws.current_name(), "main");
        assert!(ws.contains("coding"));
        assert!(ws.contains("focus"));
        assert!(!ws.contains("nope"));
    }

    #[test]
    fn switching_and_cycling() {
        let mut ws = Workspaces::builtins();
        assert!(ws.switch("gaming"));
        assert_eq!(ws.current_name(), "gaming");
        assert!(!ws.switch("nonexistent"));
        assert_eq!(ws.current_name(), "gaming");

        ws.next();
        assert_eq!(ws.current_name(), "focus"); // preset order: …, gaming, focus
        ws.next();
        assert_eq!(ws.current_name(), "main"); // wrapped past the end
        ws.prev();
        assert_eq!(ws.current_name(), "focus");
    }

    #[test]
    fn empty_list_gets_a_main_workspace() {
        let ws = Workspaces::from_workspaces(vec![]);
        assert_eq!(ws.current_name(), "main");
    }

    #[test]
    fn set_layout_inserts_and_replaces() {
        let mut ws = Workspaces::builtins();
        let spec = LayoutSpec::parse("prompt:50 | notes:50").unwrap();
        ws.set_layout("custom", spec.clone());
        assert!(ws.contains("custom"));
        ws.set_layout("custom", LayoutSpec::panel("prompt"));
        assert_eq!(ws.get("custom").unwrap().layout.names(), vec!["prompt"]);
    }
}
