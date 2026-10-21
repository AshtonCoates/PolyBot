# Rules: execution model, Move, EndTurn

> **Superseded in part by [move-and-train.md](move-and-train.md)** (replay-verified). Corrections: `CommandBase.Execute` queues a `RunDeferredAction`;
> exploration radius is the sight range (2 on a mountain); a newly explored tile scores 5. The EndTurn section below is still unverified.

Source: decompiled `GameAssembly.so` (Ghidra output under `~/src/polybot_re_out/decomp/{move_end,turn}/`). Function names are
`Type__Method`; addresses are Ghidra addresses minus the 0x100000 image base shown in the decompile headers.
**Status: read from code, NOT yet validated by a replay test** (the plan is: load the sample's initial state, apply its 41
commands in the Rust engine, compare with the saved current state). Anything marked (?) is a guess.

## 1. Execution model (`ActionManager__Update` @ 02c94560-ish)
Game state holds three lists: `CommandStack` (history, +0x50), `ActionStack` (+0x58) and an index `CurrentCommand` (+0x48).
Each update step:
1. If the ActionStack is empty (or a pending command trigger exists for the current player):
   take `CommandStack[CurrentCommand]`, increment `CurrentCommand`, call `command.Execute(state)`.
   Commands do not change the world themselves (mostly): they **append actions** to the ActionStack.
2. Otherwise take the **last** action in ActionStack (LIFO), call `action.IsValid(state)`;
   if valid call `action.Execute(state)`; in both cases remove it (by the index read *before* executing, so actions it appended
   stay above and run first).
So after a command, the actions it appended run most-recently-appended first. (?) Check against a replay: e.g. `MoveCommand`
appends Reselect, Move, Hide in that order.
Actions can also have sub-actions (`ActionBase.CommitSubActionsToStack`), not read yet.
Legality of a command is `command.IsValid(state, out error)`; every `IsValid` starts with `CommandBase.PassesBasicValidation`.

## 2. MoveCommand (`MoveCommand__IsValid`, `MoveCommand__Execute`)
Fields: playerId, `UnitId` (u32), `From`, `To` (coordinates).
IsValid (all must hold):
1. `PassesBasicValidation(state)` (not read yet; presumably "it is this player's turn", player alive, game started).
2. Player exists. Tile at `From` exists and holds a unit whose id == `UnitId`.
3. `UnitState.CanMove()` (not read yet).
4. The unit's data is in the rules data.
5. `PathFinder.GetPath(state, From, To, UnitDataExtensions.GetMovement(unit), unit)` returns a path with **more than one** tile.
   (So moving onto your own tile, or beyond movement range, is illegal. Path finding itself not yet read.)
Execute:
1. `CommandBase.Execute` (records the command).
2. Find the unit by id; set `unit.moved = true`; append `ReselectAction(player, unitId)`.
3. If the destination tile has **no unit**: compute the path again; if it exists and has > 1 tile append
   `MoveAction(player, unitId, path, ...)`. Then, if the unit has ability Hide(5) and no Invisible(3) effect, append
   `HideAction(player, From)`.
4. If the destination tile **has a unit** (e.g. hidden enemy): `unit.moved = false`, append `RevealAction(player, To, unit coords, true)`.

## 3. MoveAction (`MoveAction__ExecuteDefault` @ 02cc18f0; dispatches `ExecuteV59/V83/V98/V101` for old versions)
The path is stored **reversed**: `path[0]` is the destination, `path[last]` the start.
Ordinary land unit, no special abilities:
1. `moved` is set true (unless the move reason is 2, or reason is 1 and the unit has Escape(2) — then it keeps its old value).
   If the unit has Skate(21) and the destination terrain is not ice(6) and reason != 2: `moved = attacked = true`.
2. `startTile.unit = null`; `destTile.unit = unit`; `unit.coordinates = path[0]`;
   `SetUnitDirection(unit, path[1], path[0])` (facing from the second-to-last step).
3. Hooks: `CheckStepOnPoison`; if the player has ability 13 then `CheckBubbleStep`; if the tile has effect 3 (tentacle zone),
   the tentacle owner's unit attacks (`BattleHelpers.GetBattleResults` + `AttackAction`); then ability hooks:
   AutoFreeze(20), Stomp(40)/Tentacles(44) -> `AreaDamage`, AutoHeal(41), AutoFlood(45), Algae(51) -> `BuildAction` type 55 on nearby water.
