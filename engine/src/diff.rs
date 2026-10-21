//! Field-level comparison of two game states, for replay tests and debugging.

use std::collections::BTreeMap;
use std::fmt;

use crate::state::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Diff {
    /// Coarse grouping for summaries, e.g. `tile.unit`, `player.currency`, `header.turn`.
    pub category: &'static str,
    /// Where, e.g. `tile(3,5).unit` or `player[2].currency`.
    pub path: String,
    pub expected: String,
    pub actual: String,
}

fn clip(s: &str) -> String {
    const MAX: usize = 220;
    if s.len() <= MAX {
        s.to_string()
    } else {
        let mut end = MAX;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

impl fmt::Display for Diff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: expected {}, got {}", self.path, clip(&self.expected), clip(&self.actual))
    }
}

macro_rules! cmp {
    ($out:expr, $cat:expr, $path:expr, $a:expr, $b:expr) => {
        if $a != $b {
            $out.push(Diff {
                category: $cat,
                path: $path,
                expected: format!("{:?}", $a),
                actual: format!("{:?}", $b),
            });
        }
    };
}

/// All differences between `expected` and `actual`, header first, then map, players, commands.
pub fn diff_game_states(expected: &GameState, actual: &GameState) -> Vec<Diff> {
    let mut out = Vec::new();
    cmp!(out, "header.version", "version".to_string(), expected.version, actual.version);
    cmp!(out, "header.current_command", "current_command".to_string(), expected.current_command, actual.current_command);
    cmp!(out, "header.turn", "current_turn".to_string(), expected.current_turn, actual.current_turn);
    cmp!(out, "header.player", "current_player_index".to_string(), expected.current_player_index, actual.current_player_index);
    cmp!(out, "header.unit_id", "current_unit_id".to_string(), expected.current_unit_id, actual.current_unit_id);
    cmp!(out, "header.state", "current_state".to_string(), expected.current_state, actual.current_state);
    cmp!(out, "header.seed", "seed".to_string(), expected.seed, actual.seed);
    cmp!(out, "settings", "settings".to_string(), expected.settings, actual.settings);
    cmp!(out, "header.routes_flag", "has_flagged_need_for_update_routes".to_string(), expected.has_flagged_need_for_update_routes, actual.has_flagged_need_for_update_routes);
    cmp!(out, "header.village_seed", "village_name_seed".to_string(), expected.village_name_seed, actual.village_name_seed);
    diff_maps(&expected.map, &actual.map, &mut out);
    diff_players(&expected.players, &actual.players, &mut out);
    cmp!(out, "triggers", "triggers".to_string(), expected.triggers, actual.triggers);
    cmp!(out, "commands", "commands".to_string(), expected.commands, actual.commands);
    out
}

fn diff_maps(e: &MapData, a: &MapData, out: &mut Vec<Diff>) {
    if (e.width, e.height) != (a.width, a.height) || e.tiles.len() != a.tiles.len() {
        cmp!(out, "map.size", "map size".to_string(), (e.width, e.height, e.tiles.len()), (a.width, a.height, a.tiles.len()));
        return;
    }
    for (te, ta) in e.tiles.iter().zip(&a.tiles) {
        let at = |field: &str| format!("tile{}.{}", te.coordinates, field);
        cmp!(out, "tile.coordinates", at("coordinates"), te.coordinates, ta.coordinates);
        cmp!(out, "tile.terrain", at("terrain"), te.terrain, ta.terrain);
        cmp!(out, "tile.climate", at("climate"), te.climate, ta.climate);
        cmp!(out, "tile.altitude", at("altitude"), te.altitude, ta.altitude);
        cmp!(out, "tile.owner", at("owner"), te.owner, ta.owner);
        cmp!(out, "tile.capital_of", at("capital_of"), te.capital_of, ta.capital_of);
        cmp!(out, "tile.ruling_city", at("ruling_city"), te.ruling_city, ta.ruling_city);
        cmp!(out, "tile.resource", at("resource"), te.resource, ta.resource);
        cmp!(out, "tile.improvement", at("improvement"), te.improvement, ta.improvement);
        cmp!(out, "tile.unit", at("unit"), te.unit, ta.unit);
        cmp!(out, "tile.explorers", at("explorers"), te.explorers, ta.explorers);
        cmp!(out, "tile.road", at("has_road"), te.has_road, ta.has_road);
        cmp!(out, "tile.route", at("has_route"), te.has_route, ta.has_route);
        cmp!(out, "tile.skin", at("skin"), te.skin, ta.skin);
        cmp!(out, "tile.effects", at("effects"), te.effects, ta.effects);
    }
}

