//! `Cart.toml` manifest types and serialization.
//!
//! The manifest mirrors the `Cargo.toml` structure as defined in the
//! technical design section "Cart.toml". This module provides the types and
//! helpers that the `cart` tool uses to read, write, and modify manifests.

use serde::{Deserialize, Serialize};

/// The root `Cart.toml` manifest.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CartManifest {
    pub package: Package,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank: Option<Bank>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rom: Vec<Rom>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub dependencies: std::collections::BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<TargetSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<Features>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<Run>,
}

/// The `[package]` section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub edition: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

/// The `[bank]` section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Bank {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// A `[[rom]]` section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Rom {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub target: String,
}

/// The `[target]` section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TargetSection {
    pub default: String,
}

/// The `[features]` section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Features {
    #[serde(flatten)]
    pub flags: std::collections::BTreeMap<String, Vec<String>>,
}

/// The `[run]` section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Run {
    pub emulator: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
}

impl CartManifest {
    /// Parse a manifest from TOML text.
    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }

    /// Serialize the manifest to TOML text.
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }
}