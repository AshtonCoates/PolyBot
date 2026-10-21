"""Parser for Polytopia savestates (game version 122), built from decompiled serializers.

Layers: LZ4 pickle (lz4pickle.py) -> .NET BinaryWriter payload:
    i32 version, LocalGameData{ GameState initial, GameState current, u16 lastSeenCommand }.
Everything is little-endian. Strings are .NET BinaryWriter strings (7-bit length prefix, UTF-8).

Implemented so far: GameState header, GameSettings, MapData, PlayerState, CommandTrigger, and a table of
command layouts (more are added as they are decoded; unknown command types raise).
See docs/game-findings.md for provenance and what is still unknown.
"""

from __future__ import annotations

import struct
from dataclasses import dataclass, field


class Reader:
    def __init__(self, data: bytes, pos: int = 0):
        self.data = data
        self.pos = pos

    def _take(self, fmt: str):
        size = struct.calcsize(fmt)
        if self.pos + size > len(self.data):
            raise EOFError(f"need {size} bytes at {self.pos}, have {len(self.data) - self.pos}")
        (value,) = struct.unpack_from(fmt, self.data, self.pos)
        self.pos += size
        return value

    def u8(self) -> int:
        return self._take("<B")

    def u16(self) -> int:
        return self._take("<H")

    def i16(self) -> int:
        return self._take("<h")

    def u32(self) -> int:
        return self._take("<I")

    def i32(self) -> int:
        return self._take("<i")

    def f32(self) -> float:
        return self._take("<f")

    def bool(self) -> bool:
        return self.u8() != 0

    def string(self) -> str:
        length = shift = 0
        while True:  # 7-bit encoded length
            b = self.u8()
            length |= (b & 0x7F) << shift
            if not b & 0x80:
                break
            shift += 7
        raw = self.data[self.pos : self.pos + length]
        if len(raw) != length:
            raise EOFError("string runs past end of data")
        self.pos += length
        return raw.decode("utf-8")

    def list_u16(self, count_reader=None) -> list[int]:
        n = (count_reader or self.u16)()
        return [self.u16() for _ in range(n)]


@dataclass
class Coords:
    x: int
    y: int


def read_coords(r: Reader) -> Coords:
    return Coords(r.i32(), r.i32())


@dataclass
class GameRules:
    turn_limit: int
    score_limit: int
    win_by_capital: bool
    allow_mirror_pick: bool
    allow_special_tribes: bool
    win_by_extermination: bool
    allow_tech_sharing: bool
    player_death_condition: int


def read_game_rules(r: Reader) -> GameRules:
    # Field order as written by GameRules.SerializeDefault (not declaration order).
    return GameRules(
        turn_limit=r.i32(),
        score_limit=r.i32(),
        win_by_capital=r.bool(),
        allow_mirror_pick=r.bool(),
        allow_special_tribes=r.bool(),
        win_by_extermination=r.bool(),
        allow_tech_sharing=r.bool(),
        player_death_condition=r.u16(),
    )


@dataclass
class GameSettings:
    rules: GameRules
    base_game_mode: int
    rules_game_mode: int
    game_name: str
    map_size: int
    disabled_tribes: list[int]
    unlocked_tribes: list[int]
    difficulty: int
    opponent_count: int
    game_type: int
    map_preset: int
    time_limit: int
    time_bonus_per_city: float
    time_bonus_per_population: float
    base_time_seconds: float
    use_dynamic_timers: bool
    use_time_banks: bool
    live_game_preset: bool
    is_auto_skip_enabled: bool
    selected_skins: dict[int, int] = field(default_factory=dict)  # tribe -> skin
    west_map_placement_user_id: str = ""


