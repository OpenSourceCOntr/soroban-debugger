use crate::debugger::engine::DebuggerEngine;
use crate::inspector::{BudgetInspector, StorageInspector};
use crate::Result;
use std::io::{self, Write};

/// Terminal user interface for interactive debugging.
pub struct DebuggerUI {
    engine: DebuggerEngine,
    storage_inspector: StorageInspector,
}

impl DebuggerUI {
    pub fn new(engine: DebuggerEngine) -> Result<Self> {
        Ok(Self {
            engine,
            storage_inspector: StorageInspector::new(),
        })
    }

    /// Get mutable reference to storage inspector
    pub fn storage_inspector_mut(&mut self) -> &mut StorageInspector {
        &mut self.storage_inspector
    }

    /// Run the interactive UI loop
    pub fn run(&mut self) -> Result<()> {
        self.print_help();

        loop {
            print!("\n(debug) ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            let command = input.trim();
            if command.is_empty() {
                continue;
            }

            match self.handle_command(command) {
                Ok(should_exit) => {
                    if should_exit {
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Command execution error");
                }
            }
        }

        Ok(())
    }

    fn handle_command(&mut self, command: &str) -> Result<bool> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(false);
        }

        match parts[0] {
            "run" => {
                if parts.len() < 2 {
                    println!("Usage: run <function_name> [args_json]");
                } else {
                    let function = parts[1];
                    let args = if parts.len() > 2 {
                        Some(parts[2..].join(" "))
                    } else {
                        None
                    };
                    println!("\n--- Execution Start: {} ---", function);
                    match self.engine.execute(function, args.as_deref()) {
                        Ok(result) => {
                            if self.engine.is_paused() {
                                self.render_breakpoint_hit();
                            }
                            println!("\n--- Execution Complete ---");
                            println!("Result: {:?}", result);
                        }
                        Err(e) => {
                            println!("\n--- Execution Failed ---");
                            println!("Error: {}", e);
                            self.engine.state().call_stack().display();
                        }
                    }
                }
            }
            "s" | "step" => {
                self.engine.step()?;
                if let Ok(state) = self.engine.state().lock() {
                    crate::logging::log_step(state.step_count() as u64);
                }
            }
            "c" | "continue" => {
                self.engine.continue_execution()?;
                tracing::info!("Execution continuing");
            }
            "i" | "inspect" => {
                self.inspect();
            }
            "storage" => {
                self.storage_inspector.display();
            }
            "stack" => {
                if let Ok(state) = self.engine.state().lock() {
                    state.call_stack().display();
                }
            }
            "budget" => {
                BudgetInspector::display(self.engine.executor().host());
            }
            "break" => {
                if parts.len() < 2 {
                    tracing::warn!("breakpoint set without function name");
                } else {
                    self.engine.breakpoints_mut().add(parts[1]);
                    crate::logging::log_breakpoint_set(parts[1]);
                }
            }
            "list-breaks" => {
                let breakpoints = self.engine.breakpoints_mut().list();
                if breakpoints.is_empty() {
                    println!("No breakpoints set");
                } else {
                    for bp in breakpoints {
                        println!("- {}", bp);
                    }
                }
            }
            "clear" => {
                if parts.len() < 2 {
                    tracing::warn!("clear command missing function name");
                } else if self.engine.breakpoints_mut().remove(parts[1]) {
                    crate::logging::log_breakpoint_cleared(parts[1]);
                } else {
                    tracing::debug!(breakpoint = parts[1], "No breakpoint found at function");
                }
            }
            "help" => self.print_help(),
            "q" | "quit" | "exit" => {
                tracing::info!("Exiting debugger");
                return Ok(true);
            }
            _ => tracing::warn!(command = parts[0], "Unknown command"),
        }

        Ok(false)
    }

    /// Render a pretty breakpoint hit display
    fn render_breakpoint_hit(&self) {
        let state = self.engine.state();
        let current_func = state.current_function().unwrap_or("unknown");
        let args = state.current_args().unwrap_or("none");
        let stack = state.call_stack().get_stack();
        
        // Find previous frame if it exists
        let prev_func = if stack.len() > 1 {
            stack[stack.len() - 2].function.as_str()
        } else {
            "none"
        };

        println!("\n┌────────────────────────────────────────────────────────────────────────┐");
        println!("│ 🛑 BREAKPOINT HIT                                                      │");
        println!("├────────────────────────────────────────────────────────────────────────┤");
        println!("│ {:<14} │ {:<53} │", "Function", current_func);
        println!("│ {:<14} │ {:<53} │", "Arguments", args);
        println!("│ {:<14} │ {:<53} │", "Previous", prev_func);
        println!("├────────────────────────────────────────────────────────────────────────┤");
        println!("│ STORAGE STATE                                                          │");
        
        let storage = self.storage_inspector.get_all();
        if storage.is_empty() {
            println!("│ (empty)                                                                │");
        } else {
            let mut keys: Vec<&String> = storage.keys().collect();
            keys.sort();
            for key in keys.iter().take(5) { // Show first 5 entries
                let val = &storage[*key];
                let entry = format!("{} = {}", key, val);
                println!("│ {:<70} │", if entry.len() > 68 { format!("{}...", &entry[..65]) } else { entry });
            }
            if storage.len() > 5 {
                println!("│ ... (and {} more)                                                     │", storage.len() - 5);
            }
        }
        println!("└────────────────────────────────────────────────────────────────────────┘");
    }

    /// Display current state
    fn inspect(&self) {
        if self.engine.is_paused() {
            self.render_breakpoint_hit();
        } else {
            println!("\n=== Current State ===");
            if let Some(func) = self.engine.state().current_function() {
                println!("Function: {}", func);
            } else {
                println!("Function: (none)");
            }
            println!("Steps: {}", self.engine.state().step_count());
            println!("Paused: {}", self.engine.is_paused());

            println!();
            self.engine.state().call_stack().display();
        }
    }

    fn print_help(&self) {
        println!("\nAvailable commands:");
        println!("  run <func> [args]    Run a contract function");
        println!("  s, step              Execute next instruction");
        println!("  c, continue          Run until breakpoint or completion");
        println!("  i, inspect           Show current execution state");
        println!("  storage              Display contract storage");
        println!("  stack                Show call stack");
        println!("  budget               Show resource usage (CPU/memory)");
        println!("  break <function>     Set breakpoint at function");
        println!("  list-breaks          List all breakpoints");
        println!("  clear <function>     Remove breakpoint");
        println!("  help                 Show this help message");
        println!("  q, quit              Exit debugger");
    }
}
