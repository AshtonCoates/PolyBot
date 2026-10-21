//! Dynamic game state, mirroring the savestate layout for game version 122.
//!
//! These are plain data types (no behaviour yet). Field meanings come from the decompiled
//! game; see docs/game-findings.md. Numeric type ids (`kind`, `terrain`, ...) are the game's own
//! `idx` values from the rules JSON (`Rules::*::game_idx`), not dense engine ids.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Coords {
    pub x: i32,
    pub y: i32,
}

impl fmt::Display for Coords {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({},{})", self.x, self.y)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameRules {
    pub turn_limit: i32,
    pub score_limit: i32,
    pub win_by_capital: bool,
    pub allow_mirror_pick: bool,
    pub allow_special_tribes: bool,
    pub win_by_extermination: bool,
    pub allow_tech_sharing: bool,
    pub player_death_condition: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameSettings {
    pub rules: GameRules,
    pub base_game_mode: u8,
    pub rules_game_mode: u8,
    pub game_name: String,
    pub map_size: i32,
    pub disabled_tribes: Vec<u16>,
    pub unlocked_tribes: Vec<u16>,
    pub difficulty: u16,
    pub opponent_count: i32,
    pub game_type: u16,
    pub map_preset: u8,
    pub time_limit: i32,
    pub time_bonus_per_city: f32,
    pub time_bonus_per_population: f32,
    pub base_time_seconds: f32,
    pub use_dynamic_timers: bool,
    pub use_time_banks: bool,
    pub live_game_preset: bool,
    pub is_auto_skip_enabled: bool,
    /// (tribe, skin) pairs in file order.
    pub selected_skins: Vec<(u16, u16)>,
    pub west_map_placement_user_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImprovementState {
    pub kind: u16,
    pub level: u16,
    pub founded: u16,
    pub xp: i16,
    pub population: i16,
    pub production: u16,
    pub base_score: u16,
    pub border_size: u16,
    pub upgrade: u16,
    pub connected_to_capital_of_player: u8,
    pub name: String,
    pub founder: u8,
    pub rewards: Vec<u16>,
    pub effects: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnitState {
    pub id: u32,
    pub owner: u8,
    pub kind: u16,
    pub follower: u32,
    pub leader: u32,
    pub coordinates: Coords,
    pub home: Coords,
    pub health: u16,
    pub promotion_level: u16,
    pub xp: u16,
    pub moved: bool,
    pub attacked: bool,
    pub flipped: bool,
    pub created_turn: u16,
    pub passenger: Option<Box<UnitState>>,
    pub effects: Vec<u16>,
    pub birth_climate: i16,
    pub direction: u8,
    pub birth_climate_skin: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TileData {
    pub coordinates: Coords,
    pub terrain: u16,
    pub climate: i16,
    pub altitude: i16,
    pub owner: u8,
    pub capital_of: u8,
    pub ruling_city: Coords,
    pub resource: Option<u16>,
    pub improvement: Option<ImprovementState>,
    pub unit: Option<UnitState>,
    /// Player ids that have explored this tile (fog of war).
    pub explorers: Vec<u8>,
    pub has_road: bool,
    pub has_route: bool,
    pub skin: i32,
    pub effects: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapData {
    pub width: u16,
    pub height: u16,
    /// Row-major: index = y * width + x.
    pub tiles: Vec<TileData>,
}

impl MapData {
    pub fn index(&self, c: Coords) -> Option<usize> {
        if c.x < 0 || c.y < 0 || c.x >= self.width as i32 || c.y >= self.height as i32 {
            None
        } else {
            Some(c.y as usize * self.width as usize + c.x as usize)
        }
    }

    pub fn tile(&self, c: Coords) -> Option<&TileData> {
        self.index(c).map(|i| &self.tiles[i])
    }

    pub fn tile_mut(&mut self, c: Coords) -> Option<&mut TileData> {
        self.index(c).map(move |i| &mut self.tiles[i])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiplomacyRelation {
    pub state: u8,
    pub last_attack_turn: i32,
    pub embassy_level: u8,
    pub last_peace_broken_turn: i32,
    pub first_meet: i32,
    pub embassy_build_turn: i32,
    pub previous_attack_turn: i32,
}

impl DiplomacyRelation {
    /// What `new DiplomacyRelation()` holds: no war/peace, nothing has happened yet ("never" is turn -100).
    pub fn default_relation() -> Self {
        DiplomacyRelation {
            state: 0,
            last_attack_turn: -100,
            embassy_level: 0,
            last_peace_broken_turn: -100,
            first_meet: -100,
            embassy_build_turn: -100,
            previous_attack_turn: -100,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiplomacyMessage {
    pub kind: u8,
    pub sender: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub kind: u16,
    pub is_started: bool,
    pub is_completed: bool,
    /// Pacifist turns / Killer kills / Explorer tiles / Convert converts.
    pub counter: Option<i32>,
}

/// Task types whose record carries an extra i32 counter: Pacifist 1, Killer 5, Convert 9 (ids from the rules
/// data) and Explorer, whose id is not in the data; 7 is the unused slot (inferred, unverified).
pub fn task_has_counter(kind: u16) -> bool {
    matches!(kind, 1 | 5 | 7 | 9)
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerState {
    pub id: u8,
    pub user_name: String,
    pub account_id: String,
    pub auto_play: bool,
    pub start_tile: Coords,
    pub tribe: u16,
    pub has_chosen_tribe: bool,
    pub handicap: i32,
    pub currency: i32,
    pub score: u32,
    pub end_score: u32,
    pub cities: u16,
    pub available_tech: Vec<u16>,
    pub known_players: Vec<u8>,
    pub tasks: Vec<Task>,
    pub kills: u32,
    pub casualities: u32,
    pub wipe_outs: u32,
    pub color: i32,
    pub tribe_mix: u8,
    pub built_unique_improvements: Vec<i16>,
    /// (other player id, relation) in file order.
    pub relations: Vec<(u8, DiplomacyRelation)>,
    pub messages: Vec<DiplomacyMessage>,
    pub killer_id: u8,
    pub killed_turn: u32,
    pub resigned_at_command_index: i32,
    pub wiped_at_command_index: i32,
    /// Raw u16; 0xffff means "none".
    pub skin: u16,
    pub resigned_turn: i32,
    pub climate: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandTrigger {
    pub player_id: u8,
    pub kind: u16,
    pub coordinates: Coords,
    pub opponent_id: u8,
}

/// Player commands (the game's `CommandBase` subclasses), version 123. Every command starts with the acting
/// player's id. Types the game defines but never serializes (`SelectTribe`, `Harvest`) are absent.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Build { player: u8, kind: u16, at: Coords },
    Attack { player: u8, unit_id: u32, origin: Coords, target: Coords },
    Recover { player: u8, at: Coords },
    HealOthers { player: u8, at: Coords },
    Train { player: u8, kind: u16, at: Coords },
    Move { player: u8, from: Coords, to: Coords, unit_id: u32 },
    Capture { player: u8, unit_id: u32, at: Coords },
    Research { player: u8, tech: u16 },
    Destroy { player: u8, at: Coords },
    Disband { player: u8, at: Coords },
    CityReward { player: u8, at: Coords, reward: u16 },
    Promote { player: u8, at: Coords },
    ExamineRuins { player: u8, at: Coords },
    EndTurn { player: u8 },
    Upgrade { player: u8, kind: u16, at: Coords },
    FreezeArea { player: u8, at: Coords },
    BreakIce { player: u8, at: Coords },
    StartMatch { player: u8 },
    Stay { player: u8, at: Coords },
    EndMatch { player: u8 },
    Explode { player: u8, at: Coords },
    Boost { player: u8, at: Coords },
    Decompose { player: u8, at: Coords },
    PeaceTreaty { player: u8, at: Coords, opponent: u8 },
    PeaceRequestResponse { player: u8, opponent: u8, accepted: bool },
    BreakPeace { player: u8, at: Coords, opponent: u8 },
    EstablishEmbassy { player: u8, at: Coords, opponent: u8 },
    DestroyEmbassy { player: u8, at: Coords, opponent: u8 },
    Hide { player: u8, at: Coords },
    Resign { player: u8, resigned: u8, kicker: u8, was_kicked: bool },
    Disembark { player: u8, at: Coords },
    Flood { player: u8, at: Coords },
    Swarm { player: u8, at: Coords },
    ClearTileEffect { player: u8, effect: u16, at: Coords },
}

impl Command {
    /// The game's `CommandType` enum value.
    pub fn type_id(&self) -> u16 {
        match self {
            Command::Build { .. } => 1,
            Command::Attack { .. } => 2,
            Command::Recover { .. } => 3,
            Command::HealOthers { .. } => 4,
            Command::Train { .. } => 5,
            Command::Move { .. } => 6,
            Command::Capture { .. } => 7,
            Command::Research { .. } => 8,
            Command::Destroy { .. } => 9,
            Command::Disband { .. } => 10,
            Command::CityReward { .. } => 11,
            Command::Promote { .. } => 13,
            Command::ExamineRuins { .. } => 14,
            Command::EndTurn { .. } => 15,
            Command::Upgrade { .. } => 16,
            Command::FreezeArea { .. } => 17,
            Command::BreakIce { .. } => 18,
            Command::StartMatch { .. } => 20,
            Command::Stay { .. } => 21,
            Command::EndMatch { .. } => 22,
            Command::Explode { .. } => 24,
            Command::Boost { .. } => 25,
            Command::Decompose { .. } => 26,
            Command::PeaceTreaty { .. } => 27,
            Command::PeaceRequestResponse { .. } => 28,
            Command::BreakPeace { .. } => 29,
            Command::EstablishEmbassy { .. } => 30,
            Command::DestroyEmbassy { .. } => 32,
            Command::Hide { .. } => 33,
            Command::Resign { .. } => 35,
            Command::Disembark { .. } => 36,
            Command::Flood { .. } => 37,
            Command::Swarm { .. } => 38,
            Command::ClearTileEffect { .. } => 39,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Command::Build { .. } => "Build",
            Command::Attack { .. } => "Attack",
            Command::Recover { .. } => "Recover",
            Command::HealOthers { .. } => "HealOthers",
            Command::Train { .. } => "Train",
            Command::Move { .. } => "Move",
            Command::Capture { .. } => "Capture",
            Command::Research { .. } => "Research",
            Command::Destroy { .. } => "Destroy",
            Command::Disband { .. } => "Disband",
            Command::CityReward { .. } => "CityReward",
            Command::Promote { .. } => "Promote",
            Command::ExamineRuins { .. } => "ExamineRuins",
            Command::EndTurn { .. } => "EndTurn",
            Command::Upgrade { .. } => "Upgrade",
            Command::FreezeArea { .. } => "FreezeArea",
            Command::BreakIce { .. } => "BreakIce",
            Command::StartMatch { .. } => "StartMatch",
            Command::Stay { .. } => "Stay",
            Command::EndMatch { .. } => "EndMatch",
            Command::Explode { .. } => "Explode",
            Command::Boost { .. } => "Boost",
            Command::Decompose { .. } => "Decompose",
            Command::PeaceTreaty { .. } => "PeaceTreaty",
            Command::PeaceRequestResponse { .. } => "PeaceRequestResponse",
            Command::BreakPeace { .. } => "BreakPeace",
            Command::EstablishEmbassy { .. } => "EstablishEmbassy",
            Command::DestroyEmbassy { .. } => "DestroyEmbassy",
            Command::Hide { .. } => "Hide",
            Command::Resign { .. } => "Resign",
            Command::Disembark { .. } => "Disembark",
            Command::Flood { .. } => "Flood",
            Command::Swarm { .. } => "Swarm",
            Command::ClearTileEffect { .. } => "ClearTileEffect",
        }
    }

    pub fn player(&self) -> u8 {
        match self {
            Command::Build { player, .. } => *player,
            Command::Attack { player, .. } => *player,
            Command::Recover { player, .. } => *player,
            Command::HealOthers { player, .. } => *player,
            Command::Train { player, .. } => *player,
            Command::Move { player, .. } => *player,
            Command::Capture { player, .. } => *player,
            Command::Research { player, .. } => *player,
            Command::Destroy { player, .. } => *player,
            Command::Disband { player, .. } => *player,
            Command::CityReward { player, .. } => *player,
            Command::Promote { player, .. } => *player,
            Command::ExamineRuins { player, .. } => *player,
            Command::EndTurn { player, .. } => *player,
            Command::Upgrade { player, .. } => *player,
            Command::FreezeArea { player, .. } => *player,
            Command::BreakIce { player, .. } => *player,
            Command::StartMatch { player, .. } => *player,
            Command::Stay { player, .. } => *player,
            Command::EndMatch { player, .. } => *player,
            Command::Explode { player, .. } => *player,
            Command::Boost { player, .. } => *player,
            Command::Decompose { player, .. } => *player,
            Command::PeaceTreaty { player, .. } => *player,
            Command::PeaceRequestResponse { player, .. } => *player,
            Command::BreakPeace { player, .. } => *player,
            Command::EstablishEmbassy { player, .. } => *player,
            Command::DestroyEmbassy { player, .. } => *player,
            Command::Hide { player, .. } => *player,
            Command::Resign { player, .. } => *player,
            Command::Disembark { player, .. } => *player,
            Command::Flood { player, .. } => *player,
            Command::Swarm { player, .. } => *player,
            Command::ClearTileEffect { player, .. } => *player,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameState {
    pub version: i32,
    /// Index of the next command in `commands` to execute.
    pub current_command: u16,
    pub current_turn: u32,
    pub current_player_index: u8,
    /// Next unit id to hand out.
    pub current_unit_id: u32,
    pub current_state: u8,
    pub seed: i32,
    pub settings: GameSettings,
    pub map: MapData,
    pub players: Vec<PlayerState>,
    pub triggers: Vec<CommandTrigger>,
    /// Full command history (executed and pending).
    pub commands: Vec<Command>,
    pub has_flagged_need_for_update_routes: bool,
    pub village_name_seed: i32,
}

/// The contents of a `.state` file.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalGameData {
    /// Outer version written by `SerializationHelpers.ToByteArray`.
    pub version: i32,
    pub initial: GameState,
    pub current: GameState,
    pub last_seen_command: u16,
}