4. If the unit has the Bubble effect(4) and the destination is land (not wetland) and the unit is amphibious: remove bubble.
5. Tentacles(44): `UpdateTentacles`.
6. **Fog of war**: for every tile on the path, `ActionUtils.ExploreFromTile(state, player, tile, sightRange, true)`
   with `sightRange = UnitDataExtensions.GetSightRange(unitData)` (this adds the player to the tiles' `explorers` lists; not read yet).
7. If the unit's type is 13 and `TileData.CanDestroy` -> `DestroyImprovementAction`.
8. `ActionUtils.WillUnitEmbark` -> `EmbarkAction` (1) / `DisembarkAction` (2).
9. If the unit has a follower unit (`UnitState.HasFollower`): move/handle it (not read).
10. If the destination has improvement 1 (a city), is owned by another player (owner != 0 and != mover):
    `ModifyDiplomacyRelationsFromAttack`.
Note: the game's `UnitAbility` ids (Dash 1, Escape 2, Scout 3, Sneak 4, Hide 5, Persist 7, ..., Skate 21, ..., Tentacles 44, Algae 51,
DoubleAttack 54) are in `diffable-cs/.../Polytopia/Data/UnitAbility.cs`; the Rust `UnitAbility` enum uses different (alphabetical) numbers.

## 4. EndTurn
`EndTurnCommand.IsValid` / `Execute` (`EndTurnCommand__ExecuteDefault`):
1. `CommandBase.Execute`; append `EndTurnAction(player)`.
2. `UpdateTentacles`.
3. For each tile with a unit of this player (order = tile order): if `CanRecover` append `HealAction(...)`;
   Grow(12) units may become another unit by age; remove effects Frozen(0) and Petrified(5);
   DoubleAttack(54) -> add effect DoubleReady(7); improvement ability Heal(7) -> HealAction(+40 hp?) / HealOthersAction;
   improvements with effect 0 and ability 13 give currency (`IncreaseCurrencyAction`, amount 40?? (?)); consumed improvements ->
   `DestroyImprovementAction`.
`EndTurnAction.Execute` (`EndTurnAction__Execute`):
1. If the player has task 1 (Pacifist), notify it.
2. If `action.playerId == CurrentPlayer`: `GameState.EndPlayerTurn`:
   advance `CurrentPlayerIndex` to the next **alive** player (`PlayerExtensions.IsAlive`); if the index wrapped past the last
   player, `CurrentTurn += 1`.
3. `TryGetWinner(state)`; if there is a winner append `GameOverAction`.
4. Append `StartTurnAction(CurrentPlayer)`; if game type (settings +0x58) == 2 also `PassPlayerAction`.
5. Clear improvement effect 1 on all tiles owned by the ending player.
`StartTurnAction.ExecuteDefault`: embassy levels, task checks, diplomacy, `TryRevealAllCapitals`, per-city production
(`CalculateWork` -> `IncreaseCurrencyAction`, `CityLevelUpAction` when `ShouldLevelUp`), `CreateResourceAction` for improvements with
ability 10, `RestoreClimate`. Details not read.

## 5. Not yet read (needed before a replay test can pass)
`CommandBase.PassesBasicValidation`, `UnitState.CanMove`, `PathFinder.GetPath`, `UnitDataExtensions.GetMovement/GetSightRange`,
`ActionUtils.ExploreFromTile`, `ReselectAction`, `TileData.SetUnit`, `UnitDataExtensions.SetUnitDirection`, `StartTurnAction`
(income via `TileDataExtensions.CalculateWork`, level-up), `HealAction`, `PlayerExtensions.IsAlive`, `GameState.TryGetWinner`,
and the other four commands in the sample (Research, Train, Build, CityReward, StartMatch).
Ghidra timed out on: `ActionUtils__ExploreFromTile`, `UpdateTentacles`, `CalculateEmbassyLevel`, `RestoreClimate`,
`GameState__TryGetPlayer`, `get_CurrentPlayer`, `TryGetPendingCommandTrigger`, `CheckTask` (one overload). Re-run them with a longer limit.