fn diff_players(e: &[PlayerState], a: &[PlayerState], out: &mut Vec<Diff>) {
    if e.len() != a.len() {
        cmp!(out, "player.count", "player count".to_string(), e.len(), a.len());
        return;
    }
    for (i, (pe, pa)) in e.iter().zip(a).enumerate() {
        let at = |field: &str| format!("player[{}].{}", i, field);
        cmp!(out, "player.id", at("id"), pe.id, pa.id);
        cmp!(out, "player.user_name", at("user_name"), pe.user_name, pa.user_name);
        cmp!(out, "player.tribe", at("tribe"), pe.tribe, pa.tribe);
        cmp!(out, "player.start_tile", at("start_tile"), pe.start_tile, pa.start_tile);
        cmp!(out, "player.handicap", at("handicap"), pe.handicap, pa.handicap);
        cmp!(out, "player.currency", at("currency"), pe.currency, pa.currency);
        cmp!(out, "player.score", at("score"), pe.score, pa.score);
        cmp!(out, "player.end_score", at("end_score"), pe.end_score, pa.end_score);
        cmp!(out, "player.cities", at("cities"), pe.cities, pa.cities);
        cmp!(out, "player.available_tech", at("available_tech"), pe.available_tech, pa.available_tech);
        cmp!(out, "player.known_players", at("known_players"), pe.known_players, pa.known_players);
        cmp!(out, "player.tasks", at("tasks"), pe.tasks, pa.tasks);
        cmp!(out, "player.kills", at("kills"), pe.kills, pa.kills);
        cmp!(out, "player.casualities", at("casualities"), pe.casualities, pa.casualities);
        cmp!(out, "player.wipe_outs", at("wipe_outs"), pe.wipe_outs, pa.wipe_outs);
        cmp!(out, "player.color", at("color"), pe.color, pa.color);
        cmp!(out, "player.built_unique_improvements", at("built_unique_improvements"), pe.built_unique_improvements, pa.built_unique_improvements);
        cmp!(out, "player.relations", at("relations"), pe.relations, pa.relations);
        cmp!(out, "player.messages", at("messages"), pe.messages, pa.messages);
        cmp!(out, "player.killer", at("killer_id"), pe.killer_id, pa.killer_id);
        cmp!(out, "player.killed_turn", at("killed_turn"), pe.killed_turn, pa.killed_turn);
        cmp!(out, "player.resigned", at("resigned_turn"), pe.resigned_turn, pa.resigned_turn);
        cmp!(out, "player.other", at("other"),
            (&pe.account_id, pe.auto_play, pe.has_chosen_tribe, pe.tribe_mix, pe.skin, pe.climate, pe.resigned_at_command_index, pe.wiped_at_command_index),
            (&pa.account_id, pa.auto_play, pa.has_chosen_tribe, pa.tribe_mix, pa.skin, pa.climate, pa.resigned_at_command_index, pa.wiped_at_command_index));
    }
}

/// Number of differences per category, sorted by category name.
pub fn summarize(diffs: &[Diff]) -> BTreeMap<&'static str, usize> {
    let mut m = BTreeMap::new();
    for d in diffs {
        *m.entry(d.category).or_insert(0) += 1;
    }
    m
}
