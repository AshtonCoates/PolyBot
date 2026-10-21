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

## Done (savestate)
- Decoded LZ4 pickle container and LocalGameData/GameState header; `src/polybot/parsing/lz4pickle.py`.

## Done (savestate, part 2)
- Decoded GameSettings, MapData, TileData, Resource/Improvement/UnitState; `src/polybot/parsing/savestate.py` parses both
  game states' settings and maps from the sample and cross-checks with the rules data.

## Done (savestate, part 3)
- Whole sample file parses exactly (`parse_savestate`, `tests/test_savestate.py`), including players and the 41-command history.

## Done (rules, part 1)
- Execution model (commands append actions; actions run LIFO) and first-pass Move/EndTurn rules written up in `docs/rules/move-and-endturn.md`.
  Not yet validated by a replay test.

## Done (replay infrastructure)
- Rust `state`, `savestate` loader + exact writer, `diff`, `replay` runner and `replay` CLI (see `docs/replay-testing.md`).
  Sample parses and re-serializes byte-for-byte. Engine commands are still all `Unimplemented`; replay stops at `StartMatch`.
- `replay` CLI now uses clap (`--help`, `--info`, `--diff-initial`, `--limit/--all`, `--rules`, `--quiet`, multiple files, proper exit codes; tests in `engine/tests/cli.rs`).
- Fixtures in `testdata/savestates/` (committed) and the `import-savestate` CLI to copy saves in from the Steam directory.

## Done (game update 2.17.2 -> 2.17.3)
- Steam updated the game mid-session (version 122 -> 123). Rebuilt artifacts for 2.17.3 and compared: rules data identical, savestate
  format unchanged, only the diplomacy `MeetAction` reward changed. Details in `docs/game-updates.md`. New saves are version 123.

## Done (first engine milestone)
- Engine loop + `StartMatch`/`LearnTech`/`EnableTask` implemented; `start_game_1.state` (v123) replays to the exact saved state.
  `early_game_turn2.state` now stops at command #1 (`Build`). 34 Rust tests. Details: `docs/rules/start-match.md`.

## Done (start-of-match TODOs)
- Filled in `LearnTech` tail, `CheckTask` (Pacifist/Genius/Wealth), `IncreaseScore/IncreaseCurrency`, lighthouse and score-limit guards. Branches whose
  effects were not read stop with an explicit error. 45 Rust tests. Table of what is verified vs not: `docs/rules/start-match.md`.

## Done (8 fixtures, all command types decoded)
- Added Capture, ExamineRuins and every other command layout (34 types); 8 fixtures load and round-trip; 47 Rust tests. The `metropolis` save shows the Metropolis task completing
  (city level 5, monument 28 built) and a Killer task starting; useful for the task-completion effects still marked as errors.

## Done (Move + Train)
- `Move` (single step), `Train`, exploration/fog of war, scoring, `CommandBase.Execute` -> `RunDeferred`, deferred route/connection handling. `train_1.state` replays
  exactly; the other fixtures now get 1-3 commands further. 59 Rust tests. Details: `docs/rules/move-and-train.md`. Next blockers by fixture: EndTurn (3 fixtures),
  Research, Build, then the real pathfinder for multi-step moves and MeetAction.

## Done (EndTurn / StartTurn)
- `EndTurn` (command + action), `EndPlayerTurn`, `IsAlive`, `HealAction`, `StartTurn` income, task/meeting checks, with explicit stops for unread rules.
  71 Rust tests. Not replay-verified yet (needs Build, CityReward, 2-step Move; Research now done in the same fixtures). Details: `docs/rules/end-turn.md`.
- New fixture `full_game_1.state` (1v1, 20 turns, 370 commands, ends with player 2 eliminated).

## Done (Research, Build, CityReward)
- `Research` (price formula from the binary), `Build` for the improvements whose side effects were read (farm, mine, hunting, fishing,
  road, lumber hut, monuments, ...), `CityReward` (Workshop, Park, Resources, Population, CityWall), city population / level-up /
  scoring. 56 engine unit tests. Details: `docs/rules/build-and-city-reward.md`. Not replay-verified: every fixture still stops at
  a multi-step Move (pathfinder) or a reward still unimplemented (Explorer, BorderGrowth, SuperUnit).

## Done (pathfinder)
- A* pathfinder (`engine/src/pathfinder.rs`): exact tie-breaking and costs for walking units. **`pass_turn_1.state` replays exactly**
  (first full multi-player game verified end to end). Also fixed: population adds to both xp and population, trained units face the
  map centre. Details: `docs/rules/pathfinder.md`. 58 engine unit tests + 6+10+7 integration tests.