def read_game_settings(r: Reader, version: int) -> GameSettings:
    if version < 70:
        raise NotImplementedError("GameSettings.Serialize69 (version < 70) not implemented")
    s = GameSettings(
        rules=read_game_rules(r),
        base_game_mode=r.u8(),
        rules_game_mode=r.u8(),
        game_name=r.string(),
        map_size=r.i32(),
        disabled_tribes=r.list_u16(),
        unlocked_tribes=r.list_u16(),
        difficulty=r.u16(),
        opponent_count=r.i32(),
        game_type=r.u16(),
        map_preset=r.u8(),
        time_limit=r.i32(),
        time_bonus_per_city=r.f32(),
        time_bonus_per_population=r.f32(),
        base_time_seconds=r.f32(),
        use_dynamic_timers=r.bool(),
        use_time_banks=r.bool(),
        live_game_preset=r.bool(),
        is_auto_skip_enabled=r.bool(),
    )
    if version > 85:
        for _ in range(r.i32()):
            tribe = r.u16()
            s.selected_skins[tribe] = r.u16()
    if version > 121:
        s.west_map_placement_user_id = r.string()
    return s


@dataclass
class ImprovementState:
    type: int
    level: int
    founded: int
    xp: int
    population: int
    production: int
    base_score: int
    border_size: int
    upgrade: int
    connected_to_capital_of_player: int
    name: str
    founder: int
    rewards: list[int]
    effects: list[int]


def read_improvement(r: Reader) -> ImprovementState:
    type_ = r.u16()
    level = r.u16()
    founded = r.u16()
    xp = r.i16()
    population = r.i16()
    production = r.u16()
    base_score = r.u16()
    border_size = r.u16()
    upgrade = r.u16()
    connected = r.u8()
    name = r.string() if r.bool() else ""
    founder = r.u8()
    rewards = r.list_u16()
    effects = r.list_u16()
    return ImprovementState(
        type_, level, founded, xp, population, production, base_score, border_size,
        upgrade, connected, name, founder, rewards, effects,
    )


@dataclass
class UnitState:
    id: int
    owner: int
    type: int
    follower: int
    leader: int
    coordinates: Coords
    home: Coords
    health: int
    promotion_level: int
    xp: int
    moved: bool
    attacked: bool
    flipped: bool
    created_turn: int
    passenger: UnitState | None
    effects: list[int]
    birth_climate: int
    direction: int
    birth_climate_skin: int


def read_unit(r: Reader, version: int) -> UnitState:
    id_ = r.u32()
    owner = r.u8()
    type_ = r.u16()
    follower = r.u32()
    leader = r.u32()
    coordinates = read_coords(r)
    home = read_coords(r)
    health = r.u16()
    promotion_level = r.u16()
    xp = r.u16()
    moved = r.bool()
    attacked = r.bool()
    flipped = r.bool()
    created_turn = r.u16()
    passenger = read_unit(r, version) if r.bool() else None
    effects = r.list_u16()
    birth_climate = r.i16()
    direction = r.u8()
    skin = r.u16() if version >= 87 else 0
    return UnitState(
        id_, owner, type_, follower, leader, coordinates, home, health, promotion_level, xp,
        moved, attacked, flipped, created_turn, passenger, effects, birth_climate, direction, skin,
    )


@dataclass
class TileData:
    coordinates: Coords
    terrain: int
    climate: int
    altitude: int
    owner: int
    capital_of: int
    ruling_city: Coords
    resource: int | None
    improvement: ImprovementState | None
    unit: UnitState | None
    explorers: list[int]
    has_road: bool
    has_route: bool
    skin: int
    effects: list[int]


def read_tile(r: Reader, version: int) -> TileData:
    coordinates = read_coords(r)
    terrain = r.u16()
    climate = r.i16()
    altitude = r.i16()
    owner = r.u8()
    capital_of = r.u8()
    ruling_city = read_coords(r)
    resource = r.u16() if r.bool() else None
    improvement = read_improvement(r) if r.bool() else None
    unit = read_unit(r, version) if r.bool() else None
    explorers = [r.u8() for _ in range(r.u8())]
    has_road = r.bool()
    has_route = r.bool()
    skin = r.i32() if version >= 86 else 0
    effects = [r.i32() for _ in range(r.u8())] if version >= 105 else []
    return TileData(
        coordinates, terrain, climate, altitude, owner, capital_of, ruling_city, resource,
        improvement, unit, explorers, has_road, has_route, skin, effects,
    )


