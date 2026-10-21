# Rules: Move, Train, exploration and deferred work (implemented, replay-verified on `train_1`)

Source: decompiled / disassembled 2.17.3 build. Verified by `testdata/savestates/train_1.state` (StartMatch, one Move onto a mountain,
one Train): the replay reproduces the saved state exactly (`engine/tests/engine.rs::train_1_replays_exactly`). Code: `engine/src/engine.rs`.
Supersedes the Move/execution-model details in [move-and-endturn.md](move-and-endturn.md) where they differ (see "Corrections").

## Every command: `CommandBase.Execute`
Queues `RunDeferredAction(player)` **first**. The action stack is last-in-first-out, so it sits at the bottom and runs after everything the
command queues. `RunDeferredAction`: if `state.HasFlaggedNeedForUpdateRoutes`, queue `UpdateCityConnectionsAction([player])` then
`UpdateRoutesAction` (routes run first); then always clear the flag. (Exploring sets the flag; saved states never have it set.)
- `UpdateRoutes`: reset every tile's `has_route` (and remember `hadRoute`), then rebuild routes between "route opener" improvements
  (an improvement whose data lists `routes`, e.g. a port). **Rebuilding is not implemented**: with an opener on the map the engine stops.
- `UpdateCityConnections(player)`: connects the player's cities to their capital. A player with one city has nothing to do; with several
  the engine stops.

## Move
`MoveCommand.IsValid`: `PassesBasicValidation` (it is this player's turn; no pending command trigger for them); the unit with that id stands
on `from`; `UnitState.CanMove()` = not `moved` and not Frozen (effect 0); a path of more than one tile exists
(`PathFinder.GetPath(state, from, to, unit movement, unit)`).
`MoveCommand.Execute`: sets `unit.moved = true`; queues `ReselectAction` (UI only); if the destination has no unit and a path exists, queues
`MoveAction(path)`; if the unit has the Hide ability and no Invisible effect (3) it also queues `HideAction`. (A destination that holds a unit
un-sets `moved` and queues a `RevealAction`.) The real game checks `IsValid` when a command is submitted, not when it is replayed; the engine
does so only when `validate_commands` is on (the replay turns it on).

`MoveAction.ExecuteDefault` (ordinary land unit): the path is stored reversed (`path[0]` = destination, last = start). Start tile `unit = None`;
destination tile `unit = Some(unit)` with `coordinates = destination`; `direction`/`flipped` from `SetUnitDirection(path[1], path[0])`
(vectors SW(-1,-1) W(-1,0) NW(-1,1) N(0,1) NE(1,1) E(1,0) SE(1,-1) S(0,-1); ids 0-7; flipped when id < 4; vectors longer than one tile pick the
best-aligned direction with weights 1393 axis / 985 diagonal); then `ExploreFromTile` for every tile on the path with the unit's sight range.
`moved` stays true and `attacked` is untouched (units with Dash keep their attack).

## Exploration (fog of war)
- `GetSightRange(unitData)` = 1, +1 with the Scout ability.
- `ExploreFromTile(state, player, tile, sightRange, useActions)`: radius = 2 if `tile` is a mountain, else `sightRange`. Every tile within
  Chebyshev distance `radius` that the player has not explored gets an `ExploreAction`, queued farthest first (so nearest runs first).
  This is why a warrior on a mountain revealed a 5x5 square.
- `ExploreAction.ExecuteDefault`: add the player to the tile's `explorers`; set `HasFlaggedNeedForUpdateRoutes`; queue
  `IncreaseScoreAction(ScoreSheet.exploreValue = 5)`; if the tile holds a unit, or a city, of another player the explorer does not yet
  know, queue a `MeetAction` (**not implemented**: the engine stops with an error).

## Train
`TrainCommand.Execute`: queue `TrainAction(player, unit type, tile, cost = unitData.cost)`. (`TrainCommand.IsValid` has **not** been read.)
`TrainAction.ExecuteDefault`: `player.currency -= cost`; `ActionUtils.TrainUnit`: `UnitState.Create` with
`id = ++state.current_unit_id` (pre-increment), health = unit data health, `created_turn = current turn`, direction 6 (SE), no effects, climate 0,
home = the city tile; the new unit is **moved and attacked = true**; then `ExploreFromTile` around the tile; a score action for the unit is
committed last (so it runs first): `ScoreSheet.GetUnitScore = 5 * UnitState.GetFullCost` (unit cost + passenger cost).
Not implemented (stop with an error): training onto an occupied tile, vehicles/tentacle units, water tiles, `ActionUtils.CheckSurroundingArea`
(re-evaluates nearby improvement levels; a no-op in the fixture).

## Pathfinding (partly implemented)
Only single-step paths: the destination must be adjacent, empty, and of a terrain some known tech lets the player enter
(`techData.movementUnlocks`, cheapest wins; basic tech: field 1, forest 2, ice 1, wetland 2; climbing: mountain 2). Entering a terrain that
costs more than the unit's movement is allowed when the unit starts the move (warrior movement 1 entered a mountain). Water terrains and any
path longer than one step stop with an error. The real search is A* (`PathFinder.GetPath`/`Step`): neighbours from `ValidNeighbors` filtered by
`IsTileAccessible`, step cost `TileData.GetMovementCost` (version-dependent), priority = cost so far + `WorldCoordinates.Distance` to the target.
Its tie-breaking and the accessibility rules (zones of control, enemy tiles, ports, ...) must be read before multi-step moves can be replayed.

## Implemented status
| Behaviour | State | Evidence |
|---|---|---|
| Command -> RunDeferred, Move (1 step, land), explore, Train, score, deferred route/connection handling | implemented | `train_1.state` replays exactly |
| `IsValid` for Move (turn, unit on tile, can move, 1-step path) | implemented | unit tests; accepted every recorded move reached so far |
| Move rules: multi-step paths, water, abilities (Escape, Skate, Fly, ...), tile effects, followers/passengers, onto a foreign city, onto an occupied tile | **stop with an error** | not read |
| MeetAction (first sight of another player) | **stops with an error** | needed by most longer games |
| Train: occupied tile, vehicles, `CheckSurroundingArea`, `IsValid` | stop / TODO | not read |
| UpdateRoutes rebuild, UpdateCityConnections for several cities | **stop with an error** | not read |

## Corrections to move-and-endturn.md
- `CommandBase.Execute` is not just bookkeeping: it queues `RunDeferredAction` (above). That also explains the action order.
- Exploration radius is the sight range (2 only on a mountain), not "sight range + 1".
- A newly explored tile is worth `exploreValue = 5` score, applied through an `IncreaseScoreAction`.
- Lesson: the simplified disassembly (`isil/`) can omit address arithmetic such as `lea eax,[rax+rax*4]` (x5); when a number is off,
  read the raw `Disassembly:` section.
