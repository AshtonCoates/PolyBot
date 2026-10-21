//! Static rule tables (units, techs, tribes, improvements, ...), loaded from the
//! game's own `GameLogicData*.json` and compiled into dense, id-indexed tables.
//!
//! The JSON refers to everything by name. Loading resolves those names once, so
//! the simulation only ever handles small `Copy` ids and bitsets.
//!
//! Scope: one rules version (the file under `re/data/`). Which game version
//! that corresponds to is still to be confirmed (see docs/status.md).

mod flags;
mod ids;
mod raw;

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use serde::de::DeserializeOwned;

pub use flags::{
    Effect, Flag, FlagSet, ImprovementAbility, Resource, Terrain, TribeAbility, UnitAbility,
};
pub use ids::{ImprovementId, TaskId, TechId, TribeId, UnitId};

#[derive(Debug)]
pub enum RulesError {
    Json(serde_json::Error),
    /// A terrain/resource name is unknown, or its `idx` differs from the enum value.
    TagMismatch { kind: &'static str, name: String, idx: u32 },
    /// A fixed enum has no entry in the data.
    MissingTag { kind: &'static str, name: &'static str },
    /// More entries than a `u8` id can address.
    TooMany { kind: &'static str },
}

impl fmt::Display for RulesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RulesError::Json(e) => write!(f, "invalid rules JSON: {e}"),
            RulesError::TagMismatch { kind, name, idx } => {
                write!(f, "{kind} '{name}' (idx {idx}) does not match the engine's enum")
            }
            RulesError::MissingTag { kind, name } => write!(f, "{kind} '{name}' missing from data"),
            RulesError::TooMany { kind } => write!(f, "too many {kind} entries for a u8 id"),
        }
    }
}

impl std::error::Error for RulesError {}

impl From<serde_json::Error> for RulesError {
    fn from(e: serde_json::Error) -> Self {
        RulesError::Json(e)
    }
}

#[derive(Debug, Clone)]
pub struct TribeData {
    pub name: String,
    pub game_idx: u32,
    /// Default player colour (RGB packed in an int).
    pub color: i32,
    pub climate: u32,
    pub bonus: u32,
    pub category: u32,
    pub price_tier: u32,
    pub starting_unit: UnitId,
    pub starting_techs: Vec<TechId>,
    pub starting_resources: Vec<Resource>,
    pub starting_stars: Option<i32>,
    pub abilities: FlagSet<TribeAbility>,
    /// Multipliers on terrain generation chance for this tribe's map (data holds floats).
    pub terrain_modifier: Vec<(Terrain, f32)>,
    pub resource_modifier: Vec<(Resource, f32)>,
    /// Tribe-specific replacements, e.g. polaris: giant -> gaami.
    pub unit_overrides: Vec<(UnitId, UnitId)>,
    pub tech_overrides: Vec<(TechId, TechId)>,
    pub improvement_overrides: Vec<(ImprovementId, ImprovementId)>,
    /// Comma-separated syllables used to make city names.
    pub language: String,
}

#[derive(Debug, Clone)]
pub struct TechData {
    pub name: String,
    pub game_idx: u32,
    pub cost: i32,
    pub tech_unlocks: Vec<TechId>,
    pub unit_unlocks: Vec<UnitId>,
    pub improvement_unlocks: Vec<ImprovementId>,
    pub task_unlocks: Vec<TaskId>,
    /// Kept as names until their meaning is known: pontoon, bubble, embassy, disband, ...
    pub ability_unlocks: Vec<String>,
    pub movement_unlocks: Vec<(Terrain, i32)>,
    pub defence_bonus_unlocks: Vec<(Terrain, i32)>,
}