@dataclass
class MapData:
    width: int
    height: int
    tiles: list[TileData]


def read_map(r: Reader, version: int) -> MapData:
    width = r.u16()
    height = r.u16()
    tiles = [read_tile(r, version) for _ in range(width * height)]
    return MapData(width, height, tiles)


@dataclass
class DiplomacyRelation:
    state: int
    last_attack_turn: int
    embassy_level: int
    last_peace_broken_turn: int
    first_meet: int
    embassy_build_turn: int
    previous_attack_turn: int


def read_relation(r: Reader, version: int) -> DiplomacyRelation:
    return DiplomacyRelation(
        state=r.u8(),
        last_attack_turn=r.i32(),
        embassy_level=r.u8(),
        last_peace_broken_turn=r.i32(),
        first_meet=r.i32(),
        embassy_build_turn=r.i32(),
        previous_attack_turn=r.i32() if version > 80 else 0,
    )


@dataclass
class Task:
    type: int
    is_started: bool
    is_completed: bool
    counter: int | None  # Pacifist turns / Killer kills / Explorer tiles / Convert converts


# Task types (taskData idx) whose record carries an extra i32 after the two bools.
# Pacifist=1, Killer=5, Convert=9 come from the rules data; Explorer's id is not in the data
# (its serializer exists) so it is not handled here yet.
TASKS_WITH_COUNTER = {1, 5, 9}


def read_task(r: Reader) -> Task:
    type_ = r.u16()
    started, completed = r.bool(), r.bool()
    counter = r.i32() if type_ in TASKS_WITH_COUNTER else None
    return Task(type_, started, completed, counter)


@dataclass
class PlayerState:
    id: int
    user_name: str
    account_id: str
    auto_play: bool
    start_tile: Coords
    tribe: int
    has_chosen_tribe: bool
    handicap: int
    currency: int
    score: int
    end_score: int
    cities: int
    available_tech: list[int]
    known_players: list[int]
    tasks: list[Task]
    kills: int
    casualities: int
    wipe_outs: int
    color: int
    tribe_mix: int
    built_unique_improvements: list[int]
    relations: dict[int, DiplomacyRelation]
    messages: list[tuple[int, int]]  # (type, sender)
    killer_id: int
    killed_turn: int
    resigned_at_command_index: int
    wiped_at_command_index: int
    skin: int
    resigned_turn: int
    climate: int


def read_player(r: Reader, version: int) -> PlayerState:
    if version < 113:
        raise NotImplementedError("PlayerState layout before version 113 (aggressions) not implemented")
    p = PlayerState(
        id=r.u8(),
        user_name=r.string(),
        account_id=r.string(),
        auto_play=r.bool(),
        start_tile=read_coords(r),
        tribe=r.u16(),
        has_chosen_tribe=r.bool(),
        handicap=r.i32(),
        currency=r.i32(),
        score=r.u32(),
        end_score=r.u32(),
        cities=r.u16(),
        available_tech=r.list_u16(),
        known_players=[],
        tasks=[],
        kills=0, casualities=0, wipe_outs=0, color=0, tribe_mix=0,
        built_unique_improvements=[], relations={}, messages=[],
        killer_id=0, killed_turn=0, resigned_at_command_index=0, wiped_at_command_index=0,
        skin=-1, resigned_turn=0, climate=0,
    )
    p.known_players = [r.u8() for _ in range(r.u16())]
    p.tasks = [read_task(r) for _ in range(r.u16())]
    p.kills = r.u32()
    p.casualities = r.u32()
    p.wipe_outs = r.u32()
    p.color = r.i32()
    p.tribe_mix = r.u8()
    p.built_unique_improvements = [r.i16() for _ in range(r.u16())]
    if version >= 60:
        for _ in range(r.u16()):
            key = r.u8()
            p.relations[key] = read_relation(r, version)
        p.messages = [(r.u8(), r.u8()) for _ in range(r.u16())]
    p.killer_id = r.u8()
    p.killed_turn = r.u32()
    p.climate = p.tribe
    if version >= 70:
        p.resigned_at_command_index = r.i32()
        p.wiped_at_command_index = r.i32()
    if version >= 86:
        skin = r.u16()
        p.skin = -1 if skin == 0xFFFF else skin
    if version >= 93:
        p.resigned_turn = r.i32()
    if version >= 121:
        p.climate = r.u8()
    return p


