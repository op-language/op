//! The target registry.
//!
//! The registry maps a target triplet string to a [`Target`] constructor. The
//! compiler queries the registry to load a target at build time. Libs
//! register themselves with the registry when they are loaded from
//! `~/.carts/`.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::Target;

/// A registry of known targets keyed by triplet string.
#[derive(Default)]
pub struct Registry {
    inner: RwLock<HashMap<String, Arc<dyn Target>>>,
}

impl Registry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a target under its triplet string.
    pub fn register(&self, target: Arc<dyn Target>) {
        let key = target.triplet().to_string();
        self.inner.write().unwrap().insert(key, target);
    }

    /// Look up a target by triplet string.
    pub fn get(&self, triplet: &str) -> Option<Arc<dyn Target>> {
        self.inner.read().unwrap().get(triplet).cloned()
    }

    /// List all registered triplet strings.
    pub fn triplets(&self) -> Vec<String> {
        self.inner.read().unwrap().keys().cloned().collect()
    }
}