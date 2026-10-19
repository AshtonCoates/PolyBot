//! Serde mirror of `GameLogicData*.json`. Unknown (cosmetic) fields such as
//! colours, language syllables, skins and `shouldSuggestUnlock` are ignored.
//! Cross-references are still plain strings here; `Rules` resolves them.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::flags::{Effect, ImprovementAbility, Resource, Terrain, TribeAbility, UnitAbility};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawRules {
    pub tribe_data: BTreeMap<String, RawTribe>,
    pub tech_data: BTreeMap<String, RawTech>,
    pub unit_data: BTreeMap<String, RawUnit>,
    pub improvement_data: BTreeMap<String, RawImprovement>,
    pub terrain_data: BTreeMap<String, RawIdx>,
    pub resource_data: BTreeMap<String, RawResource>,
    pub task_data: BTreeMap<String, RawTask>,
    pub diplomacy_data: RawDiplomacy,
}

#[derive(Debug, Deserialize)]
pub struct RawIdx {
    pub idx: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawTribe {
    pub idx: u32,
    pub climate: u32,
    pub bonus: u32,
    pub category: u32,
    pub price_tier: u32,
    pub starting_unit: String,
    #[serde(default)]
    pub starting_tech: Vec<String>,
    #[serde(default)]
    pub starting_resource: Vec<Resource>,
    pub starting_stars: Option<i32>,
    #[serde(default)]
    pub tribe_abilities: Vec<TribeAbility>,
    #[serde(default)]
    pub terrain_modifier: BTreeMap<Terrain, f32>,
    #[serde(default)]
    pub resource_modifier: BTreeMap<Resource, f32>,
    #[serde(default)]
    pub unit_overrides: BTreeMap<String, String>,
    #[serde(default)]
    pub tech_overrides: BTreeMap<String, String>,
    #[serde(default)]
    pub improvement_overrides: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawTech {
    pub idx: u32,
    pub cost: i32,
    #[serde(default)]
    pub tech_unlocks: Vec<String>,
    #[serde(default)]
    pub unit_unlocks: Vec<String>,
    #[serde(default)]
    pub improvement_unlocks: Vec<String>,
    #[serde(default)]
    pub task_unlocks: Vec<String>,
    #[serde(default)]
    pub ability_unlocks: Vec<String>,
    #[serde(default)]
    pub movement_unlocks: BTreeMap<Terrain, i32>,
    #[serde(default)]
    pub defence_bonus_unlocks: BTreeMap<Terrain, i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawUnit {
    pub idx: u32,
    #[serde(default)]
    pub hidden: bool,
    pub cost: i32,
    pub health: i32,
    pub defence: i32,
    pub movement: i32,
    pub weapon: i32,
    pub range: i32,
    pub attack: i32,
    pub promotion_limit: i32,
    pub growth_rate: Option<i32>,
    pub upgrades_from: Option<String>,
    #[serde(default)]
    pub movement_terrain: Vec<Terrain>,
    #[serde(default)]
    pub unit_abilities: Vec<UnitAbility>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawImprovement {
    pub idx: u32,
    #[serde(default)]
    pub hidden: bool,
    pub cost: Option<i32>,
    pub max_level: Option<i32>,
    pub work: Option<i32>,
    pub border_size: Option<i32>,
    pub range: Option<i32>,
    pub growth_rate: Option<i32>,
    #[serde(default)]
    pub terrain_requirements: Vec<RawTerrainReq>,
    #[serde(default)]
    pub improvement_abilities: Vec<ImprovementAbility>,
    #[serde(default)]
    pub rewards: Vec<RawReward>,
    #[serde(default)]
    pub growth_rewards: Vec<RawReward>,
    #[serde(default)]
    pub adjacency_improvements: Vec<RawAdjacencyImprovement>,
    #[serde(default)]
    pub adjacency_requirements: Vec<RawAdjacencyRequirement>,
    #[serde(default)]
    pub routes: Vec<Terrain>,
    #[serde(default)]
    pub creates: Vec<RawCreates>,
}

/// Both keys may be present in one entry (e.g. field AND crop).
#[derive(Debug, Deserialize)]
pub struct RawTerrainReq {
    pub terrain: Option<Terrain>,
    pub resource: Option<Resource>,
}

#[derive(Debug, Deserialize)]
pub struct RawReward {
    #[serde(default)]
    pub population: i32,
    #[serde(default)]
    pub currency: i32,
    #[serde(default)]
    pub score: i32,
}

#[derive(Debug, Deserialize)]
pub struct RawAdjacencyImprovement {
    pub improvement: Option<String>,
    /// Key is spelled `resources` in the data but holds one resource.
    pub resources: Option<Resource>,
}

#[derive(Debug, Deserialize)]
pub struct RawAdjacencyRequirement {
    pub improvement: Option<String>,
    pub terrain: Option<Terrain>,
}

#[derive(Debug, Deserialize)]
pub struct RawCreates {
    pub terrain: Option<Terrain>,
    pub resource: Option<Resource>,
    pub unit: Option<String>,
    pub effect: Option<Effect>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawResource {
    pub idx: u32,
    #[serde(default)]
    pub resource_terrain_requirements: Vec<Terrain>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawTask {
    pub idx: u32,
    #[serde(default)]
    pub improvement_unlocks: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawDiplomacy {
    pub embassy_cost: i32,
    pub embassy_income: i32,
    /// Misspelled `embasyGrowthTime` in the game data.
    #[serde(rename = "embasyGrowthTime")]
    pub embassy_growth_time: i32,
    pub embassy_max_level: i32,
    pub embassy_tech_price_multiplier: f32,
    pub embassy_upgrade_cost: i32,
    pub dividend_income: i32,
}
