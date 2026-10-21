//! Replaying a saved game: start from its initial state, run its recorded commands with the engine,
//! and compare the result with the state the game itself saved.

use crate::diff::{diff_game_states, Diff};
use crate::engine::{Engine, EngineError, Progress};
use crate::rules::Rules;
use crate::state::*;

/// What the game's deserializer does to a state when it loads it, applied so that a freshly-played
/// state and a saved state can be compared (`PlayerState.Deserialize`, version >= 86):
/// a player whose colour is -1 (the value a new `PlayerState` starts with) gets the colour of their tribe.
/// The saved *initial* state has been through that; the saved *current* state of a game that was never
/// reloaded has not (e.g. Nature is -1).
pub fn apply_load_fixups(state: &mut GameState, rules: &Rules) {
    for p in &mut state.players {
        if p.color == -1 {
            // TODO: GetTribeColor also honours a skin colour (skinData); only the tribe colour is used here.
            if let Some(t) = rules.tribe_by_game_idx(p.tribe as u32) {
                p.color = t.color;
            }
        }
    }
}

/// Removes information that is not game state, so two states can be compared:
/// `PlayerDiplomacyExtensions.GetRelation` creates a default `DiplomacyRelation` in the player's dictionary
/// whenever it is asked about someone (and it is asked by pathfinding, the AI, the UI, ...), so an entry equal to
/// the default carries no information. Entries that differ from the default are kept.
pub fn canonicalize_for_comparison(state: &mut GameState) {
    let default = DiplomacyRelation::default_relation();
    for p in &mut state.players {
        p.relations.retain(|(_, r)| *r != default);
        // The dictionary's order only records when each entry was first asked for; it carries no game information.
        p.relations.sort_by_key(|(id, _)| *id);
    }
    // Saves older than version 123 do not store a unit's birth-climate skin; the loader fills in "none" for every unit
    // while the engine creates new units with the real value, so the field carries no information for them.
    if state.version < 123 {
        for t in &mut state.map.tiles {
            if let Some(u) = t.unit.as_mut() {
                u.birth_climate_skin = u16::MAX;
            }
        }
    }
}

#[derive(Debug)]
pub struct ReplayOutcome {
    pub total: usize,
    /// Commands fully executed before stopping.
    pub executed: usize,
    pub error: Option<EngineError>,
    /// Differences between the engine's final state and the saved one. Only filled when every command ran.
    pub diffs: Vec<Diff>,
}

impl ReplayOutcome {
    pub fn matches(&self) -> bool {
        self.error.is_none() && self.diffs.is_empty()
    }
}

/// Replay `save.current.commands` from `save.initial` and compare with `save.current`.
pub fn replay(save: &LocalGameData, rules: &Rules) -> ReplayOutcome {
    let mut start = save.initial.clone();
    start.commands = save.current.commands.clone();
    start.current_command = 0;
    let total = start.commands.len();
    let mut engine = Engine::new(start, rules);
    // Everything the real game accepted must also pass our legality checks.
    engine.validate_commands = true;
    let mut executed = 0;
    loop {
        match engine.update() {
            Ok(Progress::RanCommand) => executed += 1,
            Ok(Progress::RanAction) => {}
            Ok(Progress::Idle) => break,
            Err(e) => return ReplayOutcome { total, executed, error: Some(e), diffs: Vec::new() },
        }
    }
    let mut expected = save.current.clone();
    apply_load_fixups(&mut expected, rules);
    let mut actual = engine.state;
    canonicalize_for_comparison(&mut expected);
    canonicalize_for_comparison(&mut actual);
    let diffs = diff_game_states(&expected, &actual);
    ReplayOutcome { total, executed, error: None, diffs }
}
