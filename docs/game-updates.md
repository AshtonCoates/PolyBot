# Game updates

Steam auto-updates Polytopia. Each update changes the binary, so address-based reverse-engineering artifacts go stale
(see [external-artifacts.md](external-artifacts.md)). Procedure for a new build:

1. `cat version.txt`; compare `sha256sum GameAssembly.so` with the one an artifact was built from.
2. Re-run Cpp2IL (`diffable-cs` and `isil`) into `-<version>` directories.
3. `python3 re/scripts/compare_isil.py <old isil>/IsilDump <new isil>/IsilDump GameLogicAssembly` lists the methods that really
   changed (hex addresses are normalized away). The rules live in `GameLogicAssembly`; `PolytopiaAssembly` is mostly UI.
4. Rebuild `method_addresses-<version>.tsv` and the Ghidra project, decompile only what changed.
5. Re-extract `GameLogicData*` and compare hashes with the old files.

## 2.17.2 -> 2.17.3 (Steam update on 2026-10-19 17:50; game/savestate version 122 -> 123)
Checked with the procedure above.
- **Rules data:** all 28 `GameLogicData*` files byte-identical. `GAME_LOGIC_DATA_VERSION` still 28. No stat/cost/tech changes.
- **Savestate format:** unchanged. `GameState`/settings/map/player serializers are identical. The only difference in the serializers is
  `UnitState.birthClimateSkinType`, written/read as `short` instead of `ushort` (same 2 bytes). The Rust/Python loaders accept version
  123 (all version gates are `>=` thresholds), but there is no v123 fixture yet to prove it.
- **Game logic (`GameLogicAssembly`):** 8 methods differ, 2 added; the only behavioural change found is in the diplomacy "meet" reward:
  `MeetAction.GetReward` changed, new `MeetAction.ShouldAddAwayStars(GameState, PlayerState)` and new class `GameTypeUtils`
  ("away stars", cf. the new version name `AWAY_STARS_CHAMPIONSHIP_FIXES = 123`). `WEEKLY_CHALLENGE_GAME_VERSION` 119 -> 121.
  Other differences are compiler reordering (`UnitState.Create`) or version-constant getters.
  Details (decompiled from the 2.17.3 build; `decomp-2.17.3/meet/`): when a player first meets another (`MeetAction`, reward in stars,
  game version > 82): `base = min(round(otherPlayer.score / 1000.0), 5)`, `reward = 2 * base + 1`. New in version >= 123 (0x7b): if the
  game has exactly 2 players and the met player's id is 1, `reward = 2 * base + 3` ("away stars": the second player gets +2 when meeting
  player 1). `ShouldAddAwayStars(state, otherPlayer) = state.version >= 123 && playerCount == 2 && otherPlayer.id == 1`.
  Rounding mode of `round` not checked. `GameTypeUtils.IsOnline(gameType)` is true for game types 1, 3 and 4.
- **`PolytopiaAssembly`:** 533 methods differ, 44 added, 60 removed: UI/rendering/networking; not examined, not relevant to the rules.
- Existing fixture `early_game_turn2.state` is version 122 and stays valid. New saves are written as version 123.
- Lesson: an earlier note blamed this on a "vtable shift"; that was wrong (it is the short/ushort change above).
