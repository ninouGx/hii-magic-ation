#![allow(
    clippy::missing_errors_doc,
    reason = "internal crate, every error type is a documented enum"
)]

pub mod catalog;
pub mod edhrec;
pub mod scryfall;
pub mod sync;
pub mod verify;