@dataclass
class CommandTrigger:
    player_id: int
    type: int
    coordinates: Coords
    opponent_id: int


def read_trigger(r: Reader, version: int) -> CommandTrigger:
    player_id = r.u8()
    type_ = r.u16()
    coords = read_coords(r)
    opponent = r.u8() if version > 59 else 0
    return CommandTrigger(player_id, type_, coords, opponent)


# Command record layouts, version 122: after a u16 command type, each command is a sequence of
# fields. "player" = u8 PlayerId from CommandBase.Deserialize. Field kinds: u8 u16 u32 i32 bool coords.
# Decoded from the `*Command__Deserialize` functions; see docs/game-findings.md.
COMMAND_TYPES = {1: "Build", 2: "Attack", 3: "Recover", 4: "HealOthers", 5: "Train", 6: "Move", 7: "Capture", 8: "Research", 9: "Destroy", 10: "Disband", 11: "CityReward", 13: "Promote", 14: "ExamineRuins", 15: "EndTurn", 16: "Upgrade", 17: "FreezeArea", 18: "BreakIce", 20: "StartMatch", 21: "Stay", 22: "EndMatch", 24: "Explode", 25: "Boost", 26: "Decompose", 27: "PeaceTreaty", 28: "PeaceRequestResponse", 29: "BreakPeace", 30: "EstablishEmbassy", 32: "DestroyEmbassy", 33: "Hide", 35: "Resign", 36: "Disembark", 37: "Flood", 38: "Swarm", 39: "ClearTileEffect"}
COMMAND_LAYOUT = {
    1: [("player", "u8"), ("kind", "u16"), ("at", "coords")],
    2: [("player", "u8"), ("unit_id", "u32"), ("origin", "coords"), ("target", "coords")],
    3: [("player", "u8"), ("at", "coords")],
    4: [("player", "u8"), ("at", "coords")],
    5: [("player", "u8"), ("kind", "u16"), ("at", "coords")],
    6: [("player", "u8"), ("from", "coords"), ("to", "coords"), ("unit_id", "u32")],
    7: [("player", "u8"), ("unit_id", "u32"), ("at", "coords")],
    8: [("player", "u8"), ("tech", "u16")],
    9: [("player", "u8"), ("at", "coords")],
    10: [("player", "u8"), ("at", "coords")],
    11: [("player", "u8"), ("at", "coords"), ("reward", "u16")],
    13: [("player", "u8"), ("at", "coords")],
    14: [("player", "u8"), ("at", "coords")],
    15: [("player", "u8")],
    16: [("player", "u8"), ("kind", "u16"), ("at", "coords")],
    17: [("player", "u8"), ("at", "coords")],
    18: [("player", "u8"), ("at", "coords")],
    20: [("player", "u8")],
    21: [("player", "u8"), ("at", "coords")],
    22: [("player", "u8")],
    24: [("player", "u8"), ("at", "coords")],
    25: [("player", "u8"), ("at", "coords")],
    26: [("player", "u8"), ("at", "coords")],
    27: [("player", "u8"), ("at", "coords"), ("opponent", "u8")],
    28: [("player", "u8"), ("opponent", "u8"), ("accepted", "bool")],
    29: [("player", "u8"), ("at", "coords"), ("opponent", "u8")],
    30: [("player", "u8"), ("at", "coords"), ("opponent", "u8")],
    32: [("player", "u8"), ("at", "coords"), ("opponent", "u8")],
    33: [("player", "u8"), ("at", "coords")],
    35: [("player", "u8"), ("resigned", "u8"), ("kicker", "u8"), ("was_kicked", "bool")],
    36: [("player", "u8"), ("at", "coords")],
    37: [("player", "u8"), ("at", "coords")],
    38: [("player", "u8"), ("at", "coords")],
    39: [("player", "u8"), ("effect", "u16"), ("at", "coords")],
}

