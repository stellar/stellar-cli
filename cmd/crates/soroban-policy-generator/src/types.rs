use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MethodConfig {
    pub name: String,
    pub allowed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub methods: Vec<MethodConfig>,
} 