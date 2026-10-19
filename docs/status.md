# Status (2026-10-19)

## Done
- Plan agreed: Rust engine + PyO3 bindings + Python RL; narrow first slice.
- Cpp2IL run; declarations dumped; 112,964 method addresses tabulated.
- Rule tables extracted as JSON (28 files).
- `re/` scripts and latest rules copy added to the repo (uncommitted).
- Rust rules data model + loader drafted in `engine/src/rules/` (`flags.rs`, `ids.rs`, `raw.rs`, `mod.rs`,
  `tests.rs`); loads `re/data/GameLogicData28.json`; 7 unit tests pass. Models static rules only; dynamic
  state/commands wait for the `GameState` decompilation.

## Done (continued)
- Ghidra project built (lean approach) and decompilation verified: `AttackCommand.IsValid/ExecuteDefault` and
  `BattleHelpers.*` decompiled to `~/src/polybot_re_out/decomp/`. Output is raw but readable and uses real names.
- Early look at combat (`BattleHelpers.GetBattleResults`): damage is computed with integer math using the constant
  4500 (0x1194), rounded to the nearest 10. Not yet worked out in full; do not rely on this yet.

## Next
1. (done) Address alignment verified.
2. Decompile `GameState.Deserialize` -> savestate format and which rules version a save uses.
3. Decompile combat and command logic.
4. (done) Rust data model + loader for `GameLogicData28.json`; next after Ghidra: dynamic state types.
5. (partly done) `VersionManager` gives GAME_VERSION 122 and GAME_LOGIC_DATA_VERSION 28; see game-findings.md.
6. Savestate: first findings recorded in game-findings.md; need more captures (one action each) for black-box checks.

## Decisions
- Decompile first; savestates are an oracle for engine tests, parsed after step 2.
- Third-party tools live in `~/src/tools/`; dumps in `~/src/polybot_re_out/`; nothing large in git.

## Lessons learned
- Full Ghidra auto-analysis on a 107 MB IL2CPP binary does not finish in reasonable time and gives no progress; a
  no-analysis import plus naming from Cpp2IL addresses takes minutes and is enough for decompilation.
- REA/Ghidra route: a timed-out query kills Ghidra's temporary database, so the next query re-imports
  and re-analyses. Names applied through REA are also lost on close. Hence the standalone headless
  project.
- `rm -rf *` was nearly run in a scratch directory; avoid wildcard deletes, use fresh directories.

## Risks
- Machine has 15 GB RAM; Ghidra analysis is memory-heavy.
- `pyproject.toml` requires Python 3.14; torch/maturin wheels may lag.
- Randomness (map generation) may not match the real game exactly.
