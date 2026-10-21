# Test savestates

Real Polytopia `.state` files used as test fixtures (loader round-trip, replay tests). They are committed; keep them small and
give each a descriptive name, e.g. `full_game_1.state`, `combat_warrior_vs_archer.state`.

Add one with the importer, right after saving the game to the main menu:

    uv run import-savestate full_game_1                 # newest save in the Steam directory
    uv run import-savestate combat_1 --list             # show the saves first
    uv run import-savestate combat_1 --game <id|index>  # pick a specific save

Every `*.state` here is parsed and re-serialized byte-for-byte by `cargo test -p polytopia-engine`
(a fixture that uses a command or action we have not decoded yet fails that test; that is the to-do list).

Privacy: files contain the in-game display name. Online games can also contain real account ids; the importer warns when it sees one.

| File | Version | Contents |
|---|---|---|
| `start_game_1.state` | 123 | Just `StartMatch`. Replays exactly. |
| `train_1.state` | 123 | `StartMatch`, one `Move`, one `Train` (smallest test of both) |
| `pass_turn_1.state` | 123 | 19 commands, turn 1: Build, CityReward, EndTurn x6, Move, Research, Train |
| `early_game_turn2.state` | 122 | 41 commands, turn 2, 5 tribes + Nature |
| `build_mine_upgrade.state` | 123 | 75 commands, turn 3: includes one Attack, builds |
| `capture_1.state` | 123 | 67 commands: includes a Capture |
| `attack_1.state` | 123 | 99 commands: 4 Attacks, 2 Captures |
| `metropolis.state` | 123 | 430 commands, turn 11: the user's Metropolis task completed (city level 5, monument built), Killer task started, ExamineRuins, Upgrade, Stay, Recover |
| `full_game_1.state` | 123 | 1v1 on an 11x11 map, 370 commands, turn 20: player 2 eliminated (final turn). Simplest complete game |
