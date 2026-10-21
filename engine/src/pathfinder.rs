//! The game's A* pathfinder (`PathFinder.GetPath`, `Step`, `ValidNeighbors`, `IsTileAccessible`,
//! `TileData.GetMovementCost`, `IsAllowedToFinishOnTile`, `ReconstructPath`) for ordinary land units.
//!
//! Costs are scaled by 10 (one move = 10, a road step = 5, entering a forest/mountain = 1000, which ends the search
//! beyond it). Ties are broken exactly like the game's `MinHeap`, a sorted linked list in which a new node goes after
//! every node with an equal or lower priority. See docs/rules/pathfinder.md.
//!
//! Movers with special abilities (flying, swimming, boats, ...) and water tiles are not implemented and stop with
//! an error rather than returning a path that might differ from the game's.

use crate::engine::EngineError;
use crate::rules::{Rules, Terrain, UnitAbility};
use crate::state::{Coords, GameState, PlayerState, TileData, UnitState};

const UNIT_EFFECT_INVISIBLE: u16 = 3;
const IMPROVEMENT_CITY: u16 = 1;
const IMPROVEMENT_BRIDGE: u16 = 48;
/// A step the unit can take only as the last one of its move.
const BLOCKED_COST: i32 = 1000;
const MAX_ITERATIONS: u32 = 1000;

fn unimplemented(what: &'static str) -> EngineError {
    EngineError::Unimplemented { command: what }
}

struct Ctx<'a> {
    state: &'a GameState,
    rules: &'a Rules,
    /// `None` for the route builder (`CreateRouterSettings`): no unit, units on tiles are ignored and foreign borders block.
    unit: Option<&'a UnitState>,
    player: &'a PlayerState,
    allowed_terrain: Vec<u16>,
    /// The settings' "ignore units" flag: tiles holding units neither block nor stop the path.
    ignore_units: bool,
}

