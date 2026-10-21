//! Polytopia simulation engine.
//!
//! Will eventually hold a from-scratch reimplementation of Polytopia's
//! rules, driven by the state format reverse-engineered under
//! `src/polybot/parsing`, and exposed to Python (for the RL training loop)
//! via PyO3 + maturin.
//!
//! Static rule tables: [`rules`]. Dynamic state: [`state`], loaded from `.state` files by [`savestate`].
//! [`diff`] compares two states; [`replay`] re-runs a saved game's commands and checks the result.

pub mod actions;
pub mod diff;
pub mod engine;
pub mod names;
pub mod pathfinder;
pub mod replay;
pub mod rules;
pub mod savestate;
pub mod scout;
pub mod state;
