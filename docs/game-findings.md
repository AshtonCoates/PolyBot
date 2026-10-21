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

## Savestate format
Status: all eight fixtures in `testdata/savestates/` parse with no leftover bytes (Rust: `savestate/`, Python: `parsing/savestate.py`).
Decoded: container, `GameState`, settings, map, players, triggers and **all 34 command types the game serializes**. Not decoded: action
records (the action stack is empty in every save; the loaders raise if it is not).
Parser: `src/polybot/parsing/savestate.py`.

Correction: an earlier note here said the file was not compressed. That was wrong. It is LZ4-compressed; LZ4 keeps
incompressible runs (strings) as raw literals, which is why strings looked readable.

### Layers (observed in decompiled code, verified on `captures/*.pkl`)
1. **File** = `ToLZ4CompressedByteArray`: `K4os.Compression.LZ4.LZ4Pickler.Pickle(payload)` (see
   `ClientBase.SaveLocalGameData` -> `DiskSerializationHelpers.ToDisk`, which writes `<file>.tmp` then moves it).
   Pickler header v0: byte 0 = flags (bits 0-2 version = 0; bits 6-7 = size-of-diff code, 3 means 4 bytes);
   then that many little-endian bytes = (uncompressed length - compressed length); then a raw LZ4 block.
   Sample: header `80 f6 4d 40...` = 2-byte diff 0x4df6, data from offset 3, 9239 -> 29197 bytes.
   Decoder: `src/polybot/parsing/lz4pickle.py` (`unpickle`).
2. **Payload** = `SerializationHelpers.ToByteArray(LocalGameData, version)`: `.NET BinaryWriter`, little-endian:
   `i32 version` (122) then `LocalGameData.Serialize`.
3. **LocalGameData.Serialize**: `GameState` (initialGameState), `GameState` (currentGameState), `u16 lastSeenCommand`.
   Sample: initial state at payload offset 4 (turn 0), current state at 13793 (turn 2, command 41), file ends
   `29 00` = 41 = lastSeenCommand. So a save holds the starting state AND the current one.
4. **GameState.Serialize** (from `GameState__Serialize`, version 122):
   `i32 Version, u16 CurrentCommand, u32 CurrentTurn, u8 CurrentPlayerIndex, u32 CurrentUnitId (next unit id),
   u8 CurrentState, i32 Seed`, then `GameSettings` (`SerializeDefault`; `Serialize69` if version < 70),
   `MapData`, `u16 playerCount` + `PlayerState` each, `u16 n` + `CommandTrigger` each, `u16 n` + commands
   (each `u16 commandType` then its data), `u16 n` + actions (same pattern), `bool HasFlaggedNeedForUpdateRoutes`
   (version > 97), `i32 VillageNameSeed` (version > 121).
   Verified on the sample for the 7 header fields of both states (identical seed, plausible values).
   Field widths come from BinaryWriter vtable slots: 0x1c8 bool, 0x1d8 byte, 0x258 u16, 0x268 i32, 0x278 u32.
   (Slots inferred from which overload each field needs; consistent with the sample.)

### Layouts decoded (version 122; all little-endian, `str` = 7-bit length + UTF-8)
- **GameRules** (15 bytes): `i32 turnLimit, i32 scoreLimit, bool winByCapital, bool allowMirrorPick, bool allowSpecialTribes,
  bool winByExtermination, bool allowTechSharing, u16 playerDeathCondition`. (Written order differs from declaration order.)
- **GameSettings**: `GameRules, u8 baseGameMode, u8 rulesGameMode, str gameName, i32 mapSize, u16 n + u16[] disabledTribes,
  u16 n + u16[] unlockedTribes, u16 difficulty, i32 opponentCount, u16 gameType, u8 mapPreset, i32 timeLimit,
  f32 timeBonusPerCity, f32 timeBonusPerPopulation, f32 baseTimeSeconds, bool useDynamicTimers, bool useTimeBanks,
  bool livePreset, bool autoSkip`, then if version > 85 `i32 n + (u16 tribe, u16 skin)[]`, if version > 121 `str westMapPlacementUserId`.
  (`players`/`spectators` lists are NOT part of this record.)
- **WorldCoordinates**: `i32 x, i32 y`.
- **MapData**: `u16 width, u16 height`, then width*height `TileData` in row-major order (x fastest). Continents are not stored.
- **TileData**: `coords, u16 terrain, i16 climate, i16 altitude, u8 owner, u8 capitalOf, coords rulingCity,
  bool hasResource [+ ResourceState], bool hasImprovement [+ ImprovementState], bool hasUnit [+ UnitState],
  u8 n + u8[] explorers, bool hasRoad, bool hasRoute`, then if version >= 86 `i32 skin`, if version >= 105 `u8 n + i32[] effects`.
- **ResourceState**: `u16 type`.
- **ImprovementState**: `u16 type, u16 level, u16 founded, i16 xp, i16 population, u16 production, u16 baseScore, u16 borderSize,
  u16 upgrade, u8 connectedToCapitalOfPlayer, bool hasName [+ str name], u8 founder, u16 n + u16[] rewards, u16 n + u16[] effects`.
  (Owner comes from the tile.)
- **UnitState**: `u32 id, u8 owner, u16 type, u32 follower, u32 leader, coords coordinates, coords home, u16 health,
  u16 promotionLevel, u16 xp, bool moved, bool attacked, bool flipped, u16 createdTurn, bool hasPassenger [+ UnitState],
  u16 n + u16[] effects, i16 birthClimate, u8 direction`, then if version >= 87 `u16 birthClimateSkin`.
