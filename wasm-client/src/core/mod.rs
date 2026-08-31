//! UI-agnostic client core: address derivation, instruction construction,
//! account decoding and amount conversion. Depends on nothing
//! browser-specific, so it works from a Leptos/Yew app, a native binary, or
//! the wasm-bindgen wrapper in `lib.rs`.

pub mod ids;
pub mod ix;
pub mod pda;
pub mod state;
pub mod units;
