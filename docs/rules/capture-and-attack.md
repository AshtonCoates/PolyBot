# Capture, Attack, city names, city connections, routes

Read from the 2.17.3 binary (objdump of `GameAssembly.so`; most of these decompile badly). Code: `engine/src/engine.rs`
(`capture_*`, `attack_*`, `perform_attack`, `kill_unit_action`, `update_city_connections`), `engine/src/names.rs`.

## Capture (`CaptureCommand.ExecuteDefault` v>=116, `CaptureCityAction.ExecuteDefault` v>=109)
Command: `MakeExhausted(unit)` (moved + attacked, loses effects 7 and 2, followers too); queue `UpdateCityConnections([player, oldOwner])`,
`UpdateRoutes`, `CaptureCity` (the last runs first); invisible capturer -> `Reveal`.
Action: name the city if it has none (below); the unit standing there gets the city as home (unless Independent); other players' units
homed there lose their home; city counts of both players move; every tile of the city area (square of radius = border size, tiles with no
ruling city or ruled by this city) not owned by the capturer is taken: sub-action `IncreaseScore(20 + improvement score)` and, if it had an
owner, `DecreaseScore(old owner, same)`; tiles that were unowned and hold an improvement add their population. Level >= 2 city: Metropolis
task. Sub-actions are appended in order (`List.AddRange`, so the last runs first). Winner check, wipe check, then the city's own score is
added at once (and removed from the old owner), border tiles explored, relations updated
(`ModifyDiplomacyRelationsFromAttack`: previous/last attack turn, two `DestroyEmbassy`), and the city tile becomes Field (`Drain`).
Not implemented: capital capture (embassies), Spread / Flooded tribes, game over, wiping a player.

## City names (`MapDataExtensions.GenerateCityName` -> `PolyLanguage.MakeWord`)
`seed = XXHash(villageNameSeed)([x, y]) % 0x7fffffff`; `.NET System.Random(seed)`; length = `NextDouble()*3 + 3` (f32); append random
syllables of the tribe's `language` until the word is long enough, restarting if it starts with a space or hyphen; capitalise (not for
tribe 5). Verified against every capital name in the fixtures (`names::tests`).

## Attack (`AttackCommand.ExecuteDefault`, `BattleHelpers`, `ActionUtils.PerformAttackDefault`)
- attack = `data.attack*10` (+50 boosted); defence = `data.defence * bonus` with bonus 10, 15 on terrain a known tech protects, 15/40 for a
  fortifying unit in its own city (40 with city wall), halved when poisoned; max health = `data.health + 50*promotion`.
- `attackForce = hp*attack*100/maxHp`, `defenceForce = defence*hp*100/maxHp`, `damage = round10(attack*attackForce*450 /
  (total*1000))`, retaliation `round10(defenceForce*defence*4500/(total*1000*defenceBonus))` if the defender can retaliate (not
  Stiff / frozen; attacker not Convert / Surprise / Freeze; attacker within the defender's range); a kill means retaliation 0, and a
  melee unit (range 1) moves into the tile when a path exists ignoring units.
- Command: relation update, attacker `moved = !Escape`, `attacked = true` (or lose the double-attack effect); queue a `Move`
  (reason 1, path [target, origin]) on a kill, otherwise a retaliation `Attack` when it is > 0, then the main `Attack` (runs first).
  A visible target resets the Pacifist counter.
- `PerformAttack`: both units turn to face each other and lose effects 2 (and 6); health -= min(damage, health); on a kill by/against a
  non-Nature unit: kills += 1, attacker xp += 1 (not hidden / Static / vehicles), Killer task counter, casualties += 1, `KillUnit`.
- `KillUnit`: remove the unit, queue `DecreaseScore(owner, 5*cost)`.
Not implemented: Consume / Infiltrate / Convert / Eat / Splash / Freeze / Persist / Skate / Poison / Drench / Tentacles / Surprise
attackers, bubbles, followers, attacking an empty tile.

## City connections and routes
See `docs/rules/pathfinder.md` (route building). `UpdateCityConnections`: BFS from the capital over tiles that are roads / cities / route
openers on both sides (or both have routes); cities reached get `ConnectCity` (+1 population on the capital and the city, Network task),
cities no longer reached get `DisconnectCity` (-1 each). Network task: five connected cities.

Verified by `capture_1`, `attack_1`, `build_mine_upgrade` (exact replays).
