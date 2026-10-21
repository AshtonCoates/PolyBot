# MeetAction and first contact

Sources: `ExploreAction.ExecuteDefault`, `ActionUtils.CheckTribeConnection`, `MeetAction.IsValid/Execute(Default)/GetReward`,
`ActionUtils.TryRevealCapital`, `PlayerExtensions.KnowsPlayer`, `StartTurnAction.CheckDiplomacy`.

- `KnowsPlayer(p, other)`: Nature (255) and oneself are always known, otherwise `other` is in `p.knownPlayers`.
- `ExploreAction`: marks the tile explored, flags routes for update, queues `IncreaseScore(5)`, then queues `MeetAction(p, owner, tile)`
  for an unknown owner of the unit standing there, and for an unknown owner of a city there (a city whose owner differs from the unit's
  is queued second, so it runs first). Revealing a map corner (the four lighthouse tiles) calls `OnRevealLighthouseTile`:
  `CheckTask(8)`, `EnableTask(8)`, `IncreasePopulation(capital)`, `ImprovementLevelUp(tile)` (the lighthouse level = number of players that
  explored it; the action is only valid while that exceeds the stored level). The Explore-lighthouses task (8) completes when the player
  has explored all four corners.
- `CheckTribeConnection` (start of turn): the same meets for every tile the player has explored.
- `MeetAction.IsValid`: not yet `KnowsPlayer` (a second queued meeting with the same player is skipped).
- `MeetAction.Execute` (game version >= 113): `TryRevealCapital` (with capital vision, explore the other player's start tile),
  append the other player to `knownPlayers`, create the relation if needed and set `firstMeet = current turn`, then queue
  `IncreaseCurrency(reward)`. The other player gets nothing; they meet in their own turn.
- Reward: `2 * min(round(otherScore / 1000), 5) + 1` (banker's rounding); in game version >= 123 a two-player game gives `+2` more when
  the met player is player 1 ("away stars").
- `StartTurn.CheckDiplomacy` does nothing for a relation that only has `firstMeet` set (no messages, no peace, no embassy); anything else
  still stops with an error.

Verified by: `early_game_turn2`, `pass_turn_1` (still exact) and the longer parts of `attack_1`, `build_mine_upgrade`, `metropolis`,
`full_game_1` that now get past their first meetings.
