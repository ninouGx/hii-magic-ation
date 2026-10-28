#![allow(
    clippy::missing_errors_doc,
    reason = "internal crate, every error type is a documented enum"
)]

pub mod db;
pub mod filter;
pub use duckdb;
