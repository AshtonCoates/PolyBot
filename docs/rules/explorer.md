# Explorer, Border Growth and Super Unit city rewards

## Explorer (reward 4) - verified by `early_game_turn2`
Not a unit: a chain of `ScoutMoveAction`s (12 steps for game version >= 120, else 15) that reveal fog. The reward takes one unit id
(`GetNextUnitId`), seeds the chain with `RandomHash.GetHash(cityX, cityY)` and starts at the city.
Each step (`ScoutMoveAction.Execute`, version >= 115): path starts as `[from]`; pick the next tile with `GetNextTargetDefault`;
append it; if steps remain queue the next one (seed = `GetHash([seed])`); then `ExploreFromTile` around the tile (sight 2 on a mountain, else 1).
If nothing is reachable the chain stops.

`GetNextTargetDefault` (fog search, `engine/src/scout.rs`):
1. `PopulateLookup`: 11x11 window (radius 5, x fastest) of flags per tile: Valid 1, CanMoveOver/CanEndMoveOn 2|4 (city, or terrain
   unlocked by a known tech), Fog 8 (unexplored), Lighthouse 32, Mountain 64 (explored mountain).
2. `PopulateFogDistanceMap`: movable tiles start at 9999 (others 1000); fog tiles 0 and queued; explored mountains next to fog get a
   preset `100 + score` and become read-only; then a FIFO relaxation over the 8 neighbours with step 100. A fog tile's neighbours get
   `100 + GetScoreFromSurroundingFogTiles` instead (`-10 * min(fog tiles in the 3x3, 4) + 73`, or `+ 40` with a fogged lighthouse).
3. `ApplyBacktrackingPenalty`: every tile already on the explorer's path adds a penalty (1, 2, 3, ... by position; from 9 entries on,
   it grows every second entry).
4. The best of the 8 neighbours (order: (-1,-1) (-1,0) (-1,1) (0,-1) (0,1) (1,-1) (1,0) (1,1)); ties are broken with
   `HashFunction.Range(0, n, seed)` = `XXH32([seed], gameSeed) % n`. Nothing below 9999 -> no target.
`XXHash` is plain XXH32 over little-endian 32-bit words seeded with the game seed. Bridges in the window are not implemented.

## Border Growth (reward 5) - not yet replay-verified
`AddBorderGrowthActions`: city border size + 1, then sub-actions `ExpandCity`, `UpdateRoutes`, `UpdateCityConnections(other players)`.
`ExpandCityAction`: every tile of the city's area (square of radius = border size, tiles with no ruling city or ruled by this city)
not owned by the player is claimed (owner and ruling city set, `IncreaseScore(20)` each), then every unexplored tile of the area gets an
`Explore` action. Not implemented: claiming tiles that hold an improvement, the Spread tribe ability, Polaris freeze.

## Super Unit (reward 6) - not yet replay-verified
A free Giant (the tribe's override of `giant`) is trained on the city tile (`TrainAction`, cost 0). Occupied city tile (the game pushes the
unit away or kills it) is not implemented.

## Other fixes in this step
- A unit created by `UnitState.Create` in a game older than 123 has no stored birth-climate skin: the comparison ignores that field for such saves.
