#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
//! Validated path and name primitives shared between the dotfiles crates.
//!
//! This crate has no dependencies and performs no IO. It exists so that
//! `deps-core` and `config-manifest` can share parse-don't-validate newtypes
//! without either depending on the other: `RelPath` was written first and
//! happens to live in `config-manifest`, but a validated relative path is not
//! a git-sync concept.

mod bounded;
mod name;
mod rel;

pub use bounded::BoundedText;
pub use name::{
    CommandName, DocsUrl, GlobPattern, ModuleName, NameError, PackageId, VersionFloor,
};
pub use rel::{CheckRelPath, PathError};
