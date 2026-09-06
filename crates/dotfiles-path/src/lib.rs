//! Validated path and name primitives shared between the dotfiles crates.
//!
//! This crate has no dependencies and performs no IO. It exists so that
//! `deps-core` and `config-manifest` can share parse-don't-validate newtypes
//! without either depending on the other: `RelPath` was written first and
//! happens to live in `config-manifest`, but a validated relative path is not
//! a git-sync concept.

mod rel;

pub use rel::{CheckRelPath, PathError};
