//! Intermediate file envelope helpers.
//!
//! The `opc` pipeline uses JSON intermediate files (`.opx`, `.opa`, `.opl`).
//! Each file carries a `version` field so that future revisions can evolve the
//! formats. The [`Envelope`] trait gives every stage a uniform way to set and
//! check the version.

use serde::{de::DeserializeOwned, Serialize};

/// The intermediate file format version.
pub const CURRENT_VERSION: u32 = 1;

/// A type that can be wrapped in the standard intermediate file envelope.
pub trait Envelope: Serialize + DeserializeOwned {
    /// The intermediate file version.
    fn version(&self) -> u32;
}

/// Serialize an envelope value to a pretty JSON string.
pub fn to_json<E: Envelope>(envelope: &E) -> serde_json::Result<String> {
    serde_json::to_string_pretty(envelope)
}

/// Deserialize an envelope value from a JSON string.
pub fn from_json<E: Envelope>(json: &str) -> serde_json::Result<E> {
    serde_json::from_str(json)
}