@dataclass
class Command:
    type: int
    fields: dict

    @property
    def name(self) -> str:
        return COMMAND_TYPES.get(self.type, f"Unknown({self.type})")


def _read_field(r: Reader, kind: str):
    if kind == "coords":
        return read_coords(r)
    return getattr(r, kind)()


def read_command(r: Reader, version: int) -> Command:
    type_ = r.u16()
    layout = COMMAND_LAYOUT.get(type_)
    if layout is None:
        raise NotImplementedError(f"command type {type_} at offset {r.pos - 2} not decoded yet")
    fields = {}
    for name, kind in layout:
        if type_ == 35 and name in ("kicker", "was_kicked") and version <= 92:  # Resign
            fields[name] = 0
            continue
        fields[name] = _read_field(r, kind)
    return Command(type_, fields)


@dataclass
class GameState:
    head: "GameStateHead"
    players: list[PlayerState]
    triggers: list[CommandTrigger]
    commands: list[Command]
    action_count: int  # actions are not decoded; only an empty action stack is supported
    has_flagged_need_for_update_routes: bool
    village_name_seed: int
    end: int


@dataclass
class GameStateHead:
    """Everything of GameState decoded so far; `end` is the offset just after the map."""

    version: int
    current_command: int
    current_turn: int
    current_player_index: int
    current_unit_id: int
    current_state: int
    seed: int
    settings: GameSettings
    map: MapData
    end: int


def read_game_state_head(data: bytes, offset: int) -> GameStateHead:
    r = Reader(data, offset)
    version = r.i32()
    current_command = r.u16()
    current_turn = r.u32()
    current_player_index = r.u8()
    current_unit_id = r.u32()
    current_state = r.u8()
    seed = r.i32()
    settings = read_game_settings(r, version)
    map_ = read_map(r, version)
    return GameStateHead(
        version, current_command, current_turn, current_player_index, current_unit_id,
        current_state, seed, settings, map_, r.pos,
    )


def read_game_state(data: bytes, offset: int) -> GameState:
    head = read_game_state_head(data, offset)
    version = head.version
    r = Reader(data, head.end)
    players = [read_player(r, version) for _ in range(r.u16())]
    triggers = [read_trigger(r, version) for _ in range(r.u16())]
    commands = [read_command(r, version) for _ in range(r.u16())]
    action_count = r.u16()
    if action_count:
        raise NotImplementedError(f"{action_count} actions in the action stack; action records not decoded yet")
    flag = r.bool() if version > 97 else False
    seed = r.i32() if version > 121 else 0
    return GameState(head, players, triggers, commands, action_count, flag, seed, r.pos)


@dataclass
class LocalGameData:
    version: int
    initial: GameState
    current: GameState
    last_seen_command: int


def parse_savestate(file_bytes: bytes) -> LocalGameData:
    """Parse the raw bytes of a `.state` file (LZ4 pickle -> LocalGameData)."""
    from .lz4pickle import unpickle

    data = unpickle(file_bytes)
    version = Reader(data).i32()
    initial = read_game_state(data, 4)
    current = read_game_state(data, initial.end)
    r = Reader(data, current.end)
    last_seen = r.u16()
    if r.pos != len(data):
        raise ValueError(f"{len(data) - r.pos} unparsed bytes at end of savestate")
    return LocalGameData(version, initial, current, last_seen)
