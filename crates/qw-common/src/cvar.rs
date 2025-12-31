// Minimal cvar registry.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Cvar {
    pub name: String,
    pub value: String,
    pub archive: bool,
    pub info: bool,
    pub float_value: f32,
}

impl Cvar {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        let value = value.into();
        let float_value = value.parse::<f32>().unwrap_or(0.0);
        Self {
            name: name.into(),
            value,
            archive: false,
            info: false,
            float_value,
        }
    }

    pub fn with_flags(mut self, archive: bool, info: bool) -> Self {
        self.archive = archive;
        self.info = info;
        self
    }
}

#[derive(Debug, Default)]
pub struct CvarRegistry {
    vars: HashMap<String, Cvar>,
}

impl CvarRegistry {
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    pub fn register(&mut self, var: Cvar) {
        self.vars.entry(var.name.clone()).or_insert(var);
    }

    pub fn set(&mut self, name: &str, value: &str) {
        let float_value = value.parse::<f32>().unwrap_or(0.0);
        if let Some(var) = self.vars.get_mut(name) {
            var.value = value.to_string();
            var.float_value = float_value;
        } else {
            let mut var = Cvar::new(name, value);
            var.float_value = float_value;
            self.vars.insert(name.to_string(), var);
        }
    }

    pub fn value(&self, name: &str) -> f32 {
        self.vars.get(name).map(|v| v.float_value).unwrap_or(0.0)
    }

    pub fn string(&self, name: &str) -> String {
        self.vars
            .get(name)
            .map(|v| v.value.clone())
            .unwrap_or_default()
    }

    pub fn get(&self, name: &str) -> Option<&Cvar> {
        self.vars.get(name)
    }

    pub fn iter_archive(&self) -> impl Iterator<Item = &Cvar> {
        self.vars.values().filter(|v| v.archive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_and_sets() {
        let mut registry = CvarRegistry::new();
        registry.register(Cvar::new("r_draworder", "1"));
        assert_eq!(registry.value("r_draworder"), 1.0);

        registry.set("r_draworder", "0");
        assert_eq!(registry.value("r_draworder"), 0.0);
        assert_eq!(registry.string("r_draworder"), "0");
    }

    #[test]
    fn creates_on_set() {
        let mut registry = CvarRegistry::new();
        registry.set("sv_maxspeed", "320");
        assert_eq!(registry.value("sv_maxspeed"), 320.0);
    }
}
