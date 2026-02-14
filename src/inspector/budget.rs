use soroban_env_host::Host;

/// Tracks resource usage (CPU and memory budget)
pub struct BudgetInspector;

impl BudgetInspector {
    /// Get CPU instruction usage from host
    pub fn get_cpu_usage(host: &Host) -> BudgetInfo {
        let budget = host.budget_cloned();
        
        BudgetInfo {
            cpu_instructions: budget.get_cpu_insns_consumed().unwrap_or(0),
            cpu_limit: budget.get_cpu_insns_limit(),
            memory_bytes: budget.get_mem_bytes_consumed().unwrap_or(0),
            memory_limit: budget.get_mem_bytes_limit(),
        }
    }

    /// Display budget information
    pub fn display(host: &Host) {
        let info = Self::get_cpu_usage(host);
        
        println!("Resource Budget:");
        println!(
            "  CPU: {} / {} ({:.1}%)",
            info.cpu_instructions,
            info.cpu_limit,
            info.cpu_percentage()
        );
        println!(
            "  Memory: {} / {} bytes ({:.1}%)",
            info.memory_bytes,
            info.memory_limit,
            info.memory_percentage()
        );

        // Warn if approaching limits
        if info.cpu_percentage() > 80.0 {
            println!("  WARNING: High CPU usage!");
        }
        if info.memory_percentage() > 80.0 {
            println!("  WARNING: High memory usage!");
        }
    }
}

/// Budget information snapshot
#[derive(Debug, Clone)]
pub struct BudgetInfo {
    pub cpu_instructions: u64,
    pub cpu_limit: u64,
    pub memory_bytes: u64,
    pub memory_limit: u64,
}

impl BudgetInfo {
    /// Calculate CPU usage percentage
    pub fn cpu_percentage(&self) -> f64 {
        if self.cpu_limit == 0 {
            0.0
        } else {
            (self.cpu_instructions as f64 / self.cpu_limit as f64) * 100.0
        }
    }

    /// Calculate memory usage percentage
    pub fn memory_percentage(&self) -> f64 {
        if self.memory_limit == 0 {
            0.0
        } else {
            (self.memory_bytes as f64 / self.memory_limit as f64) * 100.0
        }
    }
}