- Fixtures now stop at: attack_1 / early_game_turn2 / full_game_1 / metropolis -> CityReward Explorer (4), BorderGrowth (5), SuperUnit (6);
  build_mine_upgrade (#50) -> MeetAction; capture_1 (#52) -> UpdateRoutes with route openers.

## Done (city rewards: Explorer, Border Growth, Super Unit)
- Explorer (XXHash, FastPath fog search, ScoutMoveAction): **`early_game_turn2.state` replays exactly** (41 commands). Border Growth
  (ExpandCity) and Super Unit are implemented but not yet replay-verified. Details: `docs/rules/explorer.md`.
- Now all of `start_game_1`, `train_1`, `pass_turn_1`, `early_game_turn2` replay exactly. The other five stop at `MeetAction`
  (attack_1 #12, build_mine_upgrade #50, full_game_1 #8, metropolis #39) or `UpdateRoutes` with route openers (capture_1 #52).

## Done (MeetAction)
- First contact (ExploreAction / CheckTribeConnection -> MeetAction, away stars, relation `firstMeet`), the lighthouse-corner reveal and
  the Explore-lighthouses task. Details: `docs/rules/meet.md`. Fixture progress: attack_1 #49 and build_mine_upgrade #61 now stop at
  `Attack`; metropolis #71 at `Capture`; capture_1 #52 and full_game_1 #18 at `UpdateRoutes` with route openers.

## Done (route building)
- `UpdateRoutes` for route openers (ports, atolls, ...), via a route-builder mode of the pathfinder. capture_1 now reaches the last-but-one
  command (#66, `Capture`). full_game_1 (#25) stops at the first unit that could board a boat through a port (EmbarkAction not implemented).

## Done (Capture, Attack, routes, city names, city connections)
- `capture_1`, `attack_1`, `build_mine_upgrade` replay exactly (together with start_game_1, train_1, pass_turn_1, early_game_turn2: 7 of 9).
  Details: `docs/rules/capture-and-attack.md`. Left: `metropolis` (ExamineRuins at #75) and `full_game_1` (boats at #25).

## Engine milestone: all 9 fixtures replay exactly
`cargo run -q --bin replay` reports every fixture in `testdata/savestates` as an exact match (62 unit + integration tests, clippy clean).
Added in the last stretch: ExamineRuins (reward shuffle `HashFunction.Range` over `[1,2,8,6,3,4]`; rewards: 1 stars, 2 +3 population in the
capital, 3 explorer, 4 free tech, 6/8 promote + train a unit on the tile (Rammership / Swordsman), validity per reward), Stay, Recover, Promote,
Upgrade, Embark / Disembark (boats via ports; a boat's max health and score use its passenger), boat pathfinding, Stomp / AreaDamage,
adjacency-based improvement levels (`UpdateImprovementLevel`, level up / down), ChangeCityConnection, TryGetWinner / GameOver /
WipePlayer / Disband, TrainUnitOnOccupiedSpace + TryPushUnit, capital vision reveals.
Corrections found by the replays: meet reward uses `ceil` (not round); Metropolis task is enabled at the first `CityReward`, not at level-up;
`ExploreAction` is invalid for an already-explored tile (no double score); a city reward consumes the trigger of its own city; the unit
direction of a unit trained on the map centre is 0; free-tech list excludes aquatism (12) for the cases seen (reason unread).
Comparison ignores the order of a player's relations dictionary.

## Next
0. Not covered by any fixture yet: bubbles, poison, followers, Persist/Splash/Freeze/Convert/Infiltrate attacks, Spread/Flooded tribes,
   treaties/embassies, resign, Disband command, ruins rewards 5/7/9, game modes 2/4 ranking, score/turn-limit endings, PyO3 bindings.
1. (done) Address alignment verified.
2. Decompile `GameState.Deserialize` -> savestate format and which rules version a save uses.
3. Decompile combat and command logic.
4. (done) Rust data model + loader for `GameLogicData28.json`; next after Ghidra: dynamic state types.
5. (partly done) `VersionManager` gives GAME_VERSION 122 and GAME_LOGIC_DATA_VERSION 28; see game-findings.md.
6. Savestate: decode the remaining ~22 command types and the action records (decompiled in `decomp/cmd/`); more captures
   (different actions: attack, capture, build road, trade, diplomacy) would verify them.
7. NEXT ENGINE STEP: Attack and Capture (then Embark / boats), adjacency-based improvement levels, Attack, Capture;
   capture more small v123 saves (one feature each) to verify each.
8. Use the command list as the engine's action space: each command = (type, player, args) with `IsValid`/`Execute` still to decompile.

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
