// WHY: Deny at the crate level so every module inherits the lint without
// per-file boilerplate.  These are graduated from "dangerous" to "pedantic".

// Panicking helpers are never acceptable in library code.
#![deny(clippy::unwrap_used)]
#![deny(clippy::panic)]
// expect() is equally a hidden panic.
#![deny(clippy::expect_used)]
// Indexing can panic on out-of-bounds; prefer .get() + explicit error handling.
#![deny(clippy::indexing_slicing)]
// Integer overflow is UB in release mode for some operations; be explicit.
#![deny(clippy::arithmetic_side_effects)]
// Pedantic lints catch style issues; keep as warn so CI is not blocked on
// false positives while we still get actionable feedback.
#![warn(clippy::pedantic)]
// Module-name repetition (e.g. wallet::WalletError) is idiomatic Rust and
// not a real problem.
#![allow(clippy::module_name_repetitions)]

// WHY: keeping wallet logic in a library makes the CLI testable without
// duplicating protocol code in integration tests.
pub mod config;
pub mod transaction;
pub mod wallet;
