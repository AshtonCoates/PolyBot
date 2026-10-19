# Overview

## Goal
Train an RL agent for The Battle of Polytopia. That needs many games running headless at max
speed, so the game rules are being re-implemented from scratch instead of driving the real game.

## Architecture
- **Rust engine** (`engine/`, crate `polytopia-engine`, `cdylib` + `rlib`): the deterministic game core.
- **Python bindings** (planned, PyO3 + maturin): batched environment, many games per `step` call.
- **Python RL** (`src/polybot/`): env wrapper, observation/action-mask encoding, training.
- **Reverse-engineering** (`re/`, plus dumps outside the repo): source of truth for the rules.

## Planned layout
```
polybot/
  Cargo.toml          workspace
  engine/             pure-Rust game core, golden tests, benchmarks
  bindings/           (planned) PyO3 crate exposing a batched env
  src/polybot/        capture/ (exists), parsing/, env/, rl/
  re/                 RE scripts and extracted rule data
  docs/               these notes
  notebooks/          marimo exploration
```
Design decisions: deterministic engine given a seed (one explicit RNG in state), flat cheap-to-clone
state, fixed integer action encoding plus legal-action mask, rule tables loaded as data files,
parallelism in Rust (rayon) with the GIL released.

## Phases
1. RE feasibility spike (in progress, see status.md)
2. Savestate format (follows from decompiling `GameState.Deserialize`)
3. Engine v1, narrow slice: one map size, 2 players, a few tribes and techs
4. Validation: load a captured state, apply the recorded action, compare with the next capture
5. Bindings and throughput benchmark
6. RL baseline (PPO + self-play)
7. Widen scope

## Rationale for sourcing rules from the binary
The game is IL2CPP, which keeps class, method and field names in `global-metadata.dat`, so
decompiling is more productive than diffing savestates. Savestates are kept as a test oracle.
