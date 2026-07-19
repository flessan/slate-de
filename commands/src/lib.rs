use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;

/// The core command trait that all system actions must implement.
pub trait Command: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn execute(&self, args: Vec<String>) -> Result<Option<String>>;
}

pub struct CommandRegistry {
    commands: HashMap<String, Arc<dyn Command>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    pub fn register(&mut self, cmd: Arc<dyn Command>) {
        self.commands.insert(cmd.name().to_string(), cmd);
    }

    pub fn execute(&self, name: &str, args: Vec<String>) -> Result<Option<String>> {
        if let Some(cmd) = self.commands.get(name) {
            cmd.execute(args)
        } else {
            Err(anyhow::anyhow!("Command not found: {}", name))
        }
    }

    pub fn list(&self) -> Vec<(&str, &str)> {
        self.commands
            .values()
            .map(|c| (c.name(), c.description()))
            .collect()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}
