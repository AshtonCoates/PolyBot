# Status (2026-10-19)

## Done
- Plan agreed: Rust engine + PyO3 bindings + Python RL; narrow first slice.
- Cpp2IL run; declarations dumped; 112,964 method addresses tabulated.
- Rule tables extracted as JSON (28 files).
- `re/` scripts and latest rules copy added to the repo (uncommitted).
- Rust rules data model + loader drafted in `engine/src/rules/` (`flags.rs`, `ids.rs`, `raw.rs`, `mod.rs`,
  `tests.rs`); loads `re/data/GameLogicData28.json`; 7 unit tests pass. Models static rules only; dynamic
  state/commands wait for the `GameState` decompilation.

## In progress
- Headless Ghidra analysis + naming job (started ~14:50). Check `~/src/polybot_re_out/ghidra_headless.log`
  for completion and the final `methods= named= ...` line.

## Next
1. Verify address alignment; fix if names did not apply.
2. Decompile `GameState.Deserialize` -> savestate format and which rules version a save uses.
3. Decompile combat and command logic.
4. (done) Rust data model + loader for `GameLogicData28.json`; next after Ghidra: dynamic state types.
5. Read the real version numbers of the rule files.

## Decisions
- Decompile first; savestates are an oracle for engine tests, parsed after step 2.
- Third-party tools live in `~/src/tools/`; dumps in `~/src/polybot_re_out/`; nothing large in git.

## Lessons learned
- REA/Ghidra route: a timed-out query kills Ghidra's temporary database, so the next query re-imports
  and re-analyses. Names applied through REA are also lost on close. Hence the standalone headless
  project.
- `rm -rf *` was nearly run in a scratch directory; avoid wildcard deletes, use fresh directories.

## Risks
- Machine has 15 GB RAM; Ghidra analysis is memory-heavy.
- `pyproject.toml` requires Python 3.14; torch/maturin wheels may lag.
- Randomness (map generation) may not match the real game exactly.
