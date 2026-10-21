# Rules: StartMatch (implemented, replay-verified)

Source: decompiled 2.17.3 build (`~/src/polybot_re_out/decomp-2.17.3/start/`, plus Cpp2IL disassembly for `ActionUtils`).
Verified by `testdata/savestates/start_game_1.state` (version 123): replaying it reproduces the saved state exactly
(`engine/tests/engine.rs`). Code: `engine/src/engine.rs`.

## Command
`StartMatchCommand.IsValid` is always true. `Execute` records the command and queues `StartMatchAction(player)`.

## StartMatchAction (`ExecuteDefault`, used from version 99 on)
For each player, in `players` order (Nature included):
1. Look up the player's tribe in the rules data. If found, `LearnTech` each of the tribe's `startingTech`, in the listed order.
2. If `settings.rules_game_mode == 7`: queue `IncreaseCurrencyAction(+6 stars)`.
3. `ExploreLightHouseTask.TryGetFirstDiscoveredLighthouseTile(player)`: lighthouses sit on the four map **corners**; true if the player
   (never Nature, id 255) has explored any corner tile. If true the game calls `ActionUtils.OnRevealLighthouseTile`.
Then `state.current_state = 2` (Started).

## LearnTech(state, player, tech, cost, shouldUseActions)
- If the player already has the tech: nothing at all.
- Else: `available_tech.push(tech)`; `currency -= cost`; score amount = `ScoreSheet.GetTechScore(tech) = tech.cost * 100`, applied with
  `ActionUtils.AddScore` directly, or (when `shouldUseActions`) by queueing an `IncreaseScoreAction`; then `EnableTask` for each task the
  tech unlocks (`techData.taskUnlocks`); then, for techs that grant the `capitalvision` ability (only `diplomacy`), `TryRevealAllCapitals`;
  finally `CheckTask(Genius)`.
- At game start `cost = 0` and `shouldUseActions = false`.

## Score, stars and tasks
- `AddScore(player, amount)`: `player.score += amount`; then, if `rules.score_limit > 0`, the score reached it and the game is Started,
  `TryGetWinner` and possibly a `GameOverAction`.
- `IncreaseScoreAction.Execute` = `AddScore`. `IncreaseCurrencyAction.Execute`: `currency += amount`, then `CheckTask(Wealth)` if the player has it.
- `EnableTask`: nothing if the player has that task; else queue `EnableTaskAction`, whose `Execute` creates the task (started, not completed,
  counter 0 for counter-type tasks) and then runs `CheckTask` for it.
- `CheckTask(player, type)`: if the player has the task and it is not completed, ask `IsTaskCompleted`; **it sets the task's own completed
  flag** and, if true, the check queues `TaskCompletedAction`. Task conditions read so far: Pacifist (1): counter > 4; Genius (2): the
  player knows every tech of the tribe's tree (tree = techs reachable from `basic` through `techUnlocks`, with the tribe's tech
  replacements applied; true if the tribe is unknown); Wealth (4): stars > 99. Tasks come only from techs: trade -> Wealth,
  meditation -> Pacifist, philosophy/mysticism -> Genius, rituals -> Convert.

## Implemented status
| Behaviour | State | Evidence |
|---|---|---|
| Start-of-match techs, score, state flag, Genius task start | implemented | `start_game_1.state` replays exactly |
| `LearnTech` known-tech no-op, cost, score, IncreaseScoreAction path | implemented | unit tests only (`engine.rs`) |
| `CheckTask` Pacifist / Genius / Wealth, `IncreaseCurrency` + Wealth re-check | implemented | unit tests; "not complete" outcome also seen in the fixtures |
| Game mode 7: +6 stars | implemented | unit test; no real game with mode 7 |
| Capital vision (`diplomacy`) when the player knows nobody | implemented (no-op) | unit test |
| Score-limit game over | **stops with an error** when a score limit is set and reached | `TryGetWinner`/`GameOverAction` not read |
| Task becomes complete -> `TaskCompletedAction` | queued, **stops with an error** when executed | its effects not read |
| Capital vision when the player knows others -> `TryRevealCapital` | **stops with an error** | not read |
| A corner already explored at match start -> `OnRevealLighthouseTile` | **stops with an error** | not read |
| `CheckTask` for Network, Metropolis, Killer, Convert, Explorer, ExploreLightHouse | **stops with an error** | rules not read/decompiled |
| Skin colours in the load fix-up (below) | not handled | `GetTribeColor` skin branch not read |
Anything "unit tests only" is the engine matching my reading of the code, not yet the real game; a fixture for each would confirm it.

## Facts about comparing states (needed to make the replay match)
1. **Load-time colour fix-up.** `PlayerState.Deserialize` replaces colour -1 with the tribe colour. A new `PlayerState` starts at -1, so
   a never-reloaded live game has Nature at -1, while the saved *initial* state (a copy that went through the deserializer) has the
   tribe colour. `replay::apply_load_fixups` applies the same fix-up to the saved current state before comparing. (Skin colours not handled.)
2. **Relations are lazily created.** `PlayerDiplomacyExtensions.GetRelation(player, id)` inserts a default `DiplomacyRelation` when
   missing, and is called from pathfinding, AI opinion code, UI, etc. After `StartMatch` the saved state has default relations for
   players 1-5 that the initial state lacks, but nothing in the match logic creates them. `replay::canonicalize_for_comparison` drops
   default-valued relations from both sides. Non-default relations are compared.
3. The saved initial state already contains the map, fog of war, capitals and starting units: `StartMatch` changes none of them.
