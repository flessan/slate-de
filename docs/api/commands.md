# Commands API

In Slate-DE, "Everything is a Command". This document explains how to implement and register new commands.

## Implementing the `Command` Trait

```rust
use commands::Command;
use anyhow::Result;

pub struct MyCommand;

impl Command for MyCommand {
    fn name(&self) -> &str { "my-command" }
    fn description(&self) -> &str { "Does something useful" }
    
    fn execute(&self, args: Vec<String>) -> Result<Option<String>> {
        println!("Executing with args: {:?}", args);
        Ok(Some("Success".to_string()))
    }
}
```

## Registering a Command

```rust
let mut registry = CommandRegistry::new();
registry.register(Arc::new(MyCommand));
```

## Invoking via CLI

Once registered, the command can be invoked via the global search overlay or the CLI:

```bash
slate-de my-command arg1 arg2
```
