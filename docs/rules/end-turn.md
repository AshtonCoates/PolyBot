# Rules: EndTurn and StartTurn (implemented from the code; NOT yet replay-verified)

Sources: `EndTurnCommand.ExecuteDefault`, `EndTurnAction.Execute`, `StartTurnAction.ExecuteDefault`, `GameState.EndPlayerTurn`,
`PlayerExtensions.IsAlive`, `HealAction`, `TileDataExtensions.CalculateWork` (decompile of 2.17.2 + 2.17.3 disassembly; these did not change).
Code: `engine/src/engine.rs`. Unit tests exist for each rule below, but **no fixture exercises EndTurn in isolation**: every save with an
EndTurn also contains the AIs' moves, builds and research, so the end-to-end check needs those commands too (see "Verification").

## EndTurnCommand (the acting player's `EndTurn`)
`CommandBase.Execute` queues `RunDeferred`; then `EndTurnAction(player)` is queued; then, for every tile in map order:
- If the tile's unit belongs to the player: if `CanRecover` queue `HealAction(player, tile, 0)`; units with the Grow ability stop the engine
  (not implemented); then the unit gets a fresh turn: `moved = attacked = false`, effects Frozen (0) and Petrified (5) are removed,
  and a unit with DoubleAttack gets effect DoubleReady (7).
- If the tile belongs to the player and holds an improvement: improvements with the Heal ability (`GetHealOptions`) and improvement effect 0
  (income) stop the engine (not implemented).
- Afterwards `UpdateTentacles` runs; with a tentacle unit on the map the engine stops (not implemented).
`CanRecover(unit)` = no leader, (health < max health or Poisoned), not moved, not frozen and not attacked. It is checked **before** the flags
are cleared, so a unit that moved this turn does not heal. (`CanAttack` was not read; "not attacked and not frozen" is assumed.)
`max health` = unit type health + 50 per promotion level (passenger / Protect-ability case not read).

## HealAction
If the unit is Poisoned: remove Poisoned (1) and Charmed (8) and nothing else. Otherwise heal `fixed` if non-zero, else 40 on the player's own
tile (20 elsewhere; the game also gives 40 on a tile owned by a player they have peace with, not read), health capped at max health.

## EndTurnAction
1. The Pacifist task (1), if the player has it and it is not completed, gets `counter += 1` (`PacifistTask.Bump`).
2. If the action's player is the current player: `EndPlayerTurn` = advance `current_player_index` to the next **living** player; if the index wrapped
   past the last player, `current_turn += 1`. (`IsAlive`: Nature (255) always; death rule 0 "cities": owns a city; rule 1 "units": owns a unit.)
3. `TryGetWinner`: if there is a winner, queue `GameOverAction`. **Not implemented**: the engine stops when at most one non-Nature player is alive
   or a turn/score limit is set.
4. Queue `StartTurnAction(new current player)`. (Game type 2, hotseat, also queues `PassPlayerAction`: stops the engine.)
5. Clear improvement effect 1 on every tile the ending player owns.

## StartTurnAction (player whose turn begins)
1. `CalculateEmbassyLevel` for every other player: sets the embassy level only when an active embassy exists; the engine stops if the player has any
   non-default relation.
2. For each tile in map order owned by the player: if it has an improvement and `current_turn != 0`: queue `IncreaseCurrencyAction(income)` when the
   income is positive, where for a **city** `income = max(production + level - 1 [+ handicap if it is the capital] + min(xp, 0), 0)`, and 0 if an enemy unit
   stands on it or it has improvement effect 1; non-city improvements pay `work * level` (doubling not read; the engine stops for improvements that
   have `work`: market, sanctuary, icebank, clathrus). If `xp > level` (`ShouldLevelUp`) and it is the current player's tile, `CityLevelUpAction` follows
   (stops the engine). `UpdateImprovementLevel` uses the current level; the engine stops for lighthouses and for temple/limited/unique improvements, whose
   level is computed differently. `RestoreClimate` only acts on spreading climates (Polaris/Cymanti): the engine stops on such a tile.
3. `CheckTask(Pacifist)` and a second task check (id 7, unused) , then `CheckTribeConnection`: for each tile the player has explored, a unit or city of a
   player they have not met triggers a `MeetAction` (stops the engine). Nature (255) skips this.
4. Not read and so not implemented: `CheckDiplomacy` (peace and embassy upkeep), `TryRevealAllCapitals` at turn start.

## Verification
- Unit tests (`engine::tests`): turn order and wrap-around, dead players skipped, Nature immortal, fresh turn flags, heal rules, promotion health, Pacifist
  bump, effect clearing, city income formula, blocked income, turn-0 rule, and each stop condition.
- `pass_turn_1.state` (StartMatch, p1 EndTurn, then each AI plays one turn, then Nature's EndTurn): the replay gets through StartMatch, the first EndTurn
  and the first AI's one-step move, then needs `Build` (next), `CityReward`, a two-step `Move`, `Train` and `Research`. When those exist, this fixture
  verifies the whole turn cycle, including the turn counter, the income actions, and hand-over to Nature.
- `full_game_1.state` (1v1, 370 commands, 20 turns) needs most of the game; it ends with player 2 eliminated and the match in state "FinalTurn" (3).

## Research (added after EndTurn)

`ResearchCommand.Execute` queues `ResearchAction(player, tech, GetTechPrice)`; the action calls `ActionUtils.LearnTech`
(same path as the start-of-game techs, with score/tasks queued as actions).
`GetTechPrice = ceil((tier + 4) + (cities - 1) * tier)`, multiplied by 0.66666 first if the player has the Literacy
ability (float constants read from `GameAssembly.so` 2.17.3). `IsValid` here checks only "not known" and "affordable";
the game also requires the tech to be in `GetUnlockableTech` (parents learned), which is not yet checked.
Replays now get past `Research` in attack_1 (to #9) and metropolis (to #5); all now stop at `Build`.
