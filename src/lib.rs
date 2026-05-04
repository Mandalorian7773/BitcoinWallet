#![deny(clippy::unwrap_used)]
#![deny(clippy::panic)]

// WHY: keeping wallet logic in a library makes the CLI testable without
// duplicating protocol code in integration tests.
pub mod config;
pub mod transaction;
pub mod wallet;