- Enum ids in the save equal the `idx` values in the rules JSON (terrain 1 water, 2 ocean, 3 field, 4 mountain, 5 forest;
  resources 1 game ... 9 aquacrop; unit type 2 warrior, 3 rider, ...). Improvement type 1 = city.

- **PlayerState** (v122): `u8 id, str userName, str accountId (Guid string), bool autoPlay, coords startTile, u16 tribe,
  bool hasChosenTribe, i32 handicap, i32 currency, u32 score, u32 endScore, u16 cities, u16 n + u16[] availableTech,
  u16 n + u8[] knownPlayers, u16 n + Task[], u32 kills, u32 casualities, u32 wipeOuts, i32 color, u8 tribeMix,
  u16 n + i16[] builtUniqueImprovements, (version>=60) u16 n + (u8 playerId, DiplomacyRelation)[] then u16 n + DiplomacyMessage[],
  u8 killerId, u32 killedTurn`, then (version>=70) `i32 resignedAtCommandIndex, i32 wipedAtCommandIndex`, (>=86) `u16 skin (0xffff=-1)`,
  (>=93) `i32 resignedTurn`, (>=121) `u8 climate`. (Versions before 113 also store an aggressions dictionary; not implemented.)
  Player 255 is "Nature". Player colours equal `tribeData.*.color` for the AI tribes.
- **Task**: `u16 type, bool isStarted, bool isCompleted`, plus `i32 counter` for the types that have one (pacifist 1, killer 5,
  convert 9; explorer has one too but its type id is not in the rules data).
- **DiplomacyRelation**: `u8 state, i32 lastAttackTurn, u8 embassyLevel, i32 lastPeaceBrokenTurn, i32 firstMeet,
  i32 embassyBuildTurn, (version>80) i32 previousAttackTurn`. Unmet players have firstMeet = -100.
- **DiplomacyMessage**: `u8 type, u8 sender`.
- **CommandTrigger**: `u8 playerId, u16 type, coords, (version>59) u8 opponentId`.
- **Command stack**: `u16 n`, each `u16 commandType` + body. Every body starts with `u8 playerId` (read inline or by `CommandBase.Deserialize`,
  checked for all). Bodies after the player id (kinds: coords = two i32):
  Build 1 `u16 improvement, coords`; Attack 2 `u32 unitId, coords origin, coords target`; Recover 3, HealOthers 4, Destroy 9, Disband 10, Promote 13,
  ExamineRuins 14, FreezeArea 17, BreakIce 18, Stay 21, Explode 24, Boost 25, Decompose 26, Hide 33, Disembark 36, Flood 37, Swarm 38: `coords`;
  Train 5 `u16 unitType, coords`; Move 6 `coords from, coords to, u32 unitId`; Capture 7 `u32 unitId, coords`; Research 8 `u16 tech`;
  CityReward 11 `coords, u16 reward`; EndTurn 15, StartMatch 20, EndMatch 22: nothing; Upgrade 16 `u16 unitType, coords`;
  PeaceTreaty 27, BreakPeace 29, EstablishEmbassy 30, DestroyEmbassy 32: `coords, u8 opponent`; PeaceRequestResponse 28 `u8 opponent, bool accepted`;
  Resign 35 `u8 resigned` then, if version > 92, `u8 kicker, bool wasKicked`; ClearTileEffect 39 `u16 effect, coords`.
  `SelectTribe` (19) and `Harvest` (23) are ids in the enum but have no command class, so they never appear.
  Verified against real saves (parse consumes the file exactly): Build, Attack, Train, Move, Capture, Research, CityReward, ExamineRuins, EndTurn, StartMatch,
  Recover, Stay, Upgrade. Decoded from the code only (no save contains them): the rest.
- **Action stack**: `u16 n`, each `u16 actionType` + body starting with `u8 playerId` (ActionType enum has ~85 types; decompiled in
  `~/src/polybot_re_out/decomp/cmd/*Action__Deserialize.c`, not yet read).
- After the stacks: `bool hasFlaggedNeedForUpdateRoutes` (version > 97), `i32 villageNameSeed` (version > 121).

### Verification on the sample (18x18 map, 5 tribes, 2 states)
- Whole file: initial state 4..13793, current state 13793..29195, then `u16 lastSeenCommand = 41`; zero bytes left over.
  The 41 commands read as a coherent game: StartMatch, capital Builds + CityRewards, Moves, EndTurns, Research, Train.
- Both states: 324 tiles, coordinates exactly row-major, same terrain counts; each tribe owns 9 tiles (city + border);
  initial state has 5 units (turn 0), current state 10 (turn 2, command 41). After the map comes `u16 playerCount = 6`
  (5 tribes + player 0 = none/nature) then the first `PlayerState` (player name visible).
- Not yet verified: any non-default branch (older versions), second-level details like unit `leader/follower`, `effects` lists
  (all empty or zero in this sample).

### Other observations
- `ByteArrayReader` / `SharedObjectReader` (big-endian ints, compressed ints) are for network/shared data, not the
  savestate, which uses a plain `BinaryReader`/`BinaryWriter`.
- Decompiled serializers are in `~/src/polybot_re_out/decomp/ser/` (`GameSettings`, `MapData`, `TileData`,
  `PlayerState`, `UnitState`, `ImprovementState`, `ResourceState`, `DiplomacyRelation`, `WorldCoordinates`,
  `GameRules`, `GameState` command/action (de)serializers). Four functions timed out in Ghidra (120 s):
  `LocalGameData__Serialize/Deserialize`, `SharedObjectReader__ReadString` (x2). `LocalGameData.Serialize` was read
  from the ISIL dump instead.
