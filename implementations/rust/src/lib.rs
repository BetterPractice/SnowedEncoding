//! Snowed binary serialization format — Rust implementation.
//!
//! `#![no_std]` compatible; requires only `alloc`.
//!
//! # Overview
//!
//! The primary type is [`SnowedValue`], which can represent any value in the
//! Snowed type system. Use [`encode`] to serialize and [`decode`] to
//! deserialize.
//!
//! # `rust_decimal` feature
//!
//! Enable the `rust_decimal` feature to get `From<rust_decimal::Decimal>` and
//! `TryFrom<SnowedValue>` conversions for `rust_decimal::Decimal`.

#![no_std]

extern crate alloc;

mod decode;
mod encode;
mod value;

pub use decode::{decode, DecodeError};
pub use encode::encode;
pub use value::SnowedValue;

#[cfg(feature = "serde")]
pub mod serde;
