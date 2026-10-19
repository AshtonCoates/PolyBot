//! Polytopia simulation engine.
//!
//! Will eventually hold a from-scratch reimplementation of Polytopia's
//! rules, driven by the state format reverse-engineered under
//! `src/polybot/parsing`, and exposed to Python (for the RL training loop)
//! via PyO3 + maturin.
//!
//! So far only the static rule tables ([`rules`]) exist. The dynamic game
//! state and commands come after `GameState` has been decompiled.

pub mod rules;
