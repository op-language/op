//! Shared types for the Op compiler and the `cart` build tool.
//!
//! This crate provides the common token stream, AST node, target descriptor,
//! and intermediate file envelope types described in the technical design.
//! Every other crate in the workspace depends on these types so that each
//! pipeline stage shares one representation of the data.

pub mod ast;
pub mod envelope;
pub mod target;
pub mod tokens;

pub use ast::{
    Access, AstFile, AttrArg, Attribute, BinaryOp, BranchHint, Condition, EnumVariant, Expr, Field,
    FnStmt, InitValue, Item, Module, OffsetOp, Operand, PlacementArg, SwitchCase, Type, UnaryOp,
    UseRoot, UseTail, UseTree,
};
pub use envelope::{from_json, to_json, Envelope};
pub use target::{TargetTriplet, TripletError};
pub use tokens::{Token, TokenStream, TokenType};
