# Replay testing

A saved game holds its initial state, the full command history and the final state (see
[game-findings.md](game-findings.md#savestate-format)). The engine is correct for a save if it can start from the
initial state, apply the commands, and end up in the saved current state.

## Pieces (all in `engine/`)
| Module | Role |
|---|---|
| `state.rs` | Plain data types for a game: `GameState`, tiles, units, players, `Command`, `LocalGameData` |
| `savestate/` | `load_state_file(bytes)` (LZ4 pickle -> payload -> `LocalGameData`); `write_local_game_data` is its exact inverse (for tests) |
| `diff.rs` | `diff_game_states(expected, actual)` -> list of `Diff { category, path, expected, actual }`; `summarize` counts by category |
| `replay.rs` | `Engine` (state + rules; `step`/`execute`) and `replay(save, rules)` -> `ReplayOutcome` |
| `src/bin/replay.rs` | CLI: summary of a save, replay result, diffs |
| `rules/` | Static rule tables; `Rules::bundled()` loads `re/data/GameLogicData28.json` |

## Using the `replay` CLI
`cargo run -p polytopia-engine --bin replay -- --help` lists everything. In short:
```
replay [OPTIONS] [FILES]...        # no FILES = every *.state in testdata/savestates (run from the repo root)

  --info             only print a summary of each save (no replay)
  --diff-initial     print how the saved initial and current states differ, instead of replaying
  --limit N          max differences listed (default 20);   --all  list every difference
  --rules PATH       use rule tables from another JSON file (default: the bundled GameLogicData28)
  -q, --quiet        skip the save summary when replaying
  -h, --help / -V, --version
```
Exit status: 0 = every file loaded and (unless `--info`/`--diff-initial`) replayed to exactly the saved state; 1 = a file
failed to load or its replay did not match; 2 = usage or I/O error. With several files it ends with a tally.
Unknown options are rejected (with a "did you mean" hint). The CLI lives behind the `cli` cargo feature (on by default), so a
library-only build (`--no-default-features`, e.g. for the Python bindings) does not need `clap`.

`cargo test -p polytopia-engine` also runs the CLI itself against the fixtures (`tests/cli.rs`).

## Test savestates
Fixtures live in `testdata/savestates/` (committed; see its README). Add one right after saving the game to the main menu:
```
uv run import-savestate full_game_1                  # newest save in the Steam directory, copied as full_game_1.state
uv run import-savestate --list                       # list the saves (id, size, modified time)
uv run import-savestate combat_1 --game <id|index>   # a specific save;  --force overwrites an existing fixture
```
The tool copies the file byte-for-byte, verifies the copy, parses it with the Python parser and prints a summary (version,
map, players, command counts). A save it cannot decode yet is still kept, with a warning. It also warns about real account ids.
`.state` files are the game's raw save files (`~/.config/unity3d/Midjiwan/Polytopia/<profile>/Data/Singleplayer/*.state`).
The capture tool's old `.pkl` files wrap the same bytes in a `raw` field.

Tests: `cargo test -p polytopia-engine` runs `every_fixture_loads_and_round_trips_exactly` over all fixtures, so a new fixture that
uses a command/action we cannot decode fails there with the reason (that is the to-do list). `python3 tests/test_savestate.py` and
`python3 tests/test_import_state.py` cover the Python side.

## Where things stand
All fixtures are listed in `testdata/savestates/README.md`. Replay status (`replay -q`, from the repo root): `start_game_1` (StartMatch) and
`train_1` (StartMatch, Move, Train) **match exactly**. `attack_1`, `build_mine_upgrade` and `capture_1` run 3 commands and stop at `EndTurn`;
`metropolis` stops at `Research` (#2); `pass_turn_1` at `EndTurn` (#1); `early_game_turn2` at `Build` (#1).

Implemented so far: the engine loop (commands queue actions, actions run last-in-first-out; every command queues `RunDeferred` first), `StartMatch`,
`LearnTech`, `EnableTask`, `Move` (single step), `Train`, exploration. Everything else returns `Unimplemented`. With `validate_commands` (on in the replay)
each recorded command must also pass our `IsValid`. See [rules/start-match.md](rules/start-match.md). `--diff-initial` on a fixture lists what its
commands must change.

Comparison details (so a "match" means the real game state): the saved state gets the game's load-time colour fix-up, and default-valued
diplomacy relations are ignored on both sides (the game creates them as a side effect of looking at them). Both are in `replay.rs`.

## Workflow for adding a command
1. Read the command's `IsValid`/`Execute` and the actions it queues (docs/rules/).
2. Implement it in `engine.rs` (commands queue `Action`s from `actions.rs`), run the replay, look at the differences.
3. A failure only shows the final state, so more captures (short, one feature each) make bugs much easier to localise.
Open design points: the action stack is not modelled yet (needed for the last-in-first-out order); RNG use is untested;
all stats are the game's integers (health on a 10x scale).
