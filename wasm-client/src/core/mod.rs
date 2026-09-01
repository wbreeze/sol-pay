//! UI-agnostic client core: address derivation, instruction and transaction
//! construction, account decoding, amount conversion, preflight arithmetic and
//! error naming. Depends on nothing browser-specific, so it works from a
//! Leptos/Yew app, a native binary, or the wasm-bindgen wrapper in `lib.rs`.
//!
//! Everything that depends on *which* deployment of the metering program is
//! being addressed hangs off [`Program`]. The free functions in [`pda`],
//! [`ix`], [`tx`] and [`error`] are the same calls against the canonical
//! deployment.

pub mod error;
pub mod ids;
pub mod ix;
pub mod pda;
pub mod preflight;
pub mod program;
pub mod state;
pub mod tx;
pub mod units;

pub use program::Program;
