//! Shared types for the Op compiler and the `cart` build tool.
//!
//! This crate provides the common token stream, AST node, target descriptor,
//! and intermediate file envelope types described in the technical design.
//! Every other crate in the workspace depends on these types so that each
//! pipeline stage shares one representation of the data.

pub mod tokens;
pub mod ast;
pub mod target;
pub mod envelope;

pub use envelope::{from_json, to_json, Envelope};
pub use target::{TargetTriplet, TripletError};
pub use tokens::{Token, TokenStream};
pub use ast::AstFile;