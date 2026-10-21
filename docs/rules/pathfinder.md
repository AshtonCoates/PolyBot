# Pathfinder (A*)

Sources: `PathFinder.GetPath/Step/ValidNeighbors/IsTileAccessible/IsAllowedToFinishOnTile/ReconstructPath`,
`PathFinderSettings.CreateForUnit`, `TileData.GetMovementCost` (game version > 114), `TileData.HasRoadTo`,
`MinHeap`, read from `objdump` of the 2.17.3 `GameAssembly.so` (decompiles of these time out). Code: `engine/src/pathfinder.rs`.

`MoveCommand.IsValid/Execute` call `GetPath(state, from, to, UnitDataExtensions.GetMovement(unit), unit)`; a path with more than
one tile is a move. The path is returned destination first, start last.

## Algorithm
- `start == dest` -> `[start]`. The destination must satisfy `IsAllowedToFinishOnTile` (no visible unit; not a city of someone we
  have peace with (relation 1) or an embassy-style relation (3)), else no path.
- Open set = `MinHeap`: a **sorted singly linked list**; a new node goes after every node with priority <= its own (FIFO on ties).
  Start priority = Manhattan distance to the target. `cameFrom` (default (0,0)) and `costSoFar` (default 0) per tile.
- Pop; if it is the target, reconstruct; else `Step`. At most 1000 iterations. Max cost = `movement * 10`.
- `Step(current)`: only expand if `costSoFar[current] < maxCost`. Neighbours = `ValidNeighbors` = `GetTileNeighbors`
  (the 8 surrounding tiles, **x outer, y inner**: x-1 (y-1,y,y+1), x (y-1,y+1), x+1 (y-1,y,y+1)) that pass `IsTileAccessible`.
  `new = GetMovementCost(neighbour, current) + costSoFar[current]`; if `costSoFar[n] <= 0 || new < costSoFar[n]` (so the start
  tile can be overwritten) set cost and `cameFrom` and push priority `euclidean(n, target) + new` (f32).

## IsTileAccessible (walking unit)
Explored by the player; a visible foreign unit blocks unless at peace or the mover has Hide; bridges, invisible foreign units and
water (any embarkation) are not implemented; a city is always accessible, otherwise the terrain must be in the player's allowed
terrain (the union of the `movementUnlocks` of the techs they know).

## GetMovementCost (steps are x10)
Zone of control: if any neighbour of the destination holds a visible, uncarried (`leader == 0`) unit of a player the mover is not at
peace with -> 1000. Ice with the Glide player ability -> 9. Road between both tiles (each has a road or is a city/bridge, owners
at peace or none) and not ice -> 5. Mountain or forest -> 1000 (the move ends there). Else 10.
Not implemented (stop with an error): flying, swimming, boats, skating, creeping, carrying and other special movers.

## Related findings (same session)
- `ImprovementState.AddPopulation(n)` is a packed 16-bit add on **both** `xp` (+0x1A) and `population` (+0x1C).
- A trained unit's direction is `GetDirectionFromDistanceVector(mapCentre - position)` (`UnitState.Create`), not a constant.
- `Escape` only matters for `MoveReason` 1 (a move after an attack); an ordinary Move command is unaffected.

## Verification
`pass_turn_1.state` (3 AI players, 6 turns, 19 commands) now replays to exactly the saved state, which exercises Research, Build,
CityReward, Train, multi-step Move, EndTurn and StartTurn together.

## Route building (`MapDataExtensions.UpdateRoutes`) - implemented, not replay-verified
`UpdateRoutesAction` (queued after building a route opener, border growth, or by `RunDeferred` after exploring) runs
`UpdateRoutes`: clear every tile's `hasRoute`; then for each player (not Nature) with their owned tiles in map order:
route openers = improvements whose data lists `routes` (port, atoll, outpost, mycelium, bridge); cities = city improvements.
For each opener `i`: `FindPathBetweenRouters(i, j)` for every later opener `j`, then, if opener `i` has the Network ability,
for every city. `FindPathBetweenRouters(origin, dest)`: skip if the Chebyshev distance exceeds the origin's `range`; the route
terrain is each terrain of the origin's `routes` that the destination also lists and the player may enter (any terrain when the
destination is a city); then `GetPath(origin, dest, maxCost = range, CreateRouterSettings)` and every tile of the path gets
`hasRoute` at once (so it is cheaper for later searches). Router settings = no unit, units ignored, foreign borders block (unless
at peace), cost 5 on roads / cities / bridges / existing routes else 10; water is only crossed from water or wetland.
Boarding a boat through a port (`EmbarkAction`, unit turns into a ship) is not implemented: the pathfinder stops with an error when a
friendly port is reachable by a player that can embark.
