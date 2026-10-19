# Game findings

"Observed" = seen directly in dumped data. "Inferred" = reasoned, not verified.

## Structure (observed)
- `GameLogicAssembly` is a separate assembly with 242 types holding the rules, independent of rendering.
- State: `GameState`, `PlayerState`, `TileData`, `UnitState`, `ImprovementState`, `ResourceState`,
  `DiplomacyRelationState`, `MapData`.
- About 30 `*Command` classes (Attack, Move, Build, Train, Research, Capture, Promote, Upgrade,
  EndTurn, Disband, Decompose, Recover, Boost, PeaceTreaty, ...). Each has `IsValid(GameState, out string)`,
  `Execute(GameState)`, `Serialize/Deserialize(BinaryReader, int version)`.
  `IsValid` is effectively the legal-move check; `Deserialize` is the savestate format.
- Some commands have versioned executors (`ExecuteV59` ... `ExecuteV120`, `ExecuteDefault`).
  Plan: implement only `ExecuteDefault`.
- Also present: `BattleHelpers`, `MapGenerator`, `MapGeneratorPreNaval`, `AI|ScoredCommand`,
  `PickBestPossibleCommand` (suggests a built-in AI; not examined).

## Rule data (observed)
- `PolytopiaDataHolder.gameLogicDatas[]` holds versioned `GameLogicData` TextAssets (JSON), loaded via
  `PolytopiaDataManager.LoadGameLogicData(version)`.
- 28 files extracted. Latest file keys: tribeData (18), techData (49), unitData (59),
  improvementData (49), terrainData (9), resourceData (10), taskData (9), skinData (3), diplomacyData (7).
- Example: warrior = health 100, attack 20, defence 20, movement 1, range 1, cost 2, abilities
  dash/fortify/land.
- Files 27 vs 28 differ only in unit `moth`.

## Data quirks (observed)
- `techData.rituals` has the key `cost` twice (both 2). Python accepts it; serde's derive rejects it, so the
  Rust loader parses via `serde_json::Value` (last duplicate wins).
- `tribeData.*.techOverrides` references a tech `whaling`, and `techData.*.improvementUnlocks` references an
  improvement `enchantwhale`; neither exists in the data. The loader drops these and records warnings.
- Some requirement entries hold several keys at once (e.g. `{terrain: field, resource: crop}`), so they are
  structs of optional fields, not enums. `adjacencyImprovements[].resources` holds a single resource.
- `idx` values have gaps (e.g. tasks skip 7), so the engine uses dense ids and keeps `game_idx` for savestates.

## Not known yet
- The real version number of each GameLogicData file (script to read it printed nothing; the number
  in the file name is a Unity path id, not the version).
- Which version a new game uses (assumed: the highest, unverified).
- Savestate encoding. First bytes of the captured state: `80 f6 4d 40 7a 00 00 00 04 00 03 02 ...`;
  not a plain version field. The game ships an LZ4 library, so it may be compressed (unverified).
- Combat formula, ability semantics, turn-end processing, map generation: all in code, not in data.
