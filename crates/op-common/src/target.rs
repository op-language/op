//! Target descriptor types.
//!
//! The [`TargetTriplet`] parses the `cpu-manufacturer-machine-variant` string
//! defined in the language specification. Libs and the registry use the
//! triplet to select the CPU family and the platform.

use serde::{Deserialize, Serialize};

/// A target triplet split into its four components.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TargetTriplet {
    pub cpu: String,
    pub manufacturer: String,
    pub machine: String,
    pub variant: String,
}

impl TargetTriplet {
    /// Parse a triplet string of the form `cpu-manufacturer-machine-variant`.
    ///
    /// The variant component is optional; an empty string is stored when it
    /// is absent.
    pub fn parse(triplet: &str) -> Result<Self, TripletError> {
        let parts: Vec<&str> = triplet.split('-').collect();
        match parts.len() {
            4 => Ok(Self {
                cpu: parts[0].to_string(),
                manufacturer: parts[1].to_string(),
                machine: parts[2].to_string(),
                variant: parts[3].to_string(),
            }),
            3 => Ok(Self {
                cpu: parts[0].to_string(),
                manufacturer: parts[1].to_string(),
                machine: parts[2].to_string(),
                variant: String::new(),
            }),
            _ => Err(TripletError::Malformed(triplet.to_string())),
        }
    }

    /// Render the triplet back to its canonical string form.
    pub fn as_str(&self) -> String {
        if self.variant.is_empty() {
            format!("{}-{}-{}", self.cpu, self.manufacturer, self.machine)
        } else {
            format!(
                "{}-{}-{}-{}",
                self.cpu, self.manufacturer, self.machine, self.variant
            )
        }
    }
}

impl std::fmt::Display for TargetTriplet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.as_str())
    }
}

/// Errors that arise while parsing a target triplet.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TripletError {
    #[error("malformed target triplet: {0}")]
    Malformed(String),
}