#[derive(Debug, Clone)]
pub struct UnitData {
    pub name: String,
    pub game_idx: u32,
    pub hidden: bool,
    pub cost: i32,
    /// Health is on the game's x10 scale (a warrior has 100).
    pub health: i32,
    pub defence: i32,
    pub movement: i32,
    pub weapon: i32,
    pub range: i32,
    pub attack: i32,
    pub promotion_limit: i32,
    pub growth_rate: Option<i32>,
    pub upgrades_from: Option<UnitId>,
    /// Terrains this unit moves on when it differs from the default (aquatic units).
    pub movement_terrain: Option<FlagSet<Terrain>>,
    pub abilities: FlagSet<UnitAbility>,
}

/// Placement requirement. When both fields are set, both must hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainReq {
    pub terrain: Option<Terrain>,
    pub resource: Option<Resource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Reward {
    pub population: i32,
    pub currency: i32,
    pub score: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdjacencyImprovement {
    pub improvement: Option<ImprovementId>,
    pub resource: Option<Resource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdjacencyRequirement {
    pub improvement: Option<ImprovementId>,
    pub terrain: Option<Terrain>,
}

/// What building the improvement changes on the tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Creates {
    pub terrain: Option<Terrain>,
    pub resource: Option<Resource>,
    pub unit: Option<UnitId>,
    pub effect: Option<Effect>,
}

#[derive(Debug, Clone)]
pub struct ImprovementData {
    pub name: String,
    pub game_idx: u32,
    pub hidden: bool,
    /// None for improvements that cannot be bought directly.
    pub cost: Option<i32>,
    pub max_level: Option<i32>,
    pub work: Option<i32>,
    pub border_size: Option<i32>,
    pub range: Option<i32>,
    pub growth_rate: Option<i32>,
    pub terrain_requirements: Vec<TerrainReq>,
    pub abilities: FlagSet<ImprovementAbility>,
    pub rewards: Vec<Reward>,
    pub growth_rewards: Vec<Reward>,
    pub adjacency_improvements: Vec<AdjacencyImprovement>,
    pub adjacency_requirements: Vec<AdjacencyRequirement>,
    pub routes: FlagSet<Terrain>,
    pub creates: Vec<Creates>,
}

#[derive(Debug, Clone)]
pub struct TaskData {
    pub name: String,
    pub game_idx: u32,
    pub improvement_unlocks: Vec<ImprovementId>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiplomacyData {
    pub embassy_cost: i32,
    pub embassy_income: i32,
    pub embassy_growth_time: i32,
    pub embassy_max_level: i32,
    pub embassy_tech_price_multiplier: f32,
    pub embassy_upgrade_cost: i32,
    pub dividend_income: i32,
}

#[derive(Debug, Clone)]
pub struct Rules {
    tribes: Vec<TribeData>,
    techs: Vec<TechData>,
    units: Vec<UnitData>,
    improvements: Vec<ImprovementData>,
    tasks: Vec<TaskData>,
    tribe_ids: HashMap<String, TribeId>,
    tech_ids: HashMap<String, TechId>,
    unit_ids: HashMap<String, UnitId>,
    improvement_ids: HashMap<String, ImprovementId>,
    task_ids: HashMap<String, TaskId>,
    /// Terrains each resource can spawn on, indexed by `Resource as usize`.
    resource_terrains: Vec<FlagSet<Terrain>>,
    pub diplomacy: DiplomacyData,
    /// References in the data that point at nothing; those links were dropped.
    pub warnings: Vec<String>,
}

macro_rules! accessors {
    ($field:ident, $ids:ident, $id:ident, $data:ident, $get:ident, $lookup:ident, $all:ident) => {
        pub fn $get(&self, id: $id) -> &$data {
            &self.$field[id.index()]
        }
        pub fn $lookup(&self, name: &str) -> Option<$id> {
            self.$ids.get(name).copied()
        }
        pub fn $all(&self) -> &[$data] {
            &self.$field
        }
    };
}

impl Rules {
    accessors!(tribes, tribe_ids, TribeId, TribeData, tribe, tribe_id, tribes);
    accessors!(techs, tech_ids, TechId, TechData, tech, tech_id, techs);
    accessors!(units, unit_ids, UnitId, UnitData, unit, unit_id, units);
    accessors!(
        improvements,
        improvement_ids,
        ImprovementId,
        ImprovementData,
        improvement,
        improvement_id,
        improvements
    );
    accessors!(tasks, task_ids, TaskId, TaskData, task, task_id, tasks);

    pub fn resource_terrains(&self, resource: Resource) -> FlagSet<Terrain> {
        self.resource_terrains[resource as usize]
    }

    /// The rules bundled with the crate (`re/data/GameLogicData28.json`, the current game version).
    pub fn bundled() -> Rules {
        Rules::from_json(include_str!("../../../re/data/GameLogicData28.json"))
            .expect("bundled rules are valid")
    }

    /// Look up a unit by the game's own numeric type id (as stored in savestates).
    pub fn unit_by_game_idx(&self, idx: u32) -> Option<&UnitData> {
        self.units.iter().find(|u| u.game_idx == idx)
    }

    pub fn tribe_by_game_idx(&self, idx: u32) -> Option<&TribeData> {
        self.tribes.iter().find(|t| t.game_idx == idx)
    }

    pub fn tech_by_game_idx(&self, idx: u32) -> Option<&TechData> {
        self.techs.iter().find(|t| t.game_idx == idx)
    }

    pub fn improvement_by_game_idx(&self, idx: u32) -> Option<&ImprovementData> {
        self.improvements.iter().find(|i| i.game_idx == idx)
    }

    pub fn from_json(json: &str) -> Result<Rules, RulesError> {
        // Via `Value` on purpose: the data has a duplicated key (`rituals.cost`, same value
        // twice), which serde's derive rejects but `Value` accepts (last one wins).
        let value: serde_json::Value = serde_json::from_str(json)?;
        let raw: raw::RawRules = serde_json::from_value(value)?;
        compile(raw)
    }
}

/// Resolves names to ids, recording (not failing on) dangling references.
struct Resolver {
    warnings: Vec<String>,
}

impl Resolver {
    fn look<I: Copy>(
        &mut self,
        ids: &HashMap<String, I>,
        kind: &str,
        owner: &str,
        name: &str,
    ) -> Option<I> {
        let found = ids.get(name).copied();
        if found.is_none() {
            self.warnings.push(format!("{owner}: unknown {kind} '{name}' (dropped)"));
        }
        found
    }

    fn look_all<I: Copy>(
        &mut self,
        ids: &HashMap<String, I>,
        kind: &str,
        owner: &str,
        names: &[String],
    ) -> Vec<I> {
        names.iter().filter_map(|n| self.look(ids, kind, owner, n)).collect()
    }

    fn look_pairs<I: Copy>(
        &mut self,
        ids: &HashMap<String, I>,
        kind: &str,
        owner: &str,
        map: &BTreeMap<String, String>,
    ) -> Vec<(I, I)> {
        map.iter()
            .filter_map(|(from, to)| {
                let from = self.look(ids, kind, owner, from)?;
                let to = self.look(ids, kind, owner, to)?;
                Some((from, to))
            })
            .collect()
    }
}

/// Names in dense-id order: sorted by the game's `idx`.
fn ordered<'a, T>(
    kind: &'static str,
    map: &'a BTreeMap<String, T>,
    idx: impl Fn(&T) -> u32,
) -> Result<Vec<(&'a String, &'a T)>, RulesError> {
    if map.len() > u8::MAX as usize {
        return Err(RulesError::TooMany { kind });
    }
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_by_key(|(_, v)| idx(v));
    Ok(entries)
}

fn id_map<I>(entries: &[(&String, &impl Sized)], make: impl Fn(u8) -> I) -> HashMap<String, I> {
    entries.iter().enumerate().map(|(i, (name, _))| ((*name).clone(), make(i as u8))).collect()
}

/// Checks that every name parses to an enum value whose discriminant equals the game's `idx`,
/// and that no enum value is missing from the data.
fn check_tags<T: DeserializeOwned + Flag>(
    kind: &'static str,
    entries: impl Iterator<Item = (String, u32)>,
    expected_count: usize,
) -> Result<(), RulesError> {
    let mut count = 0;
    for (name, idx) in entries {
        let tag: Option<T> = serde_json::from_value(serde_json::Value::String(name.clone())).ok();
        match tag {
            Some(t) if t.bit() == idx => count += 1,
            _ => return Err(RulesError::TagMismatch { kind, name, idx }),
        }
    }
    if count != expected_count {
        return Err(RulesError::MissingTag { kind, name: "<enum value>" });
    }
    Ok(())
}

fn compile(raw: raw::RawRules) -> Result<Rules, RulesError> {
    check_tags::<Terrain>(
        "terrain",
        raw.terrain_data.iter().map(|(n, v)| (n.clone(), v.idx)),
        9,
    )?;
    check_tags::<Resource>(
        "resource",
        raw.resource_data.iter().map(|(n, v)| (n.clone(), v.idx)),
        10,
    )?;

    let tribes_o = ordered("tribe", &raw.tribe_data, |v| v.idx)?;
    let techs_o = ordered("tech", &raw.tech_data, |v| v.idx)?;
    let units_o = ordered("unit", &raw.unit_data, |v| v.idx)?;
    let imps_o = ordered("improvement", &raw.improvement_data, |v| v.idx)?;
    let tasks_o = ordered("task", &raw.task_data, |v| v.idx)?;

    let tribe_ids = id_map(&tribes_o, TribeId);
    let tech_ids = id_map(&techs_o, TechId);
    let unit_ids = id_map(&units_o, UnitId);
    let improvement_ids = id_map(&imps_o, ImprovementId);
    let task_ids = id_map(&tasks_o, TaskId);

    let mut r = Resolver { warnings: Vec::new() };

    let tribes = tribes_o
        .iter()
        .map(|(name, t)| {
            let starting_unit = r
                .look(&unit_ids, "unit", name, &t.starting_unit)
                .unwrap_or(UnitId(0));
            TribeData {
                name: (*name).clone(),
                game_idx: t.idx,
                color: t.color,
                climate: t.climate,
                bonus: t.bonus,
                category: t.category,
                price_tier: t.price_tier,
                starting_unit,
                starting_techs: r.look_all(&tech_ids, "tech", name, &t.starting_tech),
                starting_resources: t.starting_resource.clone(),
                starting_stars: t.starting_stars,
                abilities: t.tribe_abilities.iter().copied().collect(),
                terrain_modifier: t.terrain_modifier.iter().map(|(k, v)| (*k, *v)).collect(),
                resource_modifier: t.resource_modifier.iter().map(|(k, v)| (*k, *v)).collect(),
                unit_overrides: r.look_pairs(&unit_ids, "unit", name, &t.unit_overrides),
                tech_overrides: r.look_pairs(&tech_ids, "tech", name, &t.tech_overrides),
                improvement_overrides: r.look_pairs(
                    &improvement_ids,
                    "improvement",
                    name,
                    &t.improvement_overrides,
                ),
                language: t.language.clone(),
            }
        })
        .collect();

    let techs = techs_o
        .iter()
        .map(|(name, t)| TechData {
            name: (*name).clone(),
            game_idx: t.idx,
            cost: t.cost,
            tech_unlocks: r.look_all(&tech_ids, "tech", name, &t.tech_unlocks),
            unit_unlocks: r.look_all(&unit_ids, "unit", name, &t.unit_unlocks),
            improvement_unlocks: r.look_all(
                &improvement_ids,
                "improvement",
                name,
                &t.improvement_unlocks,
            ),
            task_unlocks: r.look_all(&task_ids, "task", name, &t.task_unlocks),
            ability_unlocks: t.ability_unlocks.clone(),
            movement_unlocks: t.movement_unlocks.iter().map(|(k, v)| (*k, *v)).collect(),
            defence_bonus_unlocks: t.defence_bonus_unlocks.iter().map(|(k, v)| (*k, *v)).collect(),
        })
        .collect();

    let units = units_o
        .iter()
        .map(|(name, u)| UnitData {
            name: (*name).clone(),
            game_idx: u.idx,
            hidden: u.hidden,
            cost: u.cost,
            health: u.health,
            defence: u.defence,
            movement: u.movement,
            weapon: u.weapon,
            range: u.range,
            attack: u.attack,
            promotion_limit: u.promotion_limit,
            growth_rate: u.growth_rate,
            upgrades_from: u
                .upgrades_from
                .as_deref()
                .and_then(|n| r.look(&unit_ids, "unit", name, n)),
            movement_terrain: (!u.movement_terrain.is_empty())
                .then(|| u.movement_terrain.iter().copied().collect()),
            abilities: u.unit_abilities.iter().copied().collect(),
        })
        .collect();

    let improvements = imps_o
        .iter()
        .map(|(name, i)| {
            let adjacency_improvements = i
                .adjacency_improvements
                .iter()
                .map(|a| AdjacencyImprovement {
                    improvement: a
                        .improvement
                        .as_deref()
                        .and_then(|n| r.look(&improvement_ids, "improvement", name, n)),
                    resource: a.resources,
                })
                .collect();
            let adjacency_requirements = i
                .adjacency_requirements
                .iter()
                .map(|a| AdjacencyRequirement {
                    improvement: a
                        .improvement
                        .as_deref()
                        .and_then(|n| r.look(&improvement_ids, "improvement", name, n)),
                    terrain: a.terrain,
                })
                .collect();
            let creates = i
                .creates
                .iter()
                .map(|c| Creates {
                    terrain: c.terrain,
                    resource: c.resource,
                    unit: c.unit.as_deref().and_then(|n| r.look(&unit_ids, "unit", name, n)),
                    effect: c.effect,
                })
                .collect();
            let reward = |x: &raw::RawReward| Reward {
                population: x.population,
                currency: x.currency,
                score: x.score,
            };
            ImprovementData {
                name: (*name).clone(),
                game_idx: i.idx,
                hidden: i.hidden,
                cost: i.cost,
                max_level: i.max_level,
                work: i.work,
                border_size: i.border_size,
                range: i.range,
                growth_rate: i.growth_rate,
                terrain_requirements: i
                    .terrain_requirements
                    .iter()
                    .map(|t| TerrainReq { terrain: t.terrain, resource: t.resource })
                    .collect(),
                abilities: i.improvement_abilities.iter().copied().collect(),
                rewards: i.rewards.iter().map(reward).collect(),
                growth_rewards: i.growth_rewards.iter().map(reward).collect(),
                adjacency_improvements,
                adjacency_requirements,
                routes: i.routes.iter().copied().collect(),
                creates,
            }
        })
        .collect();

    let tasks = tasks_o
        .iter()
        .map(|(name, t)| TaskData {
            name: (*name).clone(),
            game_idx: t.idx,
            improvement_unlocks: r.look_all(
                &improvement_ids,
                "improvement",
                name,
                &t.improvement_unlocks,
            ),
        })
        .collect();

    let mut resource_terrains = vec![FlagSet::empty(); 10];
    for (name, res) in &raw.resource_data {
        let tag: Resource = serde_json::from_value(serde_json::Value::String(name.clone()))?;
        resource_terrains[tag as usize] = res.resource_terrain_requirements.iter().copied().collect();
    }

    let d = &raw.diplomacy_data;
    Ok(Rules {
        tribes,
        techs,
        units,
        improvements,
        tasks,
        tribe_ids,
        tech_ids,
        unit_ids,
        improvement_ids,
        task_ids,
        resource_terrains,
        diplomacy: DiplomacyData {
            embassy_cost: d.embassy_cost,
            embassy_income: d.embassy_income,
            embassy_growth_time: d.embassy_growth_time,
            embassy_max_level: d.embassy_max_level,
            embassy_tech_price_multiplier: d.embassy_tech_price_multiplier,
            embassy_upgrade_cost: d.embassy_upgrade_cost,
            dividend_income: d.dividend_income,
        },
        warnings: r.warnings,
    })
}

#[cfg(test)]
mod tests;