impl Ctx<'_> {
    fn has_ability(&self, a: UnitAbility) -> bool {
        self.unit.and_then(|u| self.rules.unit_by_game_idx(u.kind as u32)).map(|u| u.abilities.contains(a)).unwrap_or(false)
    }

    /// `PlayerDiplomacyExtensions.HasPeaceWith`: the relation state is 1.
    fn has_peace_with(&self, other: u8) -> bool {
        self.player.relations.iter().any(|(id, r)| *id == other && r.state == 1)
    }

    fn relation_state(&self, other: u8) -> u8 {
        self.player.relations.iter().find(|(id, _)| *id == other).map(|(_, r)| r.state).unwrap_or(0)
    }

    fn tile(&self, c: Coords) -> Option<&TileData> {
        self.state.map.tile(c)
    }

    /// `MapDataExtensions.GetTileNeighbors`: the in-map tiles of the 3x3 square around `c` without the centre, x outer, y inner.
    fn neighbors(&self, c: Coords) -> Vec<&TileData> {
        let mut out = Vec::with_capacity(8);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                if let Some(t) = self.tile(Coords { x: c.x + dx, y: c.y + dy }) {
                    out.push(t);
                }
            }
        }
        out
    }

    fn is_water(t: &TileData) -> bool {
        t.terrain == Terrain::Water as u16 || t.terrain == Terrain::Ocean as u16
    }

    /// `PathFinder.IsAllowedToFinishOnTile`.
    fn can_finish_on(&self, t: &TileData) -> Result<bool, EngineError> {
        if self.unit.is_some() && !self.ignore_units && t.unit.is_some() {
            return Ok(false);
        }
        let is_city = t.improvement.as_ref().map(|i| i.kind == IMPROVEMENT_CITY).unwrap_or(false);
        if !is_city {
            return Ok(true);
        }
        if self.has_ability(UnitAbility::Infiltrate) {
            return Err(unimplemented("pathfinding for an infiltrating unit"));
        }
        if t.owner != 0 {
            let s = self.relation_state(t.owner);
            if s == 1 || s == 3 {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `PathFinder.IsTileAccessible(tile, origin, settings)` for a walking unit.
    fn accessible(&self, t: &TileData, origin: &TileData) -> Result<bool, EngineError> {
        // Settings without the "allow foreign borders" flag (the route builder): a tile owned by someone else blocks,
        // unless we are at peace with them.
        if self.unit.is_none() && t.owner != 0 && t.owner != self.player.id && !self.has_peace_with(t.owner) {
            return Ok(false);
        }
        if !t.explorers.contains(&self.player.id) {
            return Ok(false);
        }
        let units_matter = self.unit.is_some() && !self.ignore_units;
        if let Some(u) = t.unit.as_ref().filter(|_| units_matter) {
            if u.effects.contains(&UNIT_EFFECT_INVISIBLE) && u.owner != self.player.id {
                return Err(unimplemented("pathfinding past an invisible unit"));
            }
            if u.owner != self.player.id {
                let friendly = u.owner != 0 && self.has_peace_with(u.owner);
                if !friendly && !self.has_ability(UnitAbility::Hide) {
                    return Ok(false);
                }
            }
        }
        if let Some(imp) = t.improvement.as_ref() {
            if imp.kind == IMPROVEMENT_BRIDGE {
                return Err(unimplemented("pathfinding over a bridge"));
            }
        }
        if Self::is_water(t) {
            let origin_land = !Self::is_water(origin) && origin.terrain != Terrain::Wetland as u16;
            if self.unit.is_none() && !origin_land {
                // Route builder from a water / wetland tile: a city, or any terrain the route may cross.
                return Ok(self.allowed_terrain.contains(&t.terrain));
            }
            // A land unit (or a route leaving land) can only enter water through an embarkation improvement (port);
            // swimmers (settings flag off) use the ordinary terrain test below.
            let walking = self.unit.map(|_| !self.has_ability(UnitAbility::Swim)).unwrap_or(true);
            if !walking {
                return Ok(self.allowed_terrain.contains(&t.terrain));
            }
            let embark = t
                .improvement
                .as_ref()
                .and_then(|i| self.rules.improvement_by_game_idx(i.kind as u32))
                .map(|d| d.abilities.contains(crate::rules::ImprovementAbility::Embark))
                .unwrap_or(false);
            if embark {
                // IsTileAccessible (version > 98): friendly territory (own, or at peace) and a tech that unlocks an embarkation
                // improvement (ActionUtils.CanPlayerEmbark); a carried / carrying unit cannot board. Then the usual terrain test.
                let friendly = t.owner == self.player.id || (t.owner != 0 && self.has_peace_with(t.owner));
                let can_embark = self
                    .player
                    .available_tech
                    .iter()
                    .filter_map(|tech| self.rules.tech_by_game_idx(*tech as u32))
                    .flat_map(|tech| tech.improvement_unlocks.iter())
                    .any(|i| self.rules.improvement(*i).abilities.contains(crate::rules::ImprovementAbility::Embark));
                if self.state.version <= 98 {
                    return Err(unimplemented("embarking before game version 99"));
                }
                if friendly && can_embark {
                    if let Some(u) = self.unit {
                        if u.follower != 0 || (self.state.version >= 120 && u.leader != 0) {
                            return Ok(false);
                        }
                    }
                    return Ok(self.allowed_terrain.contains(&t.terrain));
                }
            }
            return Ok(false);
        }
        if t.improvement.as_ref().map(|i| i.kind == IMPROVEMENT_CITY).unwrap_or(false) {
            return Ok(true);
        }
        Ok(self.allowed_terrain.contains(&t.terrain))
    }

    fn valid_neighbors(&self, at: Coords) -> Result<Vec<&TileData>, EngineError> {
        let origin = self.tile(at).ok_or_else(|| EngineError::Inconsistent { what: format!("no tile at {at}") })?;
        let mut out = Vec::new();
        for n in self.neighbors(at) {
            if self.accessible(n, origin)? {
                out.push(n);
            }
        }
        Ok(out)
    }

    /// `TileData.HasRoadTo`: both tiles carry a road (or are a city / bridge) and neither belongs to someone we are not at peace with.
    fn has_road_to(&self, a: &TileData, b: &TileData) -> bool {
        let road = |t: &TileData| t.has_road || t.improvement.as_ref().map(|i| i.kind == IMPROVEMENT_CITY || i.kind == IMPROVEMENT_BRIDGE).unwrap_or(false);
        let ok = |t: &TileData| t.owner == 0 || t.owner == self.player.id || self.has_peace_with(t.owner);
        road(a) && road(b) && ok(a) && ok(b)
    }

    /// `TileData.GetMovementCost` (game version > 114).
    fn movement_cost(&self, dest: &TileData, origin: &TileData) -> Result<i32, EngineError> {
        if self.unit.is_none() {
            // Route builder: existing roads, cities, bridges and routes are cheap.
            let cheap = dest.has_road
                || dest.has_route
                || dest.improvement.as_ref().map(|i| i.kind == IMPROVEMENT_CITY || i.kind == IMPROVEMENT_BRIDGE).unwrap_or(false);
            return Ok(if cheap { 5 } else { 10 });
        }
        // Zone of control: an enemy (not at peace, visible, not carried) unit next to the destination stops the unit.
        let invisible = self.unit.map(|u| u.effects.contains(&UNIT_EFFECT_INVISIBLE)).unwrap_or(false);
        if !self.has_ability(UnitAbility::Sneak) && !self.has_ability(UnitAbility::Hide) && !invisible {
            for n in self.neighbors(dest.coordinates) {
                if let Some(u) = n.unit.as_ref() {
                    if u.owner != self.player.id && !self.has_peace_with(u.owner) && !u.effects.contains(&UNIT_EFFECT_INVISIBLE) && u.leader == 0 {
                        return Ok(BLOCKED_COST);
                    }
                }
            }
        }
        if dest.terrain == Terrain::Ice as u16 {
            let glide = self
                .player
                .available_tech
                .iter()
                .filter_map(|t| self.rules.tech_by_game_idx(*t as u32))
                .any(|t| t.ability_unlocks.iter().any(|a| a == "glide"));
            if glide {
                return Ok(9);
            }
        }
        if self.has_road_to(dest, origin) && dest.terrain != Terrain::Ice as u16 {
            return Ok(5);
        }
        if dest.terrain == Terrain::Mountain as u16 || dest.terrain == Terrain::Forest as u16 {
            return Ok(BLOCKED_COST);
        }
        if dest.effects.contains(&4) && self.has_ability(UnitAbility::Land) {
            return Ok(BLOCKED_COST);
        }
        Ok(10)
    }
}

/// Abilities whose pathfinding rules are not implemented.
const SPECIAL_MOVERS: [UnitAbility; 7] = [
    UnitAbility::Fly,
    UnitAbility::Amphibious,
    UnitAbility::Skate,
    UnitAbility::Creep,
    UnitAbility::Algae,
    UnitAbility::Tentacles,
    UnitAbility::Persist,
];

/// `PathFinder.GetPath(state, start, destination, maxCost, unit)`: the path from `destination` back to `start`
/// (destination first), `None` if there is none.
pub(crate) fn get_path(
    state: &GameState,
    rules: &Rules,
    unit: &UnitState,
    start: Coords,
    dest: Coords,
    max_cost: i32,
    ignore_units: bool,
) -> Result<Option<Vec<Coords>>, EngineError> {
    let player = state
        .players
        .iter()
        .find(|p| p.id == unit.owner)
        .ok_or_else(|| EngineError::Inconsistent { what: format!("unit owner {}", unit.owner) })?;
    let mut allowed_terrain = Vec::new();
    for tech in &player.available_tech {
        if let Some(t) = rules.tech_by_game_idx(*tech as u32) {
            for (terrain, _) in &t.movement_unlocks {
                allowed_terrain.push(*terrain as u16);
            }
        }
    }
    let ctx = Ctx { state, rules, unit: Some(unit), player, allowed_terrain: Vec::new(), ignore_units };
    // UnitDataExtensions.GetAllowedTerrain: a swimmer may also enter shallow water.
    if ctx.has_ability(UnitAbility::Swim) {
        allowed_terrain.push(Terrain::Water as u16);
    }
    let ctx = Ctx { allowed_terrain, ..ctx };
    if SPECIAL_MOVERS.iter().any(|a| ctx.has_ability(*a)) {
        return Err(unimplemented("pathfinding for a flying / swimming / boat / special unit"));
    }
    search(&ctx, start, dest, max_cost)
}

/// `PathFinder.GetPath(map, start, destination, maxCost, CreateRouterSettings(...))` as used by `MapDataExtensions.UpdateRoutes`:
/// a path for a route between two route openers over `route_terrain`, from `player`'s point of view.
pub(crate) fn get_router_path(
    state: &GameState,
    rules: &Rules,
    player: &PlayerState,
    start: Coords,
    dest: Coords,
    max_cost: i32,
    route_terrain: Vec<u16>,
) -> Result<Option<Vec<Coords>>, EngineError> {
    let ctx = Ctx { state, rules, unit: None, player, allowed_terrain: route_terrain, ignore_units: true };
    search(&ctx, start, dest, max_cost)
}

fn search(ctx: &Ctx, start: Coords, dest: Coords, max_cost: i32) -> Result<Option<Vec<Coords>>, EngineError> {
    let state = ctx.state;
    if start == dest {
        return Ok(Some(vec![start]));
    }
    let Some(dest_tile) = ctx.tile(dest) else { return Ok(None) };
    if !ctx.can_finish_on(dest_tile)? {
        return Ok(None);
    }

    let n = state.map.tiles.len();
    let index = |c: Coords| state.map.index(c).expect("coordinates on the map");
    let mut came_from = vec![Coords { x: 0, y: 0 }; n];
    let mut cost_so_far = vec![0i32; n];
    // MinHeap: sorted ascending, a new node goes after every node with priority <= its own.
    let mut open: Vec<(f32, Coords)> = vec![((start.x - dest.x).abs() as f32 + (start.y - dest.y).abs() as f32, start)];
    let multiplied_max_cost = (max_cost * 2 * 5) as f32;

    let mut iterations = MAX_ITERATIONS;
    while !open.is_empty() {
        let (_, current) = open.remove(0);
        if current == dest {
            // ReconstructPath: dest, ..., start.
            let mut path = vec![dest];
            let mut cur = dest;
            for _ in 0..n + 1 {
                cur = came_from[index(cur)];
                path.push(cur);
                if cur == start {
                    return Ok(Some(path));
                }
            }
            return Err(EngineError::Inconsistent { what: "path reconstruction did not reach the start".to_string() });
        }
        // PathFinder.Step
        let current_cost = cost_so_far[index(current)];
        if (current_cost as f32) < multiplied_max_cost {
            let origin = ctx.tile(current).expect("current tile");
            for nb in ctx.valid_neighbors(current)? {
                let new_cost = ctx.movement_cost(nb, origin)? + current_cost;
                let ni = index(nb.coordinates);
                if cost_so_far[ni] <= 0 || new_cost < cost_so_far[ni] {
                    cost_so_far[ni] = new_cost;
                    came_from[ni] = current;
                    let (dx, dy) = ((nb.coordinates.x - dest.x) as i64, (nb.coordinates.y - dest.y) as i64);
                    let distance = ((dx * dx + dy * dy) as f64).sqrt() as f32;
                    let priority = distance + new_cost as f32;
                    let at = open.partition_point(|(p, _)| *p <= priority);
                    open.insert(at, (priority, nb.coordinates));
                }
            }
        }
        if iterations <= 1 {
            break;
        }
        iterations -= 1;
    }
    Ok(None)
}
