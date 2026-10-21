# Build, CityReward and the actions behind them

Sources: `BuildCommand/BuildAction.ExecuteDefault` (game version > 105), `CityRewardCommand/CityRewardAction`,
`IncreasePopulationAction`, `CityLevelUpAction`, `ActionUtils.LevelUpCity`, `ScoreExtensions`, `ResourceDataUtils`.
Read from Ghidra decompiles (`~/src/polybot_re_out/decomp-2.17.3/build*`) and `objdump` of the 2.17.3 binary.

## Sub-actions
`ActionBase.AddSubAction` collects actions; `CommitSubActionsToStack` pushes them last-to-first, so the first one added
runs first. Code that writes straight to `state.actionStack` (CityReward, IncreasePopulation, LevelUpCity) pushes in
order, so the last pushed runs first. The engine mirrors both (`commit_sub_actions` vs `self.actions.push`).

## BuildCommand
`Execute` queues `BuildAction(player, kind, coords, deductCost = true)`. `IsValid` (41k lines of decompile) is not read.

## BuildAction.ExecuteDefault (implemented subset)
1. Unless the kind is Road, `Discrete` or `Consumed`: create the `ImprovementState` (level 0, xp 0, production 1,
   founded = current turn, baseScore = sum of the data's reward scores, borderSize from data, founder = player),
   then queue `IncreaseScore(CalculateImprovementScore)` if > 0.
2. `Consumed` kind on a tile with a resource (not Canal): sub-action `DestroyResource` (re-checks neighbours, removes it).
3. Road: sub-action `BuildRoad` (just sets `hasRoad`).
4. Road, route opener (data lists `routes`, e.g. port) or `Bridge` ability: sub-action `UpdateRoutes` (not for Road) and
   `UpdateCityConnections([player])`.
5. One `IncreaseCurrency(+1)` per star of the data's currency reward; then the cost is deducted immediately.
6. One `IncreasePopulation(source = tile, target = ruling city)` per population point of the reward.
7. `Manual` kinds set the unit on the tile to moved+attacked. `CheckSurroundingArea`. `Unique` kinds are recorded in
   `player.builtUniqueImprovements`.

Not implemented (stop with `Unimplemented`): `creates` (terrain/resource/unit/effect), bridge, ice bank, canal, flood/fill,
patina, and any neighbour whose level depends on adjacency (windmill, sawmill, forge, market, ...: `UpdateImprovementLevel`).

## Population, level-up, score
- `IncreasePopulationAction` (version > 114), target must be a city: `score0 = CalculateImprovementScore`; `xp += 1`
  (the game's `AddPopulation` writes the field at +0x1A, which the state calls `xp`); queue `IncreaseScore(score1 - score0)`;
  if `xp > level` and the tile owner is the current player, queue `CityLevelUp`.
- `CityLevelUpAction`: if still `xp > level`: `level += 1; xp -= level; upgrade -= 1`, queue
  `IncreaseScore(50 - level * 5)`, `EnableTask(6)`, add a `CommandTrigger(player, kind 1, coords)` (the pending reward), then
  `CheckTask(Metropolis)` (any owned city of level >= 5).
- `CalculateImprovementScore`: 0 on unowned tiles; `baseScore`; cities add `(level+1)*50 + xp*5` (starvation clamps at 0).
  `ScoreSheet` constants: tile 20, cityLevel 50, starvation 100, cityXP 5, explore 5.

## CityRewardCommand / CityRewardAction
- `IsValid` does NOT use `PassesBasicValidation` (a trigger is pending by design): current player, and a pending trigger
  of kind 1 at those coordinates, and the reward must be offered for the city's level (not checked).
- `Execute`: pop the pending trigger, queue `CityRewardAction`.
- Action: `AddReward`; Workshop and Park (version >= 99) push `ModifyProduction(+1)`. Then by reward:
  Park: `ModifyScore(+250)` + `IncreaseScore(250)`; Workshop / CityWall: nothing more; Resources: 5 x `IncreaseCurrency(+1)`;
  PopulationGrowth: 3 x `IncreasePopulation`. Explorer (ScoutMoveAction with random hash), BorderGrowth (ExpandCityAction) and
  SuperUnit (TrainUnitOnOccupiedSpace) are not implemented.

## Status
Unit-tested; not yet replay-verified because every fixture still hits a missing piece (multi-step Move / pathfinder, or one of
the three unimplemented rewards) before its final state can be compared.
