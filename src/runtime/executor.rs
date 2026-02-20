use crate::utils::ArgumentParser;
use crate::{DebuggerError, Result};
use soroban_env_host::{DiagnosticLevel, Host};
use soroban_sdk::{Address, Env, InvokeError, Symbol, Val, Vec as SorobanVec};
use std::collections::HashMap;
use tracing::{info, warn};

/// Executes Soroban contracts in a test environment
pub struct ContractExecutor {
    env: Env,
    contract_address: Address,
    extra_contracts: HashMap<String, String>,
}

impl ContractExecutor {
    /// Create a new contract executor
    pub fn new(wasm: Vec<u8>) -> Result<Self> {
        info!("Initializing contract executor");

        // Create a test environment
        let env = Env::default();
        let _ = env.host().set_diagnostic_level(DiagnosticLevel::Debug);

        // Enable diagnostic events
        env.host()
            .set_diagnostic_level(DiagnosticLevel::Debug)
            .expect("Failed to set diagnostic level");

        // Register the contract with the WASM
        let contract_address = env.register(wasm.as_slice(), ());

        info!("Contract registered successfully");

        Ok(Self {
            env,
            contract_address,
            extra_contracts: HashMap::new(),
        })
    }

    /// Register an additional contract and associate it with an alias
    pub fn register_extra_contract(&mut self, alias: &str, wasm: &[u8]) -> Result<()> {
        let address = self.env.register(wasm, ());
        let address_str = address.to_string().to_string();
        info!("Registered extra contract '{}' at {}", alias, address_str);
        self.extra_contracts.insert(alias.to_string(), address_str);
        Ok(())
    }

    /// Execute a contract function
    pub fn execute(&self, function: &str, args: Option<&str>) -> Result<String> {
        info!("Executing function: {}", function);

        // Convert function name to Symbol
        let func_symbol = Symbol::new(&self.env, function);

        // Substitute aliases in arguments if they exist
        let substituted_args = args.map(|a| {
            let mut result = a.to_string();
            for (alias, address) in &self.extra_contracts {
                // Replace "alias" with "C..." in JSON arguments
                let search = format!("\"{}\"", alias);
                let replace = format!("\"{}\"", address);
                result = result.replace(&search, &replace);
            }
            result
        });

        // Parse arguments (simplified for now)
        let parsed_args = if let Some(args_json) = substituted_args.as_deref() {
            self.parse_args(args_json)?
        } else {
            vec![]
        };

        // Create argument vector
        let args_vec = if parsed_args.is_empty() {
            SorobanVec::<Val>::new(&self.env)
        } else {
            SorobanVec::from_slice(&self.env, &parsed_args)
        };

        // Call the contract
        // try_invoke_contract returns Result<Result<Val, ConversionError>, Result<InvokeError, InvokeError>>
        match self.env.try_invoke_contract::<Val, InvokeError>(
            &self.contract_address,
            &func_symbol,
            args_vec,
        ) {
            Ok(Ok(val)) => {
                info!("Function executed successfully");
                Ok(format!("{:?}", val))
            }
            Ok(Err(conv_err)) => {
                warn!("Return value conversion failed: {:?}", conv_err);
                Err(DebuggerError::ExecutionError(format!(
                    "Return value conversion failed: {:?}",
                    conv_err
                ))
                .into())
            }
            Err(Ok(inv_err)) => match inv_err {
                InvokeError::Contract(code) => {
                    warn!("Contract returned error code: {}", code);
                    Err(
                        DebuggerError::ExecutionError(format!("Contract error code: {}", code))
                            .into(),
                    )
                }
                InvokeError::Abort => {
                    warn!("Contract execution aborted");
                    Err(
                        DebuggerError::ExecutionError("Contract execution aborted".to_string())
                            .into(),
                    )
                }
            },
            Err(Err(inv_err)) => {
                warn!("Invocation error conversion failed: {:?}", inv_err);
                Err(DebuggerError::ExecutionError(format!(
                    "Invocation error conversion failed: {:?}",
                    inv_err
                ))
                .into())
            }
        }
    }

    /// Set initial storage state
    pub fn set_initial_storage(&mut self, _storage_json: String) -> Result<()> {
        // TODO: Implement storage initialization
        info!("Setting initial storage (not yet implemented)");
        Ok(())
    }

    /// Get the host instance
    pub fn host(&self) -> &Host {
        self.env.host()
    }

    /// Get the environment handle
    pub fn env(&self) -> Env {
        self.env.clone()
    }

    /// Get the authorization tree from the environment
    pub fn get_auth_tree(&self) -> Result<Vec<crate::inspector::auth::AuthNode>> {
        crate::inspector::auth::AuthInspector::get_auth_tree(&self.env)
    }

    /// Parse JSON arguments into contract values
    fn parse_args(&self, args_json: &str) -> Result<Vec<Val>> {
        info!("Parsing arguments: {}", args_json);
        let parser = ArgumentParser::new(self.env.clone());
        parser.parse_args_string(args_json).map_err(|e| {
            warn!("Failed to parse arguments: {}", e);
            DebuggerError::InvalidArguments(e.to_string()).into()
        })
    }

    /// Get events captured during execution
    pub fn get_events(&self) -> Result<Vec<crate::inspector::events::ContractEvent>> {
        crate::inspector::events::EventInspector::get_events(self.env.host())
    }

    /// Get diagnostic events (used for call tracing)
    #[cfg(any(test, feature = "testutils"))]
    pub fn get_diagnostic_events(&self) -> Result<Vec<crate::inspector::events::ContractEvent>> {
        crate::inspector::events::EventInspector::get_diagnostic_events(self.env.host())
    }

    /// Attempts to reverse-map a registered generated address to the user-provided alias
    pub fn resolve_alias(&self, address: &str) -> String {
        for (alias, registered_addr) in &self.extra_contracts {
            if registered_addr == address {
                return alias.clone();
            }
        }
        
        // Also check main contract address
        if address == self.contract_address.to_string().to_string() {
            return "main".to_string();
        }

        address.to_string()
    /// Get diagnostic events from the host
    pub fn get_diagnostic_events(&self) -> Result<Vec<soroban_env_host::xdr::ContractEvent>> {
        Ok(self
            .env
            .host()
            .get_diagnostic_events()?
            .0
            .into_iter()
            .map(|he| he.event)
            .collect())
    }
}
