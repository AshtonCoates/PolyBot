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
- The version number of each older GameLogicData file (the number in the file name is a Unity path id).
- What the first 4 bytes of a savestate are, and the types/encoding of each field after them (see below).
- Combat formula, ability semantics, turn-end processing, map generation: all in code, not in data.

## Versions (observed, from `VersionManager` constants)
- `GAME_LOGIC_DATA_VERSION = 28`: current rules data, i.e. `GameLogicData28`.
- `GAME_VERSION = 122`: current game-logic/savestate version. `MIN_NEW_GAME_VERSION = 108`,
  `MIN_SUPPORTED_GAME_VERSION = 51`. Commands branch on versions (`ExecuteV59` ... `ExecuteV120`).
  Our target is version 122 only.

## Savestate format: what is known
Sample: `captures/*.pkl` (one capture, "move warrior", 9242 bytes).
- Observed: not compressed. Entropy 4.96 bits/byte; readable .NET-style length-prefixed strings
  (`0e "Pond of Piusum"`, player name, tribe names).
- Observed: bytes 4-7 are `7a 00 00 00` = int32 122 = `GAME_VERSION`.
- Observed: bytes 0-3 (`80 f6 4d 40`) are not the CRC32 or Adler32 of the rest of the file. Unknown (maybe a
  hash or other header); `SerializationHelpers.FromByteArray/PeekVersion/ReadVersion` handle it.
- Observed: the game has readers with `ReadCompressedInt32` (`SharedObjectReader`) and overridden
  `ReadInt16/ReadInt32/ReadString` (`ByteArrayReader`, `SharedObjectReader`), so integers are probably
  variable-length. Which reader loads savestates is not yet known.
- Observed (ISIL disassembly of `GameState.Deserialize`): fields are read in this order into these members:
  Version (+0x10), CurrentCommand (+0x48), CurrentTurn (+0x1c), CurrentPlayerIndex (+0x20),
  CurrentUnitId (+0x24), CurrentState (+0x28), Seed (+0x14), then `GameSettings`
  (`DeserializeDefault` for version > 69), then more. Read widths are NOT yet established (each read is a
  virtual call through an unnamed vtable slot).
- Plan: finish via the Ghidra decompile of `GameState.Deserialize`, `GameSettings.DeserializeDefault`,
  `MapData`, `PlayerState`, `TileData`, `UnitState`, and the reader classes.
