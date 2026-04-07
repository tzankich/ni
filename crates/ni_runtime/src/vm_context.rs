use crate::error::NiResult;
use crate::value::NiValue;

/// Trait that generated code uses to interact with the host environment.
/// The engine provides an implementation of this trait.
pub trait NiVm {
    fn call_function(&mut self, name: &str, args: &[NiValue]) -> NiResult<NiValue>;
    fn get_global(&self, name: &str) -> Option<NiValue>;
    fn set_global(&mut self, name: &str, value: NiValue);
    fn print(&mut self, value: &str);
}

/// A simple standalone VM implementation for testing generated code.
pub struct SimpleVm {
    pub globals: std::collections::HashMap<String, NiValue>,
    pub output: Vec<String>,
}

impl Default for SimpleVm {
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleVm {
    pub fn new() -> Self {
        Self {
            globals: std::collections::HashMap::new(),
            output: Vec::new(),
        }
    }
}

impl NiVm for SimpleVm {
    fn call_function(&mut self, name: &str, _args: &[NiValue]) -> NiResult<NiValue> {
        if let Some(val) = self.globals.get(name) {
            Ok(val.clone())
        } else {
            Err(crate::error::NiRuntimeError::new(format!(
                "Function '{}' not found",
                name
            )))
        }
    }

    fn get_global(&self, name: &str) -> Option<NiValue> {
        self.globals.get(name).cloned()
    }

    fn set_global(&mut self, name: &str, value: NiValue) {
        self.globals.insert(name.to_string(), value);
    }

    fn print(&mut self, value: &str) {
        self.output.push(value.to_string());
    }
}
