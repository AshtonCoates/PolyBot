//! The game engine: commands queue actions, actions change the state.
//!
//! Execution model (from `ActionManager.Update`, see docs/rules/move-and-endturn.md):
//! each [`Engine::update`] runs the *last* queued action if there is one (checking `IsValid` first and
//! removing it by the index it had before running, so actions it queued stay above and run next);
//! otherwise it takes the next recorded command, advances `current_command` and executes it.
//! Commands are implemented one at a time, driven by the replay tests.

use std::fmt;

use crate::actions::Action;
use crate::rules::{Rules, UnitAbility};
use crate::state::*;

// Task ids (`taskData.*.idx`).
const TASK_PACIFIST: u16 = 1;
const TASK_GENIUS: u16 = 2;
const TASK_WEALTH: u16 = 4;

// `UnitEffect` ids.
const UNIT_EFFECT_FROZEN: u16 = 0;
const UNIT_EFFECT_INVISIBLE: u16 = 3;
const UNIT_EFFECT_POISONED: u16 = 1;
const UNIT_EFFECT_PETRIFIED: u16 = 5;
const UNIT_EFFECT_DOUBLE_READY: u16 = 7;
const UNIT_EFFECT_CHARMED: u16 = 8;

// Improvement ids (`improvementData.*.idx`) and the improvement effect cleared at the end of a turn.
const IMPROVEMENT_CITY: u16 = 1;
const IMPROVEMENT_LIGHTHOUSE: u16 = 47;
const NO_COORDS: Coords = Coords { x: -1, y: -1 };
const IMPROVEMENT_EFFECT_CLEARED_AT_TURN_END: u16 = 1;

/// `ScoreSheet.exploreValue`: score for each newly explored tile.
const TASK_KILLER: u16 = 5;
const TASK_NETWORK: u16 = 3;
const TASK_METROPOLIS: u16 = 6;
const TASK_EXPLORE_LIGHTHOUSES: u16 = 8;
const IMPROVEMENT_ROAD: u16 = 3;
const TRIGGER_CITY_REWARD: u16 = 1;
const CITY_REWARD_CITY_WALL: u16 = 1;
const CITY_REWARD_PARK: u16 = 2;
const CITY_REWARD_WORKSHOP: u16 = 3;
const CITY_REWARD_EXPLORER: u16 = 4;
const CITY_REWARD_BORDER_GROWTH: u16 = 5;
const CITY_REWARD_SUPER_UNIT: u16 = 6;
const CITY_REWARD_RESOURCES: u16 = 7;
const CITY_REWARD_POPULATION: u16 = 8;
const EXPLORE_VALUE: i32 = 5;

fn is_water_terrain(terrain: u16) -> bool {
    terrain == crate::rules::Terrain::Water as u16 || terrain == crate::rules::Terrain::Ocean as u16
}

fn unit_has_ability(rules: &Rules, unit_kind: u16, ability: UnitAbility) -> bool {
    rules.unit_by_game_idx(unit_kind as u32).map(|u| u.abilities.contains(ability)).unwrap_or(false)
}

/// `UnitDataExtensions.SetUnitDirection(unit, origin, target)`: the `GridDirection` (SW 0, W 1, NW 2, N 3, NE 4, E 5,
/// SE 6, S 7) of the step from `origin` to `target`, and whether the sprite is flipped (directions below 4).
/// Adjacent steps map directly: the vector for each direction is SW(-1,-1) W(-1,0) NW(-1,1) N(0,1) NE(1,1) E(1,0)
/// SE(1,-1) S(0,-1). For longer vectors the game picks the best-aligned direction (weights 1393 axis, 985 diagonal).
fn unit_direction(origin: Coords, target: Coords) -> (u8, bool) {
    const VECTORS: [(i32, i32); 8] = [(-1, -1), (-1, 0), (-1, 1), (0, 1), (1, 1), (1, 0), (1, -1), (0, -1)];
    let (dx, dy) = (target.x - origin.x, target.y - origin.y);
    let direction = if dx.abs().max(dy.abs()) < 2 {
        VECTORS.iter().position(|v| *v == (dx, dy)).map(|i| i as u8).unwrap_or(8)
    } else {
        let (mut best, mut best_score) = (0u8, 0i64);
        for (i, v) in VECTORS.iter().enumerate() {
            let weight = if v.0 * v.0 + v.1 * v.1 == 1 { 1393 } else { 985 };
            let score = (v.0 * dx + v.1 * dy) as i64 * weight;
            if score > best_score {
                (best, best_score) = (i as u8, score);
            }
        }
        best
    };
    (direction, direction < 4)
}

#[derive(Debug, Clone, PartialEq)]
pub enum EngineError {
    /// The command's rules have not been implemented yet.
    Unimplemented { command: &'static str },
    /// A command or action referred to something that does not exist (bad player id, tech, ...).
    Inconsistent { what: String },
    /// The command failed its legality check (`IsValid`).
    Invalid { command: &'static str, reason: String },
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Unimplemented { command } => write!(f, "{command} is not implemented yet"),
            EngineError::Inconsistent { what } => write!(f, "inconsistent state: {what}"),
            EngineError::Invalid { command, reason } => write!(f, "{command} is not a valid command: {reason}"),
        }
    }
}

impl std::error::Error for EngineError {}

/// What one [`Engine::update`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// Nothing left to run.
    Idle,
    RanAction,
    RanCommand,
}

pub struct Engine<'r> {
    pub state: GameState,
    pub rules: &'r Rules,
    /// Pending actions; the last one runs first.
    pub actions: Vec<Action>,
    /// When true, every command is checked with [`Engine::command_is_valid`] before it runs. The real game checks
    /// when a command is submitted, not when it is replayed; replay tests turn this on to prove our legality
    /// rules accept everything the real game accepted.
    pub validate_commands: bool,
}

impl<'r> Engine<'r> {
    pub fn new(state: GameState, rules: &'r Rules) -> Self {
        Engine { state, rules, actions: Vec::new(), validate_commands: false }
    }

    /// One step of the game loop.
    pub fn update(&mut self) -> Result<Progress, EngineError> {
        if let Some(action) = self.actions.last().cloned() {
            let index = self.actions.len() - 1;
            if self.action_is_valid(&action) {
                self.execute_action(&action)?;
            }
            self.actions.remove(index);
            return Ok(Progress::RanAction);
        }
        let Some(command) = self.state.commands.get(self.state.current_command as usize).cloned() else {
            return Ok(Progress::Idle);
        };
        self.state.current_command += 1;
        if self.validate_commands {
            self.command_is_valid(&command)?;
        }
        // CommandBase.Execute runs first in every command: it queues a RunDeferredAction, which (being at the bottom
        // of the last-in-first-out stack) runs after everything else the command queues.
        self.actions.push(Action::RunDeferred { player: command.player() });
        self.execute_command(&command)?;
        Ok(Progress::RanCommand)
    }

    /// Run until every recorded command and every queued action has been executed.
    pub fn run(&mut self) -> Result<(), EngineError> {
        while self.update()? != Progress::Idle {}
        Ok(())
    }

    // ---- commands -----------------------------------------------------------------------

    fn execute_command(&mut self, command: &Command) -> Result<(), EngineError> {
        match command {
            // StartMatchCommand: IsValid is always true; Execute only queues the action.
            Command::StartMatch { player } => {
                self.actions.push(Action::StartMatch { player: *player });
                Ok(())
            }
            Command::Move { player, from, to, unit_id } => self.move_command(*player, *from, *to, *unit_id),
            Command::Research { player, tech } => self.research_command(*player, *tech),
            Command::Train { player, kind, at } => self.train_command(*player, *kind, *at),
            Command::Build { player, kind, at } => {
                self.actions.push(Action::Build { player: *player, kind: *kind, at: *at, deduct_cost: true });
                Ok(())
            }
            Command::CityReward { player, at, reward } => self.city_reward_command(*player, *at, *reward),
            Command::Attack { player, unit_id, origin, target } => self.attack_command(*player, *unit_id, *origin, *target),
            Command::Capture { player, unit_id, at } => self.capture_command(*player, *unit_id, *at),
            Command::EndTurn { player } => self.end_turn_command(*player),
            Command::Stay { at, .. } => {
                // StayCommand.Execute: the unit uses up its turn.
                let unit = self.state.map.tile_mut(*at).and_then(|t| t.unit.as_mut()).ok_or_else(|| EngineError::Inconsistent { what: format!("Stay without a unit at {at}") })?;
                unit.moved = true;
                unit.attacked = true;
                Ok(())
            }
            Command::Recover { player, at } => {
                self.actions.push(Action::Recover { player: *player, at: *at });
                self.actions.push(Action::Heal { player: *player, at: *at, fixed: 0 });
                Ok(())
            }
            Command::Promote { player, at } => {
                // UnitData.promotesTo would queue an UpgradeAction instead; no unit type is known to have one.
                self.actions.push(Action::Promote { player: *player, at: *at });
                Ok(())
            }
            Command::ExamineRuins { player, at } => self.examine_ruins_command(*player, *at),
            Command::Upgrade { player, kind, at } => {
                // UpgradeCommand.Execute: the price is the new unit type's cost.
                let cost = self.rules.unit_by_game_idx(*kind as u32).ok_or_else(|| EngineError::Inconsistent { what: format!("unknown unit type {kind}") })?.cost;
                self.actions.push(Action::Upgrade { player: *player, kind: *kind, at: *at, cost });
                Ok(())
            }
            // Every other command is decoded but its rules are not implemented yet.
            other => Err(EngineError::Unimplemented { command: other.name() }),
        }
    }

    // ---- actions ------------------------------------------------------------------------

    /// Every action implemented so far is unconditionally valid.
    fn action_is_valid(&self, action: &Action) -> bool {
        match action {
            // MeetAction.IsValid: the two players have not met (by the time it runs the other may have been met already).
            Action::Meet { player, other, .. } => self.player_index(*player).map(|i| !self.knows_player(i, *other)).unwrap_or(false),
            // ExploreAction.IsValid: the tile must still be unexplored by the player.
            Action::Explore { player, at } => self.state.map.tile(*at).map(|t| !t.explorers.contains(player)).unwrap_or(false),
            // ImprovementLevelUpAction.IsValid: the computed level exceeds the stored one (down: is below it).
            Action::ImprovementLevelUp { at, .. } | Action::ImprovementLevelDown { at, .. } => {
                let up = matches!(action, Action::ImprovementLevelUp { .. });
                self.state
                    .map
                    .tile(*at)
                    .and_then(|t| t.improvement.as_ref().filter(|i| i.kind != 0).map(|i| (t, i)))
                    .and_then(|(t, i)| self.calculate_improvement_level(t).ok().map(|c| if up { c > i.level as i32 } else { c < i.level as i32 }))
                    .unwrap_or(false)
            }
            _ => true,
        }
    }

    /// `PlayerExtensions.KnowsPlayer`: Nature and oneself are always known.
    fn knows_player(&self, player_index: usize, other: u8) -> bool {
        let p = &self.state.players[player_index];
        other == 255 || other == p.id || p.known_players.contains(&other)
    }

    fn execute_action(&mut self, action: &Action) -> Result<(), EngineError> {
        match action {
            Action::StartMatch { .. } => self.start_match(),
            Action::EnableTask { player, kind } => self.enable_task_action(*player, *kind),
            Action::IncreaseScore { player, amount } => {
                let index = self.player_index(*player)?;
                self.add_score(index, *amount)
            }
            Action::IncreaseCurrency { player, amount } => {
                let index = self.player_index(*player)?;
                self.state.players[index].currency += *amount;
                self.check_task(index, TASK_WEALTH)
            }
            Action::RunDeferred { player } => self.run_deferred(*player),
            Action::UpdateRoutes => self.update_routes(),
            Action::UpdateCityConnections { players } => self.update_city_connections(players),
            Action::EndTurn { player } => self.end_turn_action(*player),
            Action::StartTurn { player } => self.start_turn_action(*player),
            Action::Heal { player, at, fixed } => self.heal_action(*player, *at, *fixed),
            Action::Reselect { .. } => Ok(()),
            Action::Recover { at, .. } => {
                if let Some(u) = self.state.map.tile_mut(*at).and_then(|t| t.unit.as_mut()) {
                    u.moved = true;
                    u.attacked = true;
                }
                Ok(())
            }
            Action::Promote { at, .. } => {
                let Some(u) = self.state.map.tile(*at).and_then(|t| t.unit.clone()) else { return Ok(()) };
                if u.passenger.is_some() {
                    return Err(EngineError::Unimplemented { command: "PromoteAction for a unit with a passenger" });
                }
                let rules = self.rules;
                let u2 = self.tile_mut(*at)?.unit.as_mut().expect("unit");
                u2.promotion_level += 1;
                let max = rules.unit_by_game_idx(u2.kind as u32).map(|d| d.health).unwrap_or(0) + 50 * u2.promotion_level as i32;
                u2.health = max as u16;
                Ok(())
            }
            Action::Upgrade { player, kind, at, cost } => self.upgrade_action(*player, *kind, *at, *cost),
            Action::GameOver { .. } => {
                self.state.current_state = 3;
                Ok(())
            }
            Action::WipePlayer { player, wiped } => self.wipe_player_action(*player, *wiped),
            Action::Disband { player, at, refund } => self.disband_action(*player, *at, *refund),
            Action::Embark { player, at } => self.embark_action(*player, *at),
            Action::Disembark { player, at } => self.disembark_action(*player, *at),
            Action::ExamineRuins { player, reward, at } => self.examine_ruins_action(*player, *reward, *at),
            Action::Move { player, unit_id, path } => self.move_action(*player, *unit_id, path),
            Action::Research { player, tech, cost } => {
                let player_index = self.player_index(*player)?;
                self.learn_tech(player_index, *tech, *cost, true)
            }
            Action::Train { player, kind, at, cost } => self.train_action(*player, *kind, *at, *cost),
            Action::Build { player, kind, at, deduct_cost } => self.build_action(*player, *kind, *at, *deduct_cost),
            Action::DestroyResource { player, at } => self.destroy_resource_action(*player, *at),
            Action::BuildRoad { at } => {
                self.tile_mut(*at)?.has_road = true;
                Ok(())
            }
            Action::IncreasePopulation { player, at } => self.increase_population_action(*player, *at),
            Action::CityLevelUp { player, at } => self.city_level_up_action(*player, *at),
            Action::CityReward { player, reward, at } => self.city_reward_action(*player, *reward, *at),
            Action::Meet { player, other, at } => self.meet_action(*player, *other, *at),
            Action::ImprovementLevelUp { player, at } => self.improvement_level_up_action(*player, *at),
            Action::ImprovementLevelDown { player, at, amount } => self.improvement_level_down_action(*player, *at, *amount),
            Action::Attack { player, origin, target, damage } => self.perform_attack(*player, *origin, *target, *damage),
            Action::KillUnit { player: _, at } => self.kill_unit_action(*at),
            Action::CaptureCity { player, at, old_owner } => self.capture_city_action(*player, *at, *old_owner),
            Action::DecreaseScore { player, amount } => {
                // ActionUtils.RemoveScore
                let index = self.player_index(*player)?;
                let p = &mut self.state.players[index];
                p.score = p.score.wrapping_sub(*amount as u32);
                Ok(())
            }
            Action::DestroyEmbassy { player, owner } => {
                if let Ok(index) = self.player_index(*owner) {
                    if let Some((_, r)) = self.state.players[index].relations.iter_mut().find(|(id, _)| id == player) {
                        r.embassy_level = 0;
                    }
                }
                Ok(())
            }
            Action::ConnectCity { player, at } => self.connect_city_action(*player, *at),
            Action::ChangeCityConnection { player, at } => {
                let pi = self.player_index(*player)?;
                let capital = self.state.players[pi].start_tile;
                self.actions.push(Action::IncreasePopulation { player: *player, at: capital });
                let old = self.state.map.tile(*at).and_then(|t| t.improvement.as_ref()).map(|i| i.connected_to_capital_of_player).ok_or_else(|| EngineError::Inconsistent { what: format!("connection change at {at}") })?;
                let old_capital = self.state.players[self.player_index(old)?].start_tile;
                self.actions.push(Action::DecreasePopulation { player: *player, at: old_capital });
                self.tile_mut(*at)?.improvement.as_mut().expect("city").connected_to_capital_of_player = *player;
                self.check_task(pi, TASK_NETWORK)
            }
            Action::DisconnectCity { player, at } => self.disconnect_city_action(*player, *at),
            Action::DecreasePopulation { at, .. } => self.decrease_population_action(*at),
            Action::ExpandCity { player, at } => self.expand_city_action(*player, *at),
            Action::ScoutMove { player, unit_id, remaining, seed, from, path } => self.scout_move_action(*player, *unit_id, *remaining, *seed, *from, path.clone()),
            Action::ModifyProduction { at, amount } => {
                if let Some(i) = self.tile_mut(*at)?.improvement.as_mut() {
                    i.production = i.production.wrapping_add(*amount as u16);
                }
                Ok(())
            }
            Action::ModifyScore { at, amount } => {
                if let Some(i) = self.tile_mut(*at)?.improvement.as_mut() {
                    i.base_score = i.base_score.wrapping_add(*amount as u16);
                }
                Ok(())
            }
            Action::Explore { player, at } => self.explore_action(*player, *at),
            // TaskCompletedAction.Execute is an empty method (the reward is paid by `GameState.CheckTask`); only the UI reacts.
            Action::TaskCompleted { .. } => Ok(()),
        }
    }

    // ---- legality --------------------------------------------------------------------------

    /// `Command.IsValid` for the commands whose rules are implemented; the others are assumed valid.
    ///
    /// `Err(Invalid)` means the game would reject the command; `Err(Unimplemented)` means a rule needed to decide has not
    /// been implemented yet (so the answer is unknown).
    pub fn command_is_valid(&self, command: &Command) -> Result<(), EngineError> {
        let invalid = |reason: String| EngineError::Invalid { command: command.name(), reason };
        if let Command::CityReward { player, at, .. } = command {
            // CityRewardCommand.IsValid does not use PassesBasicValidation (a trigger is pending by design): it
            // needs to be this player's turn and a city-reward trigger for exactly this city.
            if self.current_player_id() != *player {
                return Err(invalid(format!("it is not player {player}'s turn")));
            }
            let pending = self.state.triggers.iter().any(|t| t.player_id == *player && t.kind == TRIGGER_CITY_REWARD && t.coordinates == *at);
            return if pending { Ok(()) } else { Err(invalid(format!("no pending city reward at {at}"))) };
        }
        self.basic_validation(command.player()).map_err(invalid)?;
        match command {
            Command::Move { from, to, unit_id, .. } => self.move_is_valid(command, command.player(), *from, *to, *unit_id),
            Command::Research { player, tech } => {
                let index = self.player_index(*player)?;
                let known = self.state.players[index].available_tech.contains(tech);
                let price = self.tech_price(index, *tech)?;
                if known || self.state.players[index].currency < price {
                    return Err(invalid(format!("tech {tech} is known or unaffordable (price {price})")));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// `CommandBase.PassesBasicValidation`: it is this player's turn and no command trigger is pending for them.
    fn basic_validation(&self, player: u8) -> Result<(), String> {
        let current = self.state.players.get(self.state.current_player_index as usize).map(|p| p.id);
        if current != Some(player) {
            return Err(format!("it is not player {player}'s turn"));
        }
        if self.state.triggers.iter().any(|t| t.player_id == player) {
            return Err("a command trigger is pending".to_string());
        }
        Ok(())
    }

    // ---- Move ---------------------------------------------------------------------------------

    /// `MoveCommand.IsValid`: the unit stands on `from`, can move, and a path of more than one tile exists.
    fn move_is_valid(&self, command: &Command, player: u8, from: Coords, to: Coords, unit_id: u32) -> Result<(), EngineError> {
        let invalid = |reason: String| EngineError::Invalid { command: command.name(), reason };
        let unit = self
            .state
            .map
            .tile(from)
            .and_then(|t| t.unit.as_ref())
            .filter(|u| u.id == unit_id)
            .ok_or_else(|| invalid(format!("unit {unit_id} is not on {from}")))?;
        let _ = player;
        if unit.moved || unit.effects.contains(&UNIT_EFFECT_FROZEN) {
            return Err(invalid("the unit cannot move".to_string()));
        }
        match self.find_path(unit, from, to)? {
            Some(path) if path.len() > 1 => Ok(()),
            _ => Err(invalid(format!("no path from {from} to {to}"))),
        }
    }

    /// `MoveCommand.Execute`: the unit is marked as moved, a (UI-only) reselect is queued, and a `MoveAction`
    /// is queued when the destination is empty.
    fn move_command(&mut self, player: u8, from: Coords, to: Coords, unit_id: u32) -> Result<(), EngineError> {
        let unit = self.unit_on(from, unit_id)?.clone();
        let path = self.find_path(&unit, from, to)?;
        self.tile_unit_mut(from, unit_id)?.moved = true;
        self.actions.push(Action::Reselect { player, unit_id });
        let Some(path) = path.filter(|p| p.len() > 1) else {
            return Ok(()); // the game returns here without queueing a move
        };
        if self.state.map.tile(to).map(|t| t.unit.is_some()).unwrap_or(false) {
            // The destination holds a unit: the game un-sets `moved` and queues a RevealAction.
            return Err(EngineError::Unimplemented { command: "Move onto a tile that holds a unit (RevealAction)" });
        }
        self.actions.push(Action::Move { player, unit_id, path });
        if unit_has_ability(self.rules, unit.kind, UnitAbility::Hide) && !unit.effects.contains(&UNIT_EFFECT_INVISIBLE) {
            return Err(EngineError::Unimplemented { command: "Move of a unit with the Hide ability (HideAction)" });
        }
        Ok(())
    }

    /// `PathFinder.GetPath(state, from, to, movement, unit)`; the path is returned reversed (destination first).
    /// See [`crate::pathfinder`].
    fn find_path(&self, unit: &UnitState, from: Coords, to: Coords) -> Result<Option<Vec<Coords>>, EngineError> {
        crate::pathfinder::get_path(&self.state, self.rules, unit, from, to, self.unit_movement(unit), false)
    }

    /// `UnitDataExtensions.GetMovement`: the unit type's movement. (Effects such as Boosted/Swift/Poisoned add or
    /// remove movement; those are not implemented and stop with an error in [`Self::unit_movement_checked`].)
    fn unit_movement(&self, unit: &UnitState) -> i32 {
        let base = self.rules.unit_by_game_idx(unit.kind as u32).map(|u| u.movement).unwrap_or(0);
        let has = |e: u16| unit.effects.contains(&e);
        let mut m = base + has(2) as i32;
        if has(6) || has(4) {
            m += 1;
        }
        // Poisoned units lose one movement (but keep at least one).
        if self.state.version >= 115 && has(1) && m > 0 {
            m = if m >= 3 { m } else { 2 } - 1;
        }
        m
    }


    /// `MoveAction.ExecuteDefault` for an ordinary move (no special abilities or tile effects).
    fn move_action(&mut self, player: u8, unit_id: u32, path: &[Coords]) -> Result<(), EngineError> {
        let (dest, start) = (path[0], *path.last().expect("path has at least two tiles"));
        let player_index = self.player_index(player)?;
        let unit = self.unit_on(start, unit_id)?.clone();
        // Unit abilities/effects and tile effects with their own hooks (poison, bubbles, tentacles, freezing,
        // embarking, followers, ...) are not implemented: stop instead of diverging.
        if !unit.effects.is_empty() || unit.follower != 0 {
            return Err(EngineError::Unimplemented { command: "MoveAction for a unit with effects, followers or passengers" });
        }
        if let Some(a) = self.rules.unit_by_game_idx(unit.kind as u32) {
            use UnitAbility::*;
            // Escape only matters for MoveReason 1 (a move after an attack); a Move command uses reason 0.
            let hooked = [Skate, Autofreeze, Tentacles, Autoflood, Algae, Fly];
            if hooked.iter().any(|h| a.abilities.contains(*h)) {
                return Err(EngineError::Unimplemented { command: "MoveAction for a unit with movement-hook abilities" });
            }
        }
        let dest_tile = self.state.map.tile(dest).ok_or_else(|| EngineError::Inconsistent { what: format!("move to {dest}") })?;
        if !dest_tile.effects.is_empty() {
            return Err(EngineError::Unimplemented { command: "MoveAction onto a tile with effects" });
        }
        let dest_terrain = dest_tile.terrain;
        let foreign_city_owner = dest_tile.improvement.as_ref().filter(|i| i.kind == 1 && dest_tile.owner != 0 && dest_tile.owner != player).map(|_| dest_tile.owner);

        // `moved` stays set (the command set it) and `attacked` is left alone; direction faces the last step.
        let mut unit = unit;
        unit.coordinates = dest;
        if path.len() > 1 {
            let (direction, flipped) = unit_direction(path[1], path[0]);
            unit.direction = direction;
            unit.flipped = flipped;
        }
        self.state.map.tile_mut(start).expect("start tile exists").unit = None;
        self.state.map.tile_mut(dest).expect("dest tile exists").unit = Some(unit.clone());

        // Fog of war: explore around every tile of the path.
        let sight = self.sight_range(unit.kind);
        for c in path {
            self.explore_from_tile(player_index, *c, sight);
        }
        let _ = dest_terrain;
        if self.unit_data(&unit)?.abilities.contains(UnitAbility::Stomp) {
            self.area_damage(&unit, dest)?;
        }
        match self.will_unit_embark(&unit, dest)? {
            1 => self.actions.push(Action::Embark { player, at: dest }),
            2 => self.actions.push(Action::Disembark { player, at: dest }),
            _ => {}
        }
        // Stepping into somebody else's city counts as an attack on them.
        if let Some(owner) = foreign_city_owner {
            self.modify_relations_from_attack(player, owner)?;
        }
        Ok(())
    }

    /// `TileDataExtensions.HasEmbarkImprovement`: the tile's improvement lets land units board (a port).
    fn has_embark_improvement(&self, tile: &TileData) -> bool {
        match tile.improvement.as_ref() {
            None => false,
            Some(i) if self.state.version > 94 => self.rules.improvement_by_game_idx(i.kind as u32).map(|d| d.abilities.contains(crate::rules::ImprovementAbility::Embark)).unwrap_or(false),
            Some(i) => i.kind == 8,
        }
    }

    /// `ActionUtils.CanPlayerEmbark`: some technology the player has unlocks an embarkation improvement.
    fn can_player_embark(&self, player_index: usize) -> bool {
        self.state.players[player_index]
            .available_tech
            .iter()
            .filter_map(|t| self.rules.tech_by_game_idx(*t as u32))
            .flat_map(|t| t.improvement_unlocks.iter())
            .any(|i| self.rules.improvement(*i).abilities.contains(crate::rules::ImprovementAbility::Embark))
    }

    /// `ActionUtils.WillUnitEmbark`: 1 when `unit` just moved onto a port and becomes a boat, 2 when a boat lands.
    fn will_unit_embark(&self, unit: &UnitState, dest: Coords) -> Result<u8, EngineError> {
        let data = self.unit_data(unit)?;
        let walking = !data.abilities.contains(UnitAbility::Swim) && !data.abilities.contains(UnitAbility::Fly);
        let tile = self.state.map.tile(dest).ok_or_else(|| EngineError::Inconsistent { what: format!("embark at {dest}") })?;
        let carried_or_carrying = self.state.version >= 120 && (unit.follower != 0 || unit.leader != 0);
        if walking && !carried_or_carrying
            && is_water_terrain(tile.terrain) && self.has_embark_improvement(tile) {
                let player_index = self.player_index(unit.owner)?;
                if self.can_player_embark(player_index) {
                    let at_peace = self.state.players[player_index].relations.iter().any(|(id, r)| *id == tile.owner && r.state == 1);
                    if tile.owner == unit.owner || at_peace {
                        return Ok(1);
                    }
                }
            }
        if data.abilities.contains(UnitAbility::Carry) {
            if tile.improvement.as_ref().map(|i| i.kind == IMPROVEMENT_CITY).unwrap_or(false) {
                return Ok(2);
            }
            if !is_water_terrain(tile.terrain) {
                return Ok(2);
            }
        }
        Ok(0)
    }

    /// `WipePlayerAction.ExecuteDefault`: record the kill, disband every unit of the wiped player and dissolve embassies.
    fn wipe_player_action(&mut self, player: u8, wiped: u8) -> Result<(), EngineError> {
        let killer_index = self.player_index(player)?;
        self.state.players[killer_index].wipe_outs += 1;
        let turn = self.state.current_turn;
        let command_count = self.state.current_command as i32;
        let wiped_index = self.player_index(wiped)?;
        let w = &mut self.state.players[wiped_index];
        w.killed_turn = turn;
        w.killer_id = player;
        w.wiped_at_command_index = command_count - 1;
        // WipePlayerEndAction.Execute is empty, so only the actions with an effect are queued.
        let units: Vec<Coords> = self.state.map.tiles.iter().filter(|t| t.unit.as_ref().map(|u| u.owner == wiped).unwrap_or(false)).map(|t| t.coordinates).collect();
        for at in units {
            self.actions.push(Action::Disband { player: wiped, at, refund: true });
        }
        let ids: Vec<u8> = self.state.players.iter().map(|p| p.id).collect();
        for other in ids {
            self.actions.push(Action::DestroyEmbassy { player: wiped, owner: other });
            self.actions.push(Action::DestroyEmbassy { player: other, owner: wiped });
        }
        Ok(())
    }

    /// `DisbandUnitAction.ExecuteDefault` (game version > 105).
    fn disband_action(&mut self, player: u8, at: Coords, refund: bool) -> Result<(), EngineError> {
        let Some(unit) = self.state.map.tile(at).and_then(|t| t.unit.clone()) else { return Ok(()) };
        if unit.follower != 0 {
            return Err(EngineError::Unimplemented { command: "DisbandUnitAction for a unit with a follower" });
        }
        if unit.leader != 0 {
            return Err(EngineError::Unimplemented { command: "DisbandUnitAction for a carried unit" });
        }
        let full_cost = self.unit_data(&unit)?.cost + unit.passenger.as_ref().map(|p| self.unit_data(p).map(|d| d.cost).unwrap_or(0)).unwrap_or(0);
        if refund && full_cost >= 2 {
            for _ in 0..full_cost / 2 {
                self.actions.push(Action::IncreaseCurrency { player, amount: 1 });
            }
        }
        self.actions.push(Action::DecreaseScore { player: unit.owner, amount: full_cost * 5 });
        self.tile_mut(at)?.unit = None;
        if unit_has_ability(self.rules, unit.kind, UnitAbility::Tentacles) {
            return Err(EngineError::Unimplemented { command: "UpdateTentacles after disbanding a tentacle unit" });
        }
        Ok(())
    }

    /// `UpgradeAction.ExecuteDefault` (current versions): the new unit is trained on the tile and takes over the old unit's
    /// state (id, passenger, home, flags, direction, effects); damage already taken carries over; the score difference is
    /// paid; the tile's surroundings are explored with the new unit's sight.
    fn upgrade_action(&mut self, player: u8, kind: u16, at: Coords, cost: i32) -> Result<(), EngineError> {
        let player_index = self.player_index(player)?;
        let old = self.state.map.tile(at).and_then(|t| t.unit.clone()).ok_or_else(|| EngineError::Inconsistent { what: format!("upgrade without a unit at {at}") })?;
        let data = self.rules.unit_by_game_idx(kind as u32).ok_or_else(|| EngineError::Inconsistent { what: format!("unknown unit type {kind}") })?;
        if old.follower != 0 || old.leader != 0 {
            return Err(EngineError::Unimplemented { command: "Upgrade of a carried or carrying unit" });
        }
        self.state.players[player_index].currency -= cost;
        let carry = data.abilities.contains(UnitAbility::Carry);
        // ActionUtils.TrainUnit: a vehicle also trains its (throw-away) passenger, which costs a second unit id.
        self.state.current_unit_id += if carry { 2 } else { 1 };
        let mut health = data.health;
        if carry {
            health = old.health as i32;
        } else {
            let taken = self.max_health(&old) - old.health as i32;
            health -= taken.min(health - 1);
        }
        let new = UnitState {
            id: old.id,
            owner: player,
            kind,
            follower: old.follower,
            leader: 0,
            coordinates: at,
            home: old.home,
            health: health as u16,
            promotion_level: 0,
            xp: 0,
            moved: old.moved,
            attacked: old.attacked,
            flipped: old.flipped,
            created_turn: self.state.current_turn as u16,
            passenger: old.passenger.clone(),
            effects: old.effects.clone(),
            birth_climate: 0,
            direction: old.direction,
            birth_climate_skin: 0,
        };
        let new_score = (data.cost + new.passenger.as_ref().map(|p| self.unit_data(p).map(|d| d.cost).unwrap_or(0)).unwrap_or(0)) * 5;
        let old_score = (self.unit_data(&old)?.cost + old.passenger.as_ref().map(|p| self.unit_data(p).map(|d| d.cost).unwrap_or(0)).unwrap_or(0)) * 5;
        self.state.map.tile_mut(at).expect("tile exists").unit = Some(new);
        self.actions.push(Action::IncreaseScore { player, amount: new_score - old_score });
        let sight = self.sight_range(kind);
        self.explore_from_tile(player_index, at, sight);
        Ok(())
    }

    /// `EmbarkAction.ExecuteDefault`: the unit becomes a boat (`TrainUnit` also creates a throw-away passenger, which
    /// costs a second unit id) that carries the original unit.
    fn embark_action(&mut self, player: u8, at: Coords) -> Result<(), EngineError> {
        let old = self.state.map.tile(at).and_then(|t| t.unit.clone()).ok_or_else(|| EngineError::Inconsistent { what: format!("embark without a unit at {at}") })?;
        let hide = self.unit_data(&old)?.abilities.contains(UnitAbility::Hide);
        let boat_kind: u16 = match old.kind {
            12 => 45,
            37 => 40,
            _ if hide => 39,
            _ => 43,
        };
        let boat_data = self.rules.unit_by_game_idx(boat_kind as u32).ok_or_else(|| EngineError::Inconsistent { what: "boat type".into() })?;
        self.state.current_unit_id += 2;
        let id = self.state.current_unit_id - 1;
        let boat = UnitState {
            id,
            owner: player,
            kind: boat_kind,
            follower: 0,
            leader: 0,
            coordinates: at,
            home: old.home,
            health: old.health,
            promotion_level: 0,
            xp: 0,
            moved: true,
            attacked: true,
            flipped: old.flipped,
            created_turn: self.state.current_turn as u16,
            passenger: Some(Box::new(old.clone())),
            effects: old.effects.clone(),
            birth_climate: 0,
            direction: old.direction,
            birth_climate_skin: 0,
        };
        self.state.map.tile_mut(at).expect("tile exists").unit = Some(boat.clone());
        // A boat with the Stomp ability (Juggernaut) damages its surroundings when it appears.
        if boat_data.abilities.contains(UnitAbility::Stomp) {
            self.area_damage(&boat, at)?;
        }
        Ok(())
    }

    /// `ActionUtils.AreaDamage` (game version > 120): every unit around `at` that is not the attacker's own or at peace
    /// with it takes half of a normal hit (a full hit from a Tentacles unit).
    fn area_damage(&mut self, attacker: &UnitState, at: Coords) -> Result<(), EngineError> {
        let player = attacker.owner;
        let player_index = self.player_index(player)?;
        if self.state.version <= 120 {
            return Err(EngineError::Unimplemented { command: "AreaDamage before game version 121" });
        }
        for c in self.area(at, 1, false) {
            let Some(defender) = self.state.map.tile(c).and_then(|t| t.unit.clone()) else { continue };
            if defender.owner == player {
                continue;
            }
            let relation = self.state.players[player_index].relations.iter().find(|(id, _)| *id == defender.owner).map(|(_, r)| r.state).unwrap_or(0);
            if relation == 1 {
                continue;
            }
            if relation != 0 {
                return Err(EngineError::Unimplemented { command: "AreaDamage against a player with a special diplomatic state" });
            }
            let (damage, _, _, visible) = self.battle_results(attacker, &defender)?;
            if visible {
                if let Some(t) = self.state.players[player_index].tasks.iter_mut().find(|t| t.kind == TASK_PACIFIST && !t.is_completed) {
                    t.counter = Some(0);
                }
                self.modify_relations_from_attack(player, defender.owner)?;
            }
            let damage = if unit_has_ability(self.rules, attacker.kind, UnitAbility::Tentacles) { damage } else { damage / 2 / 10 * 10 };
            self.actions.push(Action::Attack { player, origin: at, target: c, damage });
        }
        Ok(())
    }

    /// `DisembarkAction.ExecuteDefault`: the passenger takes the boat's place and the score of the boat is lost.
    fn disembark_action(&mut self, player: u8, at: Coords) -> Result<(), EngineError> {
        let boat = self.state.map.tile(at).and_then(|t| t.unit.clone()).ok_or_else(|| EngineError::Inconsistent { what: format!("disembark without a unit at {at}") })?;
        let Some(passenger) = boat.passenger.clone() else { return Ok(()) };
        let mut passenger = *passenger;
        if self.unit_data(&passenger)?.abilities.contains(UnitAbility::Grow) {
            return Err(EngineError::Unimplemented { command: "DisembarkAction for a unit with the Grow ability (UpgradeAction)" });
        }
        passenger.health = boat.health;
        passenger.flipped = boat.flipped;
        passenger.direction = boat.direction;
        passenger.moved = true;
        passenger.attacked = true;
        passenger.coordinates = at;
        let boat_score = (self.unit_data(&boat)?.cost + self.unit_data(&passenger)?.cost) * 5;
        let passenger_score = self.unit_data(&passenger)?.cost * 5;
        self.state.map.tile_mut(at).expect("tile exists").unit = Some(passenger);
        self.actions.push(Action::DecreaseScore { player, amount: boat_score - passenger_score });
        Ok(())
    }

    // ---- Train --------------------------------------------------------------------------------

    /// `ResearchCommand.Execute`: queue a `ResearchAction` at the current price; the action calls
    /// `ActionUtils.LearnTech` with its actions queued rather than applied inline.
    fn research_command(&mut self, player: u8, tech: u16) -> Result<(), EngineError> {
        let player_index = self.player_index(player)?;
        let cost = self.tech_price(player_index, tech)?;
        self.actions.push(Action::Research { player, tech, cost });
        Ok(())
    }

    /// `GameLogicData.GetTechPrice`: `ceil(tier * cities + 4)`, times 0.66666 with the Literacy ability.
    /// (The float constants were read from the binary: 4.0 and 0.66666.)
    fn tech_price(&self, player_index: usize, tech: u16) -> Result<i32, EngineError> {
        let data = self
            .rules
            .tech_by_game_idx(tech as u32)
            .ok_or_else(|| EngineError::Inconsistent { what: format!("unknown tech {tech}") })?;
        let player = &self.state.players[player_index];
        let mut price = (data.cost as f32 + 4.0) + ((player.cities as i32 - 1) * data.cost) as f32;
        let literacy = player
            .available_tech
            .iter()
            .filter_map(|t| self.rules.tech_by_game_idx(*t as u32))
            .any(|t| t.ability_unlocks.iter().any(|a| a == "literacy"));
        if literacy {
            price *= 0.666_66_f32;
        }
        Ok(price.ceil() as i32)
    }

    /// `TrainCommand.Execute`: queue a `TrainAction` that costs the unit type's price.
    /// (`TrainCommand.IsValid` has not been read; every Train is assumed valid.)
    fn train_command(&mut self, player: u8, kind: u16, at: Coords) -> Result<(), EngineError> {
        let cost = self
            .rules
            .unit_by_game_idx(kind as u32)
            .ok_or_else(|| EngineError::Inconsistent { what: format!("unknown unit type {kind}") })?
            .cost;
        self.actions.push(Action::Train { player, kind, at, cost });
        Ok(())
    }

    /// `TrainAction.ExecuteDefault` for an ordinary land unit trained on an empty city tile.
    fn train_action(&mut self, player: u8, kind: u16, at: Coords, cost: i32) -> Result<(), EngineError> {
        let player_index = self.player_index(player)?;
        let data = self
            .rules
            .unit_by_game_idx(kind as u32)
            .ok_or_else(|| EngineError::Inconsistent { what: format!("unknown unit type {kind}") })?;
        let tile = self.state.map.tile(at).ok_or_else(|| EngineError::Inconsistent { what: format!("train at {at}") })?;
        if tile.unit.is_some() {
            return Err(EngineError::Unimplemented { command: "Train onto an occupied tile (TrainUnitOnOccupiedSpace)" });
        }
        if data.abilities.contains(UnitAbility::Carry) || data.abilities.contains(UnitAbility::Tentacles) || tile.terrain == crate::rules::Terrain::Water as u16 {
            return Err(EngineError::Unimplemented { command: "Train of a vehicle/tentacle unit or onto water" });
        }
        let home = if tile.improvement.as_ref().map(|i| i.kind == 1).unwrap_or(false) { at } else { tile.ruling_city };
        let health = data.health as u16;
        // ScoreSheet.GetUnitScore = 5 * UnitState.GetFullCost (unit cost + passenger cost); the 5 is `lea eax,[rax+rax*4]`.
        let unit_score = data.cost * 5;

        self.state.players[player_index].currency -= cost;
        // ActionUtils.TrainUnit: UnitState.Create, id from GameState.GetNextUnitId (pre-increment), and a freshly
        // trained unit has already moved and attacked.
        self.state.current_unit_id += 1;
        let unit = UnitState {
            id: self.state.current_unit_id,
            owner: player,
            kind,
            follower: 0,
            leader: 0,
            coordinates: at,
            home,
            health,
            promotion_level: 0,
            xp: 0,
            moved: true,
            attacked: true,
            flipped: false,
            created_turn: self.state.current_turn as u16,
            passenger: None,
            effects: Vec::new(),
            birth_climate: 0,
            // UnitState.Create: faces the middle of the map (`GetDirectionFromDistanceVector(centre - position)`).
            direction: match unit_direction(at, Coords { x: self.state.map.width as i32 / 2, y: self.state.map.height as i32 / 2 }).0 {
                8 => 0, // a zero vector (the map's middle) faces direction 0
                d => d,
            },
            birth_climate_skin: 0,
        };
        self.state.map.tile_mut(at).expect("tile exists").unit = Some(unit);
        // Sub-actions are committed to the stack after the exploration actions, so the score action runs first.
        let sight = self.sight_range(kind);
        self.explore_from_tile(player_index, at, sight);
        // TODO: ActionUtils.CheckSurroundingArea (re-evaluates nearby improvement levels) is not implemented.
        self.actions.push(Action::IncreaseScore { player, amount: unit_score });
        Ok(())
    }


    // ---- Build / CityReward ----------------------------------------------------------------

    fn tile_mut(&mut self, at: Coords) -> Result<&mut TileData, EngineError> {
        self.state.map.tile_mut(at).ok_or_else(|| EngineError::Inconsistent { what: format!("no tile at {at}") })
    }

    /// `ActionBase.CommitSubActionsToStack`: the sub-actions were added in execution order, and each is pushed
    /// onto the stack last-to-first, so the first one added runs first.
    fn commit_sub_actions(&mut self, subs: Vec<Action>) {
        self.actions.extend(subs.into_iter().rev());
    }

    /// `ScoreExtensions.CalculateImprovementScore`: nothing on unowned tiles; otherwise the improvement's base
    /// score, plus for a city `(level + 1) * 50 + xp * 5` (a city in starvation is clamped at 0 by `xp * 100`).
    fn improvement_score(&self, at: Coords) -> Result<i32, EngineError> {
        let tile = self.state.map.tile(at).ok_or_else(|| EngineError::Inconsistent { what: format!("no tile at {at}") })?;
        let Some(imp) = tile.improvement.as_ref() else { return Ok(0) };
        if tile.owner == 0 || tile.owner == 255 {
            return Ok(0);
        }
        let mut score = imp.base_score as i32;
        if imp.kind == IMPROVEMENT_CITY {
            score += (imp.level as i32 + 1) * 50 + imp.xp as i32 * 5;
            if imp.xp < 0 {
                score = (score + imp.xp as i32 * 100).max(0);
            }
        }
        if self.improvement_has(imp.kind, "patina") {
            return Err(EngineError::Unimplemented { command: "ScoreExtensions.GetPatinaScore" });
        }
        Ok(score)
    }

    fn improvement_has(&self, kind: u16, ability: &str) -> bool {
        use crate::rules::ImprovementAbility as A;
        let Some(data) = self.rules.improvement_by_game_idx(kind as u32) else { return false };
        let flag = match ability {
            "patina" => A::Patina,
            "consumed" => A::Consumed,
            "discrete" => A::Discrete,
            "flood" => A::Flood,
            "manual" => A::Manual,
            "unique" => A::Unique,
            "bridge" => A::Bridge,
            "limited" => A::Limited,
            "simple" => A::Simple,
            _ => return false,
        };
        data.abilities.contains(flag)
    }

    /// `ActionUtils.CheckSurroundingArea`: every improvement in the 3x3 area that has adjacency rules is re-levelled.
    fn check_surrounding_area(&mut self, player: u8, at: Coords) -> Result<(), EngineError> {
        for c in self.area(at, 1, true) {
            let Some(imp) = self.state.map.tile(c).and_then(|t| t.improvement.as_ref()) else { continue };
            let depends = self.rules.improvement_by_game_idx(imp.kind as u32).map(|d| !d.adjacency_improvements.is_empty()).unwrap_or(false);
            if depends {
                self.update_improvement_level(player, c)?;
            }
        }
        Ok(())
    }

    /// `ActionUtils.UpdateImprovementLevel`: queue one level-up (or level-down) action per level of difference.
    fn update_improvement_level(&mut self, player: u8, at: Coords) -> Result<(), EngineError> {
        let tile = self.state.map.tile(at).ok_or_else(|| EngineError::Inconsistent { what: format!("improvement level at {at}") })?;
        let calculated = self.calculate_improvement_level(tile)?;
        let Some(imp) = tile.improvement.as_ref() else { return Ok(()) };
        let diff = calculated - imp.level as i32;
        for _ in 0..diff.max(0) {
            self.actions.push(Action::ImprovementLevelUp { player, at });
        }
        for _ in 0..(-diff).max(0) {
            self.actions.push(Action::ImprovementLevelDown { player, at, amount: 1 });
        }
        Ok(())
    }

    /// `ActionUtils.CalculateImprovementLevel` (game version > 103).
    fn calculate_improvement_level(&self, tile: &TileData) -> Result<i32, EngineError> {
        let imp = tile.improvement.as_ref().expect("improvement");
        if self.state.version <= 103 {
            return Err(EngineError::Unimplemented { command: "CalculateImprovementLevelV104" });
        }
        let data = self
            .rules
            .improvement_by_game_idx(imp.kind as u32)
            .ok_or_else(|| EngineError::Inconsistent { what: format!("unknown improvement {}", imp.kind) })?;
        if self.improvement_has(imp.kind, "patina") || imp.kind == 34 || imp.kind == 37 {
            return Err(EngineError::Unimplemented { command: "CalculateImprovementLevel for patina / ice bank / fungi" });
        }
        let mut level = if imp.kind == IMPROVEMENT_LIGHTHOUSE { tile.explorers.len() as i32 + 1 } else { imp.level as i32 };
        if !data.adjacency_improvements.is_empty() {
            let simple = self.improvement_has(imp.kind, "simple");
            let mut total = 0;
            for c in self.area(tile.coordinates, 1, true) {
                let other = self.state.map.tile(c).expect("in map");
                if other.owner != tile.owner {
                    continue;
                }
                for rule in &data.adjacency_improvements {
                    if let (Some(id), Some(oi)) = (rule.improvement, other.improvement.as_ref()) {
                        if oi.kind as u32 == self.rules.improvement(id).game_idx {
                            total += if simple { 1 } else { (oi.level as i32).max(1) };
                        }
                    }
                    if let Some(res) = rule.resource {
                        if other.improvement.is_none() && other.resource == Some(res as u16) {
                            total += 1;
                        }
                    }
                }
            }
            level = total;
        }
        if let Some(max) = data.max_level.filter(|m| *m > 0) {
            level = level.min(max);
        }
        Ok(level)
    }

    /// `ImprovementLevelUpAction.ExecuteDefault` (game version > 103): level up, apply the growth rewards, re-check
    /// neighbours.
    fn improvement_level_up_action(&mut self, player: u8, at: Coords) -> Result<(), EngineError> {
        let tile = self.tile_mut(at)?;
        let Some(imp) = tile.improvement.as_mut() else { return Ok(()) };
        imp.level += 1;
        let (kind, ruling) = (imp.kind, tile.ruling_city);
        let rewards = self.rules.improvement_by_game_idx(kind as u32).map(|d| d.growth_rewards.clone()).unwrap_or_default();
        for reward in &rewards {
            if reward.population > 0 && ruling != NO_COORDS {
                for _ in 0..reward.population {
                    self.actions.push(Action::IncreasePopulation { player, at: ruling });
                }
            }
            if reward.score > 0 {
                self.actions.push(Action::IncreaseScore { player, amount: reward.score });
            }
        }
        self.check_surrounding_area(player, at)
    }

    /// `ImprovementLevelDownAction.ExecuteDefault`: the reverse of the population growth reward.
    fn improvement_level_down_action(&mut self, player: u8, at: Coords, amount: u16) -> Result<(), EngineError> {
        let tile = self.tile_mut(at)?;
        let Some(imp) = tile.improvement.as_mut() else { return Ok(()) };
        if imp.level == 0 {
            return Ok(());
        }
        imp.level -= amount;
        let (kind, ruling) = (imp.kind, tile.ruling_city);
        let rewards = self.rules.improvement_by_game_idx(kind as u32).map(|d| d.growth_rewards.clone()).unwrap_or_default();
        for reward in &rewards {
            for _ in 0..reward.population.max(0) {
                self.actions.push(Action::DecreasePopulation { player, at: ruling });
            }
        }
        self.check_surrounding_area(player, at)
    }

    /// `BuildAction.ExecuteDefault` (game version > 105) for the improvements whose side effects have been read.
    fn build_action(&mut self, player: u8, kind: u16, at: Coords, deduct_cost: bool) -> Result<(), EngineError> {
        let player_index = self.player_index(player)?;
        let data = self
            .rules
            .improvement_by_game_idx(kind as u32)
            .ok_or_else(|| EngineError::Inconsistent { what: format!("unknown improvement {kind}") })?;
        if !data.creates.is_empty() {
            return Err(EngineError::Unimplemented { command: "Build of an improvement that creates terrain/resources/units (creates)" });
        }
        if kind == 48 || kind == 34 || kind == 52 || self.improvement_has(kind, "flood") {
            return Err(EngineError::Unimplemented { command: "Build of a bridge / ice bank / canal / flood / fill improvement" });
        }
        if self.improvement_has(kind, "patina") {
            return Err(EngineError::Unimplemented { command: "Build of a patina improvement (ActionUtils.CalculateImprovementLevel)" });
        }
        let cost = data.cost.unwrap_or(0);
        let border_size = data.border_size.unwrap_or(0) as u16;
        let reward_score: i32 = data.rewards.iter().map(|r| r.score.max(0)).sum();
        let reward_currency: i32 = data.rewards.iter().map(|r| r.currency.max(0)).sum();
        let reward_population: i32 = data.rewards.iter().map(|r| r.population.max(0)).sum();
        let route_opener = !data.routes.is_empty();
        let consumed = self.improvement_has(kind, "consumed");
        let discrete = self.improvement_has(kind, "discrete");
        let manual = self.improvement_has(kind, "manual");
        let unique = self.improvement_has(kind, "unique");
        let bridge_ability = self.improvement_has(kind, "bridge");

        let tile = self.state.map.tile(at).ok_or_else(|| EngineError::Inconsistent { what: format!("build at {at}") })?;
        let has_resource = tile.resource.is_some();
        let ruling = tile.ruling_city;
        let mut subs: Vec<Action> = Vec::new();

        if kind != IMPROVEMENT_ROAD && !discrete && !consumed {
            let turn = self.state.current_turn as u16;
            self.tile_mut(at)?.improvement = Some(ImprovementState {
                kind,
                level: 0,
                founded: turn,
                xp: 0,
                population: 0,
                production: 1,
                base_score: reward_score.clamp(0, u16::MAX as i32) as u16,
                border_size,
                upgrade: 0,
                connected_to_capital_of_player: 0,
                name: String::new(),
                founder: player,
                rewards: Vec::new(),
                effects: Vec::new(),
            });
            let score = self.improvement_score(at)?;
            if score > 0 {
                subs.push(Action::IncreaseScore { player, amount: score });
            }
        }
        if consumed && has_resource && kind != 52 {
            subs.push(Action::DestroyResource { player, at });
        }
        if kind == IMPROVEMENT_ROAD {
            subs.push(Action::BuildRoad { at });
        }
        if kind == IMPROVEMENT_ROAD || route_opener || bridge_ability {
            if kind != IMPROVEMENT_ROAD {
                subs.push(Action::UpdateRoutes);
            }
            subs.push(Action::UpdateCityConnections { players: vec![player] });
        }
        for _ in 0..reward_currency {
            subs.push(Action::IncreaseCurrency { player, amount: 1 });
        }
        if deduct_cost {
            self.state.players[player_index].currency -= cost;
        }
        if reward_population > 0 && self.state.map.tile(ruling).is_some() {
            for _ in 0..reward_population {
                subs.push(Action::IncreasePopulation { player, at: ruling });
            }
        }
        if manual {
            if let Some(u) = self.tile_mut(at)?.unit.as_mut() {
                u.moved = true;
                u.attacked = true;
                if u.effects.contains(&UNIT_EFFECT_INVISIBLE) {
                    return Err(EngineError::Unimplemented { command: "RevealAction after a manual build" });
                }
            }
        }
        self.check_surrounding_area(player, at)?;
        if unique {
            self.state.players[player_index].built_unique_improvements.push(kind as i16);
        }
        self.commit_sub_actions(subs);
        Ok(())
    }

    /// `DestroyResourceAction.Execute`.
    fn destroy_resource_action(&mut self, player: u8, at: Coords) -> Result<(), EngineError> {
        if self.state.map.tile(at).map(|t| t.resource.is_some()).unwrap_or(false) {
            self.check_surrounding_area(player, at)?;
            self.tile_mut(at)?.resource = None;
        }
        Ok(())
    }

    /// `IncreasePopulationAction.ExecuteDefault` (game version > 114): +1 population on the city, score for the
    /// change, and a level-up if the city's turn owner has enough population.
    fn increase_population_action(&mut self, player: u8, at: Coords) -> Result<(), EngineError> {
        let is_city = self.state.map.tile(at).and_then(|t| t.improvement.as_ref()).map(|i| i.kind == IMPROVEMENT_CITY).unwrap_or(false);
        if !is_city {
            return Ok(());
        }
        let before = self.improvement_score(at)?;
        let tile = self.tile_mut(at)?;
        let imp = tile.improvement.as_mut().expect("city");
        imp.xp += 1;
        imp.population += 1; // ImprovementState.AddPopulation adds to both packed shorts (+0x1A xp, +0x1C population)
        let (owner, should_level_up) = (tile.owner, imp.xp > imp.level as i16);
        let after = self.improvement_score(at)?;
        self.actions.push(Action::IncreaseScore { player, amount: after - before });
        if should_level_up && owner == self.current_player_id() {
            self.actions.push(Action::CityLevelUp { player, at });
        }
        Ok(())
    }

    /// `CityLevelUpAction.Execute` / `ActionUtils.LevelUpCity`.
    fn city_level_up_action(&mut self, _player: u8, at: Coords) -> Result<(), EngineError> {
        let Some(tile) = self.state.map.tile(at) else { return Ok(()) };
        let Some(imp) = tile.improvement.as_ref().filter(|i| i.kind == IMPROVEMENT_CITY) else { return Ok(()) };
        if imp.xp <= imp.level as i16 {
            return Ok(());
        }
        let owner = tile.owner;
        let player_index = self.player_index(owner)?;
        let imp = self.tile_mut(at)?.improvement.as_mut().expect("city");
        imp.level += 1;
        imp.xp -= imp.level as i16;
        imp.upgrade = imp.upgrade.wrapping_sub(1);
        let level = imp.level as i32;
        // ScoreExtensions.CalculateLevelUpScore = cityLevelScore - level * cityXPScore.
        self.actions.push(Action::IncreaseScore { player: owner, amount: 50 - level * 5 });
        self.state.triggers.push(CommandTrigger { player_id: owner, kind: TRIGGER_CITY_REWARD, coordinates: at, opponent_id: 0 });
        self.check_task(player_index, TASK_METROPOLIS)
    }

    /// `CityRewardCommand.Execute`: consume the pending city-reward trigger and queue the `CityRewardAction`.
    fn city_reward_command(&mut self, player: u8, at: Coords, reward: u16) -> Result<(), EngineError> {
        if let Some(i) = self.state.triggers.iter().position(|t| t.player_id == player && t.kind == TRIGGER_CITY_REWARD && t.coordinates == at) {
            self.state.triggers.remove(i);
        }
        self.actions.push(Action::CityReward { player, reward, at });
        Ok(())
    }

    /// `CityRewardAction.Execute` for the rewards whose effects have been read.
    fn city_reward_action(&mut self, player: u8, reward: u16, at: Coords) -> Result<(), EngineError> {
        if self.state.map.tile(at).and_then(|t| t.improvement.as_ref()).is_none() {
            return Ok(());
        }
        self.player_index(player)?;
        self.enable_task(player, TASK_METROPOLIS);
        self.tile_mut(at)?.improvement.as_mut().expect("improvement").rewards.push(reward);
        // CityRewardData.GetRewardProductionModifier: Workshop, and Park from game version 99.
        if reward == CITY_REWARD_WORKSHOP || reward == CITY_REWARD_PARK {
            self.actions.push(Action::ModifyProduction { at, amount: 1 });
        }
        match reward {
            CITY_REWARD_PARK => {
                self.actions.push(Action::ModifyScore { at, amount: 250 });
                self.actions.push(Action::IncreaseScore { player, amount: 250 });
            }
            CITY_REWARD_CITY_WALL | CITY_REWARD_WORKSHOP => {}
            CITY_REWARD_RESOURCES => {
                for _ in 0..5 {
                    self.actions.push(Action::IncreaseCurrency { player, amount: 1 });
                }
            }
            CITY_REWARD_POPULATION => {
                for _ in 0..3 {
                    self.actions.push(Action::IncreasePopulation { player, at });
                }
            }
            CITY_REWARD_BORDER_GROWTH => self.add_border_growth_actions(player, at)?,
            CITY_REWARD_SUPER_UNIT => {
                let player_index = self.player_index(player)?;
                let tribe = self.rules.tribe_by_game_idx(self.state.players[player_index].tribe as u32);
                let giant = self.rules.unit_id("giant").expect("giant exists");
                let kind = tribe
                    .and_then(|t| t.unit_overrides.iter().find(|(from, _)| *from == giant).map(|(_, to)| *to))
                    .unwrap_or(giant);
                let kind = self.rules.unit(kind).game_idx as u16;
                self.train_unit_on_occupied_space(player, kind, at)?;
            }
            CITY_REWARD_EXPLORER => {
                // The explorer is not a unit: a chain of ScoutMoveActions that explore tile by tile. It only takes an id.
                self.state.current_unit_id += 1;
                let unit_id = self.state.current_unit_id;
                let moves = if self.state.version < 120 { 15 } else { 12 };
                let seed = crate::scout::hash_xy(self.state.seed, at.x, at.y);
                self.actions.push(Action::ScoutMove { player, unit_id, remaining: moves, seed, from: at, path: Vec::new() });
            }
            _ => {}
        }
        Ok(())
    }

    /// `ActionUtils.TrainUnitOnOccupiedSpace`: a free unit appears on the tile; a unit standing there is pushed aside, or
    /// killed when there is nowhere to go.
    fn train_unit_on_occupied_space(&mut self, player: u8, kind: u16, at: Coords) -> Result<(), EngineError> {
        self.actions.push(Action::Train { player, kind, at, cost: 0 });
        let Some(occupant) = self.state.map.tile(at).and_then(|t| t.unit.clone()) else { return Ok(()) };
        if !self.try_push_unit(player, at)? {
            self.actions.push(Action::KillUnit { player, at });
        }
        if self.state.version >= 106 && occupant.effects.contains(&UNIT_EFFECT_INVISIBLE) {
            return Err(EngineError::Unimplemented { command: "RevealAction after pushing away an invisible unit" });
        }
        Ok(())
    }

    /// `ActionUtils.TryPushUnitDefault`: move the unit on `at` to a free neighbouring tile, trying the directions in the
    /// order 0, -1, +1, -2, +2, ... around the way it faces.
    fn try_push_unit(&mut self, player: u8, at: Coords) -> Result<bool, EngineError> {
        const VECTORS: [(i32, i32); 8] = [(-1, -1), (-1, 0), (-1, 1), (0, 1), (1, 1), (1, 0), (1, -1), (0, -1)];
        let unit = self.state.map.tile(at).and_then(|t| t.unit.clone()).ok_or_else(|| EngineError::Inconsistent { what: format!("no unit to push at {at}") })?;
        if unit.follower != 0 || unit.leader != 0 {
            return Err(EngineError::Unimplemented { command: "TryPushUnit for a carried or carrying unit" });
        }
        let mut base = unit.direction as i32;
        if unit.owner != player {
            base = (base + 4) % 8; // GridDirections.Opposite
        }
        let vehicle = self.unit_data(&unit)?.abilities.contains(UnitAbility::Carry);
        let origin_water = is_water_terrain(self.state.map.tile(at).expect("tile").terrain);
        let mut fallback: Option<Coords> = None;
        let mut r = 1;
        while r - 1 < 8 {
            let step = if (r - 1) % 2 == 0 { 2 } else { -2 };
            let offset = r / step;
            let dir = (offset + base + 8).rem_euclid(8) as usize;
            let (dx, dy) = VECTORS[dir];
            r += 1;
            let c = Coords { x: at.x + dx, y: at.y + dy };
            let Some(tile) = self.state.map.tile(c) else { continue };
            if tile.unit.is_some() {
                continue;
            }
            let leaves_water = origin_water && !is_water_terrain(tile.terrain);
            if vehicle && leaves_water && fallback.is_none() {
                fallback = Some(c);
                continue;
            }
            if let Some(path) = crate::pathfinder::get_path(&self.state, self.rules, &unit, at, c, 1, false)? {
                self.actions.push(Action::Move { player: unit.owner, unit_id: unit.id, path });
                return Ok(true);
            }
        }
        if let Some(c) = fallback {
            if let Some(path) = crate::pathfinder::get_path(&self.state, self.rules, &unit, at, c, 1, false)? {
                self.actions.push(Action::Move { player: unit.owner, unit_id: unit.id, path });
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `CityRewardAction.AddBorderGrowthActions`: the city's border grows by one; sub-actions expand the city, rebuild
    /// routes and update every other player's city connections.
    fn add_border_growth_actions(&mut self, player: u8, at: Coords) -> Result<(), EngineError> {
        let player_index = self.player_index(player)?;
        if self.state.players[player_index].tribe == 16 {
            return Err(EngineError::Unimplemented { command: "FreezeAreaAction after border growth (Polaris)" });
        }
        self.tile_mut(at)?.improvement.as_mut().expect("city").border_size += 1;
        let others: Vec<u8> = self.state.players.iter().map(|p| p.id).filter(|id| *id != 255 && *id != player).collect();
        let subs = vec![Action::ExpandCity { player, at }, Action::UpdateRoutes, Action::UpdateCityConnections { players: others }];
        self.commit_sub_actions(subs);
        Ok(())
    }

    /// `MapDataExtensions.GetArea(map, centre, radius, square, includeCentre)`: in-map coordinates, x outer, y inner.
    fn area(&self, centre: Coords, radius: i32, include_centre: bool) -> Vec<Coords> {
        let mut out = Vec::new();
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                if dx == 0 && dy == 0 && !include_centre {
                    continue;
                }
                let c = Coords { x: centre.x + dx, y: centre.y + dy };
                if self.state.map.tile(c).is_some() {
                    out.push(c);
                }
            }
        }
        out
    }

    /// `ExpandCityAction.ExecuteDefault`: claim every tile of the city's area (tiles without a ruling city or ruled by this
    /// city) that the player does not own, scoring each, then explore the area.
    fn expand_city_action(&mut self, player: u8, at: Coords) -> Result<(), EngineError> {
        let player_index = self.player_index(player)?;
        let border = self.state.map.tile(at).and_then(|t| t.improvement.as_ref()).map(|i| i.border_size as i32).unwrap_or(0);
        let city_area: Vec<Coords> = self
            .area(at, border, true)
            .into_iter()
            .filter(|c| {
                let r = self.state.map.tile(*c).expect("in map").ruling_city;
                r.x < 0 || r == at
            })
            .collect();
        let tribe_spreads = self
            .rules
            .tribe_by_game_idx(self.state.players[player_index].tribe as u32)
            .map(|t| t.abilities.contains(crate::rules::TribeAbility::Spread))
            .unwrap_or(false);
        if tribe_spreads {
            return Err(EngineError::Unimplemented { command: "ExpandCityAction for a tribe with the Spread ability" });
        }
        let mut claimed: Vec<Coords> = Vec::new();
        for c in &city_area {
            let tile = self.state.map.tile(*c).expect("in map");
            if tile.owner == player {
                continue;
            }
            // ScoreSheet.tileValue (20); the owner changes after the score is computed.
            let _ = tile;
            self.actions.push(Action::IncreaseScore { player, amount: 20 });
            claimed.push(*c);
            let tile = self.tile_mut(*c)?;
            tile.owner = player;
            tile.ruling_city = at;
        }
        self.add_population_for_tiles(player, &claimed, at);
        for c in &city_area {
            self.check_surrounding_area(player, *c)?;
        }
        for c in self.area(at, border, true) {
            if !self.state.map.tile(c).expect("in map").explorers.contains(&player) {
                self.actions.push(Action::Explore { player, at: c });
            }
        }
        Ok(())
    }

    /// `ScoutMoveAction.Execute` (game version >= 115, ordinary behaviour): pick the next tile, queue the next step, and
    /// explore around the tile (a mountain shows 2 tiles instead of 1).
    fn scout_move_action(&mut self, player: u8, unit_id: u32, remaining: u32, seed: u32, from: Coords, mut path: Vec<Coords>) -> Result<(), EngineError> {
        if path.is_empty() {
            path.push(from);
        }
        let Some(target) = crate::scout::next_target(&self.state, self.rules, player, from, &path, seed)? else {
            return Ok(()); // nothing reachable: the explorer stops
        };
        path.push(target);
        if remaining >= 2 {
            let next_seed = crate::scout::xxh32_words(self.state.seed as u32, &[seed]);
            self.actions.push(Action::ScoutMove { player, unit_id, remaining: remaining - 1, seed: next_seed, from: target, path });
        }
        let player_index = self.player_index(player)?;
        let sight = if self.state.map.tile(target).map(|t| t.terrain == crate::rules::Terrain::Mountain as u16).unwrap_or(false) { 2 } else { 1 };
        self.explore_from_tile(player_index, target, sight);
        Ok(())
    }

    // ---- Capture ------------------------------------------------------------------------------

    /// `UnitState.MakeExhausted`: the unit (and its follower chain) has moved and attacked, and loses the
    /// double-attack-ready and frozen effects.
    fn make_exhausted(&mut self, unit_id: u32) {
        let mut next = Some(unit_id);
        let mut first = true;
        while let Some(id) = next {
            let Some(u) = self.state.map.tiles.iter_mut().find_map(|t| t.unit.as_mut().filter(|u| u.id == id)) else { break };
            u.moved = true;
            u.attacked = true;
            if first {
                if let Some(i) = u.effects.iter().position(|e| *e == UNIT_EFFECT_DOUBLE_READY) {
                    u.effects.remove(i);
                }
                if let Some(i) = u.effects.iter().position(|e| *e == 2) {
                    u.effects.remove(i);
                }
                first = false;
            }
            next = Some(u.follower).filter(|f| *f != 0);
        }
    }

    /// `CaptureCommand.ExecuteDefault` (game version >= 116).
    fn capture_command(&mut self, player: u8, unit_id: u32, at: Coords) -> Result<(), EngineError> {
        let tile = self.state.map.tile(at).ok_or_else(|| EngineError::Inconsistent { what: format!("capture at {at}") })?;
        if tile.improvement.is_none() {
            return Ok(());
        }
        let old_owner = tile.owner;
        let invisible = self
            .state
            .map
            .tiles
            .iter()
            .find_map(|t| t.unit.as_ref().filter(|u| u.id == unit_id))
            .map(|u| u.effects.contains(&UNIT_EFFECT_INVISIBLE))
            .unwrap_or(false);
        self.make_exhausted(unit_id);
        self.actions.push(Action::UpdateCityConnections { players: vec![player, old_owner] });
        self.actions.push(Action::UpdateRoutes);
        self.actions.push(Action::CaptureCity { player, at, old_owner });
        if invisible {
            return Err(EngineError::Unimplemented { command: "RevealAction after capturing with an invisible unit" });
        }
        Ok(())
    }

    /// `GameState.TryGetWinner` (game version > 90): the id of the winner if the game is over. The winner is the player with
    /// the best rank (score) once ranks are recalculated.
    fn try_get_winner(&self) -> Result<Option<u8>, EngineError> {
        let rules = &self.state.settings.rules;
        if self.state.settings.rules_game_mode == 4 || rules.win_by_capital {
            return Err(EngineError::Unimplemented { command: "TryGetWinner: ranking by capitals" });
        }
        if self.state.version <= 90 {
            return Err(EngineError::Unimplemented { command: "TryGetWinner before game version 91" });
        }
        let mut ranked: Vec<&PlayerState> = self.state.players.iter().collect();
        // GameState.SortPlayersByCities (mode 2: more cities first, then score) or SortPlayersByScore (descending, stable).
        if self.state.settings.rules_game_mode == 2 {
            ranked.sort_by(|a, b| b.cities.cmp(&a.cities).then(b.score.cmp(&a.score)));
        } else {
            ranked.sort_by(|a, b| b.score.cmp(&a.score));
        }
        let winner = ranked.first().ok_or_else(|| EngineError::Inconsistent { what: "no players".into() })?;
        let alive_index = |p: &PlayerState| self.state.players.iter().position(|q| q.id == p.id).map(|i| self.player_is_alive(i)).unwrap_or(false);
        let real_alive = self.state.players.iter().filter(|p| p.id != 255 && !p.auto_play && alive_index(p)).count();
        let all_alive = self.state.players.iter().filter(|p| p.id != 255 && alive_index(p)).count();
        let game_type = self.state.settings.game_type;
        if real_alive <= 1 && (game_type == 1 || game_type == 4 || real_alive == 0) {
            return Ok(Some(winner.id));
        }
        if rules.win_by_extermination && all_alive == 1 {
            return Ok(Some(winner.id));
        }
        if rules.turn_limit > 0 {
            return Ok((self.state.current_turn >= rules.turn_limit as u32).then_some(winner.id));
        }
        if rules.score_limit > 0 {
            return Ok((winner.score >= rules.score_limit as u32).then_some(winner.id));
        }
        Ok(None)
    }

    /// `CaptureCityAction.ExecuteDefault` (game version >= 109).
    fn capture_city_action(&mut self, player: u8, at: Coords, _old_owner: u8) -> Result<(), EngineError> {
        let player_index = self.player_index(player)?;
        let tile = self.state.map.tile(at).ok_or_else(|| EngineError::Inconsistent { what: format!("capture {at}") })?;
        let Some(city) = tile.improvement.as_ref() else { return Ok(()) };
        let unnamed = city.name.is_empty();
        let tribe = self.rules.tribe_by_game_idx(self.state.players[player_index].tribe as u32);
        if tribe.map(|t| t.abilities.contains(crate::rules::TribeAbility::Spread) || t.abilities.contains(crate::rules::TribeAbility::Flooded)).unwrap_or(false) {
            return Err(EngineError::Unimplemented { command: "CaptureCityAction for a Spread / Flooded tribe" });
        }
        let relations_in_use = self.state.players.iter().any(|p| p.relations.iter().any(|(_, r)| r.embassy_level > 0));
        if relations_in_use {
            return Err(EngineError::Unimplemented { command: "CaptureCityAction with embassies (RemoveEmbassies)" });
        }
        let tile_owner = tile.owner;
        if unnamed {
            // A village taken for the first time is named in its new owner's language.
            let p = &self.state.players[player_index];
            if p.skin != 0 && p.skin != u16::MAX {
                return Err(EngineError::Unimplemented { command: "city names for a tribe skin (GetTribeLanguage)" });
            }
            let tribe = self.rules.tribe_by_game_idx(p.tribe as u32).ok_or_else(|| EngineError::Inconsistent { what: format!("tribe {}", p.tribe) })?;
            let name = crate::names::city_name(self.state.village_name_seed, at.x, at.y, &tribe.language, p.tribe != 5);
            self.tile_mut(at)?.improvement.as_mut().expect("city").name = name;
        }
        // The capturing unit's home becomes the city (unless it is independent).
        let rules = self.rules;
        if let Some(u) = self.tile_mut(at)?.unit.as_mut() {
            if !unit_has_ability(rules, u.kind, UnitAbility::Independent) {
                u.home = at;
            }
        }
        // Everyone else's units that were homed in the city lose their home.
        for t in &mut self.state.map.tiles {
            if let Some(u) = t.unit.as_mut() {
                if u.owner != player && u.home == at {
                    u.home = Coords { x: -1, y: -1 };
                }
            }
        }
        self.state.players[player_index].cities += 1;
        if let Ok(old_index) = self.player_index(tile_owner) {
            self.state.players[old_index].cities = self.state.players[old_index].cities.wrapping_sub(1);
        }

        let border = self.state.map.tile(at).and_then(|t| t.improvement.as_ref()).map(|i| i.border_size as i32).unwrap_or(0);
        let city_area: Vec<Coords> = self
            .area(at, border, true)
            .into_iter()
            .filter(|c| {
                let r = self.state.map.tile(*c).expect("in map").ruling_city;
                r.x < 0 || r == at
            })
            .collect();
        let mut subs: Vec<Action> = Vec::new();
        let mut claimed: Vec<Coords> = Vec::new();
        for c in &city_area {
            let t = self.state.map.tile(*c).expect("in map");
            if t.owner == player {
                continue;
            }
            let previous = t.owner;
            let mut score = 20; // ScoreSheet.tileValue
            if t.improvement.is_some() && *c != at {
                score += self.improvement_score(*c)?;
            }
            if previous == 0 {
                claimed.push(*c);
            }
            subs.push(Action::IncreaseScore { player, amount: score });
            if previous != 0 {
                subs.push(Action::DecreaseScore { player: previous, amount: score });
            }
            let t = self.tile_mut(*c)?;
            t.owner = player;
            t.ruling_city = at;
        }
        if self.state.map.tile(at).and_then(|t| t.improvement.as_ref()).map(|i| i.level >= 2).unwrap_or(false) {
            self.enable_task(player, TASK_METROPOLIS);
            self.check_task(player_index, TASK_METROPOLIS)?;
        }
        self.actions.extend(subs);
        self.add_population_for_tiles(player, &claimed, at);
        if let Some(winner) = self.try_get_winner()? {
            self.actions.push(Action::GameOver { player, winner });
        }
        if let Ok(old_index) = self.player_index(tile_owner) {
            if !self.player_is_alive(old_index) {
                self.actions.push(Action::WipePlayer { player, wiped: tile_owner });
            }
        }
        let captured_score = self.improvement_score(at)?;
        self.add_score(player_index, captured_score)?;
        if tile_owner != 0 {
            self.actions.push(Action::DecreaseScore { player: tile_owner, amount: captured_score });
        }
        for c in &city_area {
            self.check_surrounding_area(player, *c)?;
        }
        for c in self.area(at, border, true) {
            if !self.state.map.tile(c).expect("in map").explorers.contains(&player) {
                self.actions.push(Action::Explore { player, at: c });
            }
        }
        // CaptureCityAction.RemoveEmbassies: a captured capital loses the embassies other players have in it.
        if self.state.map.tile(at).map(|t| t.capital_of != 0).unwrap_or(false)
            && self.state.players.iter().any(|p| p.relations.iter().any(|(_, r)| r.embassy_level != 0))
        {
            return Err(EngineError::Unimplemented { command: "capturing a capital that holds embassies (CaptureCityAction.RemoveEmbassies)" });
        }
        if tile_owner != 0 {
            // PlayerDiplomacyExtensions.ModifyDiplomacyRelationsFromAttack(player, old owner)
            let turn = self.state.current_turn as i32;
            let me = &mut self.state.players[player_index];
            if !me.relations.iter().any(|(id, _)| *id == tile_owner) {
                me.relations.push((tile_owner, DiplomacyRelation::default_relation()));
            }
            let r = &mut me.relations.iter_mut().find(|(id, _)| *id == tile_owner).expect("relation").1;
            r.previous_attack_turn = r.last_attack_turn;
            r.last_attack_turn = turn;
            if let Ok(old_index) = self.player_index(tile_owner) {
                let them = &mut self.state.players[old_index];
                if !them.relations.iter().any(|(id, _)| *id == player) {
                    them.relations.push((player, DiplomacyRelation::default_relation()));
                }
                them.relations.iter_mut().find(|(id, _)| *id == player).expect("relation").1.previous_attack_turn = turn;
            }
            self.actions.push(Action::DestroyEmbassy { player, owner: tile_owner });
            self.actions.push(Action::DestroyEmbassy { player: tile_owner, owner: player });
        }
        // The city tile becomes plain land; Drain removes flooding / freezing effects.
        let t = self.tile_mut(at)?;
        t.terrain = crate::rules::Terrain::Field as u16;
        for e in [4, 1] {
            if let Some(i) = t.effects.iter().position(|x| *x == e) {
                t.effects.remove(i);
            }
        }
        if t.skin == 17 {
            if let Some(i) = t.effects.iter().position(|x| *x == 2) {
                t.effects.remove(i);
            }
        }
        Ok(())
    }

    // ---- Attack -------------------------------------------------------------------------------

    fn unit_data(&self, unit: &UnitState) -> Result<&'r crate::rules::UnitData, EngineError> {
        self.rules.unit_by_game_idx(unit.kind as u32).ok_or_else(|| EngineError::Inconsistent { what: format!("unit type {}", unit.kind) })
    }

    /// `UnitDataExtensions.GetAttack(unit, state)`: attack x 10, +50 when boosted.
    fn unit_attack(&self, unit: &UnitState) -> Result<i32, EngineError> {
        Ok(self.unit_data(unit)?.attack * 10 + if unit.effects.contains(&2) { 50 } else { 0 })
    }

    /// `UnitDataExtensions.GetDefenceBonus` (game version >= 115), as tenths: 10 normal, 15 on terrain the owner's techs
    /// protect, 15 / 40 for a fortifying unit in its own city (40 with a city wall); halved when poisoned.
    fn defence_bonus(&self, unit: &UnitState) -> Result<i32, EngineError> {
        let tile = self.state.map.tile(unit.coordinates);
        let mut bonus = 10;
        if let Some(tile) = tile {
            if unit.owner != 0 {
                if let Ok(pi) = self.player_index(unit.owner) {
                    let protected = self.state.players[pi]
                        .available_tech
                        .iter()
                        .filter_map(|t| self.rules.tech_by_game_idx(*t as u32))
                        .any(|t| t.defence_bonus_unlocks.iter().any(|(terrain, v)| *terrain as u16 == tile.terrain && *v >= 2));
                    // GetDefenceBonus returns the tech's value in tenths (15) or 1; the unit bonus is 15 or 10.
                    if protected {
                        bonus = 15;
                    }
                }
            }
            if let Some(imp) = tile.improvement.as_ref() {
                if imp.kind == IMPROVEMENT_CITY && tile.owner == unit.owner && unit_has_ability(self.rules, unit.kind, UnitAbility::Fortify) {
                    bonus = if imp.rewards.contains(&1) { 40 } else { 15 };
                }
            }
        }
        if unit.effects.contains(&1) {
            bonus >>= 1;
        }
        Ok(bonus)
    }

    /// `UnitDataExtensions.GetDefence`: defence x the defence bonus.
    fn unit_defence(&self, unit: &UnitState) -> Result<i32, EngineError> {
        Ok(self.unit_data(unit)?.defence * self.defence_bonus(unit)?)
    }

    /// `BattleHelpers.Round`: to the nearest 10, halves up.
    fn round10(x: i32) -> i32 {
        let up = (x / 10) * 10 + 10;
        if x % 10 < up - x { (x / 10) * 10 } else { up }
    }

    /// `BattleHelpers.CanDefenderRetaliate`: the defender must be able to attack the attacker's tile.
    fn can_defender_retaliate(&self, attacker: &UnitState, defender: &UnitState) -> Result<bool, EngineError> {
        use UnitAbility::*;
        let ability = |u: &UnitState, a| unit_has_ability(self.rules, u.kind, a);
        if ability(defender, Stiff) || ability(attacker, Convert) || ability(attacker, Surprise) || defender.effects.contains(&0) || ability(attacker, Freeze) {
            return Ok(false);
        }
        let range = self.unit_data(defender)?.range;
        let d = (attacker.coordinates.x - defender.coordinates.x).abs().max((attacker.coordinates.y - defender.coordinates.y).abs());
        Ok(d <= range)
    }

    /// `BattleHelpers.GetBattleResults(state, attacker, defender, true, 0)`: (damage dealt, retaliation (-1: none),
    /// the attacker should move into the tile, the defender was visible).
    fn battle_results(&self, attacker: &UnitState, defender: &UnitState) -> Result<(i32, i32, bool, bool), EngineError> {
        let max_a = self.max_health(attacker);
        let max_d = self.max_health(defender);
        if max_a < 1 || max_d < 1 {
            return Err(EngineError::Inconsistent { what: "a unit with no health in battle".to_string() });
        }
        if unit_has_ability(self.rules, attacker.kind, UnitAbility::Tentacles) {
            return Err(EngineError::Unimplemented { command: "battle of a tentacle unit" });
        }
        let attack = self.unit_attack(attacker)?;
        let attack_force = attacker.health as i32 * attack * 100 / max_a;
        let defence = self.unit_defence(defender)?;
        let defence_force = defence * defender.health as i32 * 100 / max_d;
        let total = attack_force as i64 + defence_force as i64;
        if total == 0 {
            return Err(EngineError::Unimplemented { command: "battle with no attack or defence force" });
        }
        let raw = (attack as i64 * attack_force as i64 * 450 / (total * 1000)) as i32;
        let damage = Self::round10(raw);
        let mut retaliation = -1;
        if self.can_defender_retaliate(attacker, defender)? {
            let bonus = self.defence_bonus(defender)? as i64;
            let raw = (defence_force as i64 * defence as i64 * 4500 / (total * 1000 * bonus)) as i32;
            retaliation = Self::round10(raw);
        }
        let mut move_in = false;
        if damage >= defender.health as i32 {
            retaliation = 0;
            if self.unit_data(attacker)?.range == 1 {
                let movement = self.unit_movement(attacker);
                move_in = crate::pathfinder::get_path(&self.state, self.rules, attacker, attacker.coordinates, defender.coordinates, movement, true)?.is_some();
            }
        }
        let visible = damage > 0 && !defender.effects.contains(&UNIT_EFFECT_INVISIBLE);
        Ok((damage, retaliation, move_in, visible))
    }

    /// `PlayerDiplomacyExtensions.ModifyDiplomacyRelationsFromAttack(attacker, other)`.
    fn modify_relations_from_attack(&mut self, attacker: u8, other: u8) -> Result<(), EngineError> {
        if other == 0 {
            return Ok(());
        }
        let turn = self.state.current_turn as i32;
        let ai = self.player_index(attacker)?;
        let me = &mut self.state.players[ai];
        if !me.relations.iter().any(|(id, _)| *id == other) {
            me.relations.push((other, DiplomacyRelation::default_relation()));
        }
        let r = &mut me.relations.iter_mut().find(|(id, _)| *id == other).expect("relation").1;
        r.previous_attack_turn = r.last_attack_turn;
        r.last_attack_turn = turn;
        if let Ok(oi) = self.player_index(other) {
            let them = &mut self.state.players[oi];
            if !them.relations.iter().any(|(id, _)| *id == attacker) {
                them.relations.push((attacker, DiplomacyRelation::default_relation()));
            }
            them.relations.iter_mut().find(|(id, _)| *id == attacker).expect("relation").1.previous_attack_turn = turn;
        }
        self.actions.push(Action::DestroyEmbassy { player: attacker, owner: other });
        self.actions.push(Action::DestroyEmbassy { player: other, owner: attacker });
        Ok(())
    }

    /// `AttackCommand.ExecuteDefault` (game version >= 121) for ordinary melee / ranged attacks.
    fn attack_command(&mut self, player: u8, unit_id: u32, origin: Coords, target: Coords) -> Result<(), EngineError> {
        use UnitAbility::*;
        let attacker = self.unit_on(origin, unit_id)?.clone();
        let unsupported = [Consumed, Infiltrate, Convert, Eat, Splash, Freeze, Persist, Skate, Poison, Drench, Tentacles, Surprise];
        if unsupported.iter().any(|a| unit_has_ability(self.rules, attacker.kind, *a)) {
            return Err(EngineError::Unimplemented { command: "Attack by a unit with special attack abilities (consume / infiltrate / convert / eat / splash / freeze / persist / poison / drench)" });
        }
        let defender = self
            .state
            .map
            .tile(target)
            .and_then(|t| t.unit.clone())
            .ok_or(EngineError::Unimplemented { command: "Attack onto an empty tile" })?;
        let (damage, retaliation, move_in, visible) = self.battle_results(&attacker, &defender)?;
        self.modify_relations_from_attack(player, defender.owner)?;
        // The attacker's own flags.
        let escape = unit_has_ability(self.rules, attacker.kind, Escape);
        {
            let u = self.tile_unit_mut(origin, unit_id)?;
            u.moved = !escape;
            if let Some(i) = u.effects.iter().position(|e| *e == UNIT_EFFECT_DOUBLE_READY) {
                u.effects.remove(i);
            } else {
                u.attacked = true;
            }
        }
        if move_in {
            self.actions.push(Action::Move { player, unit_id, path: vec![target, origin] });
        } else if retaliation > 0 {
            self.actions.push(Action::Attack { player, origin: target, target: origin, damage: retaliation });
        }
        self.actions.push(Action::Attack { player, origin, target, damage });
        if attacker.effects.contains(&UNIT_EFFECT_INVISIBLE) {
            return Err(EngineError::Unimplemented { command: "RevealAction after an attack by an invisible unit" });
        }
        // PacifistTask.Reset: attacking a visible target restarts the count.
        if visible {
            let pi = self.player_index(player)?;
            if let Some(t) = self.state.players[pi].tasks.iter_mut().find(|t| t.kind == TASK_PACIFIST && !t.is_completed) {
                t.counter = Some(0);
            }
        }
        Ok(())
    }

    /// `ActionUtils.PerformAttackDefault`.
    fn perform_attack(&mut self, player_param: u8, origin: Coords, target: Coords, damage: i32) -> Result<(), EngineError> {
        let attacker = self.state.map.tile(origin).and_then(|t| t.unit.clone());
        let origin_is_target = origin == target;
        if let Some(a) = attacker.as_ref() {
            let (dir, flipped) = unit_direction(origin, target);
            let u = self.tile_mut(origin)?.unit.as_mut().expect("attacker");
            u.direction = dir;
            u.flipped = flipped;
            if let Some(i) = u.effects.iter().position(|e| *e == 2) {
                u.effects.remove(i);
            }
            let _ = a;
        }
        let player = match attacker.as_ref() {
            Some(a) if !origin_is_target => a.owner,
            _ => player_param,
        };
        let player_index = self.player_index(player)?;
        let Some(defender) = self.state.map.tile(target).and_then(|t| t.unit.clone()) else {
            return self.drench_after_attack(attacker.as_ref(), target, player);
        };
        {
            let (dir, flipped) = unit_direction(target, origin);
            let d = self.tile_mut(target)?.unit.as_mut().expect("defender");
            d.direction = dir;
            d.flipped = flipped;
            for e in [2, 6] {
                if let Some(i) = d.effects.iter().position(|x| *x == e) {
                    d.effects.remove(i);
                }
            }
            if d.effects.contains(&4) {
                return Err(EngineError::Unimplemented { command: "attacking a unit in a bubble (BubbleAction)" });
            }
            d.health -= (damage.max(0) as u16).min(d.health);
        }
        let health = self.state.map.tile(target).and_then(|t| t.unit.as_ref()).map(|u| u.health).unwrap_or(0);
        if health != 0 {
            if attacker.is_some() && unit_has_ability(self.rules, attacker.as_ref().expect("attacker").kind, UnitAbility::Poison) {
                return Err(EngineError::Unimplemented { command: "PoisonUnitAction" });
            }
        } else {
            if defender.owner != 255 {
                self.state.players[player_index].kills += 1;
                if let Some(a) = attacker.as_ref() {
                    let data = self.unit_data(a)?;
                    if !origin_is_target && !data.hidden && !unit_has_ability(self.rules, a.kind, UnitAbility::Static) && !self.is_vehicle(a) {
                        self.tile_mut(origin)?.unit.as_mut().expect("attacker").xp += 1;
                    }
                }
                // Killer task: every kill counts (the first one creates the task already counting it).
                self.enable_task(player, TASK_KILLER);
                if let Some(t) = self.state.players[player_index].tasks.iter_mut().find(|t| t.kind == TASK_KILLER) {
                    if !t.is_completed {
                        t.counter = Some(t.counter.unwrap_or(0) + 1);
                    }
                }
                let reached = self.state.players[player_index].tasks.iter().any(|t| t.kind == TASK_KILLER && t.counter.unwrap_or(0) >= 10);
                if reached {
                    self.check_task(player_index, TASK_KILLER)?;
                }
            }
            if let Ok(di) = self.player_index(defender.owner) {
                self.state.players[di].casualities += 1;
            }
            self.actions.push(Action::KillUnit { player, at: target });
        }
        self.drench_after_attack(attacker.as_ref(), target, player)
    }

    fn drench_after_attack(&mut self, attacker: Option<&UnitState>, _target: Coords, _player: u8) -> Result<(), EngineError> {
        if attacker.map(|a| unit_has_ability(self.rules, a.kind, UnitAbility::Drench)).unwrap_or(false) {
            return Err(EngineError::Unimplemented { command: "FloodTileAction after a drench attack" });
        }
        Ok(())
    }

    /// `ScoreSheet.GetUnitScore`: five points per star of `UnitState.GetFullCost` (the unit's cost plus its passenger's).
    fn unit_score(&self, unit: &UnitState) -> Result<i32, EngineError> {
        let passenger = match unit.passenger.as_deref() {
            Some(p) => self.unit_data(p)?.cost,
            None => 0,
        };
        Ok((self.unit_data(unit)?.cost + passenger) * 5)
    }

    /// `UnitDataExtensions.IsVehicle`: boats and other carriers.
    fn is_vehicle(&self, unit: &UnitState) -> bool {
        unit_has_ability(self.rules, unit.kind, UnitAbility::Carry)
    }

    /// `ActionUtils.KillUnit`: take the unit off the map, remove its score from its owner.
    fn kill_unit_action(&mut self, at: Coords) -> Result<(), EngineError> {
        let Some(unit) = self.state.map.tile(at).and_then(|t| t.unit.clone()) else { return Ok(()) };
        if unit.follower != 0 || unit.leader != 0 || unit.effects.contains(&1) || unit_has_ability(self.rules, unit.kind, UnitAbility::Tentacles) {
            return Err(EngineError::Unimplemented { command: "KillUnit of a unit with followers / a leader / poison / tentacles" });
        }
        let unit_score = self.unit_score(&unit)?;
        self.tile_mut(at)?.unit = None;
        self.actions.push(Action::DecreaseScore { player: unit.owner, amount: unit_score });
        Ok(())
    }

    fn current_player_id(&self) -> u8 {
        self.state.players.get(self.state.current_player_index as usize).map(|p| p.id).unwrap_or(0)
    }

    /// `ImprovementExtensions.CalculateImprovementPopulationAtLevel`: the reward population plus, for levelled improvements, the
    /// growth reward population per level.
    fn improvement_population(&self, kind: u16, level: u16) -> i32 {
        let Some(d) = self.rules.improvement_by_game_idx(kind as u32) else { return 0 };
        let mut pop: i32 = d.rewards.iter().map(|r| r.population.max(0)).sum();
        if d.max_level.unwrap_or(0) > 0 {
            pop += d.growth_rewards.iter().map(|r| r.population * level as i32).sum::<i32>();
        }
        pop
    }

    /// `ActionUtils.AddPopulationForTiles`: every newly claimed tile with an improvement adds its population to the city.
    fn add_population_for_tiles(&mut self, player: u8, tiles: &[Coords], city: Coords) {
        for c in tiles {
            let Some(imp) = self.state.map.tile(*c).and_then(|t| t.improvement.as_ref()) else { continue };
            for _ in 0..self.improvement_population(imp.kind, imp.level).max(0) {
                self.actions.push(Action::IncreasePopulation { player, at: city });
            }
        }
    }

    /// `UnitDataExtensions.GetSightRange`: 1, or 2 for units with the Scout ability.
    fn sight_range(&self, unit_kind: u16) -> i32 {
        1 + unit_has_ability(self.rules, unit_kind, UnitAbility::Scout) as i32
    }

    /// `ActionUtils.ExploreFromTile(state, player, tile, sightRange, shouldUseActions = true)`: a mountain tile sees
    /// 2 tiles around it, any other tile `sight_range`; every unexplored tile in that square gets an `ExploreAction`
    /// (queued farthest first, so the nearest runs first).
    fn explore_from_tile(&mut self, player_index: usize, center: Coords, sight_range: i32) {
        let radius = match self.state.map.tile(center) {
            Some(t) if t.terrain == crate::rules::Terrain::Mountain as u16 => 2,
            Some(_) => sight_range,
            None => return,
        };
        let player = self.state.players[player_index].id;
        let mut area: Vec<Coords> = Vec::new();
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let c = Coords { x: center.x + dx, y: center.y + dy };
                if self.state.map.tile(c).is_some() {
                    area.push(c);
                }
            }
        }
        // GetAreaSorted: nearest first (tie order within a distance is not read; it only orders queued actions).
        area.sort_by_key(|c| ((c.x - center.x).abs().max((c.y - center.y).abs()), (c.y, c.x)));
        for c in area.into_iter().rev() {
            let explored = self.state.map.tile(c).map(|t| t.explorers.contains(&player)).unwrap_or(true);
            if !explored {
                self.actions.push(Action::Explore { player, at: c });
            }
        }
    }

    /// `ExploreAction.ExecuteDefault`: mark the tile explored, flag the routes for update, queue +5 score
    /// (`ScoreSheet.exploreValue`), and queue a `MeetAction` for a stranger's unit and/or city on the tile. The meets
    /// are queued after the score, so they run first (the city's meet before the unit's).
    fn explore_action(&mut self, player: u8, at: Coords) -> Result<(), EngineError> {
        let player_index = self.player_index(player)?;
        let tile = self.state.map.tile_mut(at).ok_or_else(|| EngineError::Inconsistent { what: format!("explore {at}") })?;
        if !tile.explorers.contains(&player) {
            tile.explorers.push(player);
        }
        self.state.has_flagged_need_for_update_routes = true;
        self.actions.push(Action::IncreaseScore { player, amount: EXPLORE_VALUE });
        let tile = self.state.map.tile(at).expect("tile exists");
        if let Some(u) = tile.unit.as_ref() {
            if u.effects.contains(&UNIT_EFFECT_INVISIBLE) && u.owner != player {
                return Err(EngineError::Unimplemented { command: "ExploreAction onto an invisible unit (UnitState.IsInvisible)" });
            }
        }
        let unit_owner = tile.unit.as_ref().map(|u| u.owner).filter(|o| !self.knows_player(player_index, *o));
        let city_owner = tile
            .improvement
            .as_ref()
            .filter(|i| i.kind == IMPROVEMENT_CITY)
            .map(|_| tile.owner)
            .filter(|o| *o != 0 && *o != player && !self.knows_player(player_index, *o));
        if let Some(o) = unit_owner {
            self.actions.push(Action::Meet { player, other: o, at });
            if let Some(c) = city_owner.filter(|c| *c != o) {
                self.actions.push(Action::Meet { player, other: c, at });
            }
        } else if let Some(c) = city_owner {
            self.actions.push(Action::Meet { player, other: c, at });
        }
        // ExploreLightHouseTask.EvaluateTile: revealing a map corner triggers the lighthouse task.
        if self.is_corner(at) {
            self.on_reveal_lighthouse_tile(player_index, at)?;
        }
        Ok(())
    }

    fn is_corner(&self, c: Coords) -> bool {
        let (w, h) = (self.state.map.width as i32, self.state.map.height as i32);
        (c.x == 0 || c.x == w - 1) && (c.y == 0 || c.y == h - 1)
    }

    /// `ActionUtils.OnRevealLighthouseTile`: re-check / enable the Explore-lighthouses task, give the capital a population
    /// point and level the lighthouse up (queued last, so it runs first).
    fn on_reveal_lighthouse_tile(&mut self, player_index: usize, at: Coords) -> Result<(), EngineError> {
        let player = self.state.players[player_index].id;
        self.check_task(player_index, TASK_EXPLORE_LIGHTHOUSES)?;
        self.enable_task(player, TASK_EXPLORE_LIGHTHOUSES);
        // PlayerExtensions.GetCurrentCapitalCoordinates: the start tile, while the player still owns it.
        let capital = self.state.players[player_index].start_tile;
        if self.state.map.tile(capital).map(|t| t.owner) != Some(player) {
            return Err(EngineError::Unimplemented { command: "GetCurrentCapitalCoordinates for a player that lost their start tile" });
        }
        self.actions.push(Action::IncreasePopulation { player, at: capital });
        self.actions.push(Action::ImprovementLevelUp { player, at });
        Ok(())
    }

    /// `ActionUtils.TryRevealCapital`: with the capital-vision ability, meeting a player also explores their start tile.
    fn try_reveal_capital(&mut self, player_index: usize, other_index: usize) {
        if !self.player_has_capital_vision(player_index) {
            return;
        }
        let capital = self.state.players[other_index].start_tile;
        let player = self.state.players[player_index].id;
        if let Some(t) = self.state.map.tile(capital) {
            if !t.explorers.contains(&player) {
                self.actions.push(Action::Explore { player, at: capital });
            }
        }
    }

    /// `MeetAction.ExecuteDefault` (game version >= 113).
    fn meet_action(&mut self, player: u8, other: u8, at: Coords) -> Result<(), EngineError> {
        let me = self.player_index(player)?;
        let them = self.player_index(other)?;
        self.try_reveal_capital(me, them);
        let turn = self.state.current_turn as i32;
        let p = &mut self.state.players[me];
        p.known_players.push(other);
        // GetRelation creates the relation on demand.
        if !p.relations.iter().any(|(id, _)| *id == other) {
            p.relations.push((other, DiplomacyRelation::default_relation()));
        }
        p.relations.iter_mut().find(|(id, _)| *id == other).expect("relation exists").1.first_meet = turn;
        // MeetAction.GetReward: 2 * min(round(score / 1000), 5) + 1, +2 more for the second player of a two-player game
        // meeting player 1 (game version >= 123, "away stars").
        let base = ((self.state.players[them].score as f32 / 1000.0).ceil() as i32).min(5);
        let away = self.state.version >= 123 && self.state.players.iter().filter(|p| p.id != 255).count() == 2 && other == 1;
        let reward = 2 * base + if away { 3 } else { 1 };
        let _ = at;
        self.actions.push(Action::IncreaseCurrency { player, amount: reward });
        Ok(())
    }

    fn unit_on(&self, at: Coords, unit_id: u32) -> Result<&UnitState, EngineError> {
        self.state
            .map
            .tile(at)
            .and_then(|t| t.unit.as_ref())
            .filter(|u| u.id == unit_id)
            .ok_or_else(|| EngineError::Inconsistent { what: format!("unit {unit_id} is not on {at}") })
    }

    fn tile_unit_mut(&mut self, at: Coords, unit_id: u32) -> Result<&mut UnitState, EngineError> {
        self.state
            .map
            .tile_mut(at)
            .and_then(|t| t.unit.as_mut())
            .filter(|u| u.id == unit_id)
            .ok_or_else(|| EngineError::Inconsistent { what: format!("unit {unit_id} is not on {at}") })
    }

    // ---- EndTurn / StartTurn ---------------------------------------------------------------

    /// `PlayerExtensions.IsAlive`: Nature always lives; otherwise, under the "cities" death rule (0) the player needs a
    /// city, under the "units" rule (1) a unit; any other rule means alive.
    fn player_is_alive(&self, player_index: usize) -> bool {
        let id = self.state.players[player_index].id;
        if id == 255 {
            return true;
        }
        match self.state.settings.rules.player_death_condition {
            0 => self.state.map.tiles.iter().any(|t| t.owner == id && t.improvement.as_ref().map(|i| i.kind == IMPROVEMENT_CITY).unwrap_or(false)),
            1 => self.state.map.tiles.iter().any(|t| t.unit.as_ref().map(|u| u.owner == id).unwrap_or(false)),
            _ => true,
        }
    }

    /// `GameState.EndPlayerTurn`: move on to the next living player; when the order wraps around, the turn counter
    /// goes up. (The loop would not end if nobody were alive; Nature always is.)
    fn end_player_turn(&mut self) {
        let count = self.state.players.len();
        let mut wrapped = false;
        loop {
            let mut next = self.state.current_player_index as usize + 1;
            if next >= count {
                next = 0;
                wrapped = true;
            }
            self.state.current_player_index = next as u8;
            if self.player_is_alive(next) {
                if wrapped {
                    self.state.current_turn += 1;
                }
                return;
            }
        }
    }


    /// `UnitDataExtensions.GetMaxHealth`: the type's health, +50 per promotion; a vehicle uses its passenger's values.
    fn max_health(&self, unit: &UnitState) -> i32 {
        let u = unit.passenger.as_deref().unwrap_or(unit);
        self.rules.unit_by_game_idx(u.kind as u32).map(|d| d.health).unwrap_or(0) + 50 * u.promotion_level as i32
    }

    /// `UnitDataExtensions.CanRecover`: no leader, hurt (or poisoned), and neither moved nor attacked.
    /// (`CanAttack` was not read; assumed to be "has not attacked and is not frozen".)
    fn can_recover(&self, unit: &UnitState) -> bool {
        let frozen = unit.effects.contains(&UNIT_EFFECT_FROZEN);
        unit.leader == 0
            && ((unit.health as i32) < self.max_health(unit) || unit.effects.contains(&UNIT_EFFECT_POISONED))
            && !unit.moved
            && !frozen
            && !unit.attacked
    }

    /// `EndTurnCommand.ExecuteDefault`: queue the `EndTurnAction`; then, for every tile in map order, heal the
    /// player's idle hurt units, give their units a fresh turn (`moved`/`attacked` cleared, frozen and petrified lifted),
    /// and apply improvement upkeep.
    fn end_turn_command(&mut self, player: u8) -> Result<(), EngineError> {
        self.actions.push(Action::EndTurn { player });
        for index in 0..self.state.map.tiles.len() {
            let at = self.state.map.tiles[index].coordinates;
            if let Some(unit) = self.state.map.tiles[index].unit.clone().filter(|u| u.owner == player) {
                if self.can_recover(&unit) {
                    self.actions.push(Action::Heal { player, at, fixed: 0 });
                }
                if unit_has_ability(self.rules, unit.kind, UnitAbility::Grow) {
                    return Err(EngineError::Unimplemented { command: "EndTurn: units with the Grow ability (GetUnitWillGrowInto)" });
                }
                let double_attack = unit_has_ability(self.rules, unit.kind, UnitAbility::Doubleattack);
                let u = self.state.map.tiles[index].unit.as_mut().expect("unit exists");
                u.moved = false;
                u.attacked = false;
                u.effects.retain(|e| *e != UNIT_EFFECT_FROZEN && *e != UNIT_EFFECT_PETRIFIED);
                if double_attack && !u.effects.contains(&UNIT_EFFECT_DOUBLE_READY) {
                    u.effects.push(UNIT_EFFECT_DOUBLE_READY);
                }
            }
            let tile = &self.state.map.tiles[index];
            if tile.owner == player {
                if let Some(imp) = &tile.improvement {
                    let data = self.rules.improvement_by_game_idx(imp.kind as u32);
                    if data.map(|d| d.abilities.contains(crate::rules::ImprovementAbility::Heal)).unwrap_or(false) {
                        return Err(EngineError::Unimplemented { command: "EndTurn: healing improvements (GetHealOptions)" });
                    }
                    if imp.effects.contains(&0) {
                        return Err(EngineError::Unimplemented { command: "EndTurn: improvement effect 0 income" });
                    }
                }
            }
        }
        // ActionUtils.UpdateTentacles runs last; it only does something when a tentacle unit exists.
        let tentacles = self.state.map.tiles.iter().any(|t| t.unit.as_ref().map(|u| unit_has_ability(self.rules, u.kind, UnitAbility::Tentacles)).unwrap_or(false));
        if tentacles {
            return Err(EngineError::Unimplemented { command: "ActionUtils.UpdateTentacles" });
        }
        Ok(())
    }

    /// `EndTurnAction.Execute`.
    fn end_turn_action(&mut self, player: u8) -> Result<(), EngineError> {
        let player_index = self.player_index(player)?;
        // The Pacifist task counts the turns the player has ended (`PacifistTask.Bump`, which does nothing once done).
        if let Some(t) = self.state.players[player_index].tasks.iter_mut().find(|t| t.kind == TASK_PACIFIST) {
            if !t.is_completed {
                t.counter = Some(t.counter.unwrap_or(0) + 1);
            }
        }
        if player == self.current_player_id() {
            self.end_player_turn();
        }
        if let Some(winner) = self.try_get_winner()? {
            self.actions.push(Action::GameOver { player, winner });
        }
        let new_current = self.current_player_id();
        self.actions.push(Action::StartTurn { player: new_current });
        if self.state.settings.game_type == 2 {
            return Err(EngineError::Unimplemented { command: "PassPlayerAction (game type 2)" });
        }
        for tile in &mut self.state.map.tiles {
            if tile.owner == player {
                if let Some(imp) = &mut tile.improvement {
                    imp.effects.retain(|e| *e != IMPROVEMENT_EFFECT_CLEARED_AT_TURN_END);
                }
            }
        }
        Ok(())
    }

    /// `StartTurnAction.ExecuteDefault`: for each tile the player owns that has an improvement (from turn 1 on) queue the
    /// income `CalculateWork` gives; then re-check the Pacifist task and look for newly met players.
    fn start_turn_action(&mut self, player: u8) -> Result<(), EngineError> {
        let player_index = self.player_index(player)?;
        // CalculateEmbassyLevel for every other player: only does something when an embassy exists.
        for (id, relation) in self.state.players[player_index].relations.clone() {
            // Meeting a player only sets `first_meet`; anything else (peace, war, attacks, embassies) is not implemented.
            // Only the meeting and attack history may be set: peace / war states, cooldowns and embassies are not implemented.
            let mut plain = relation.clone();
            let default = DiplomacyRelation::default_relation();
            (plain.first_meet, plain.last_attack_turn, plain.previous_attack_turn) = (default.first_meet, default.last_attack_turn, default.previous_attack_turn);
            if plain != default || !self.state.players[player_index].messages.is_empty() {
                let _ = id;
                return Err(EngineError::Unimplemented { command: "StartTurn: non-default diplomatic relations (embassies, peace)" });
            }
        }
        let spreading: Vec<u16> = self
            .rules
            .tribes()
            .iter()
            .filter(|t| t.abilities.contains(crate::rules::TribeAbility::Spread))
            .map(|t| t.game_idx as u16)
            .collect();
        for index in 0..self.state.map.tiles.len() {
            let (owner, has_improvement, climate, capital_of) = {
                let t = &self.state.map.tiles[index];
                (t.owner, t.improvement.is_some(), t.climate, t.capital_of)
            };
            if owner == player {
                if has_improvement && self.state.current_turn != 0 {
                    let work = self.tile_income(index)?;
                    if work > 0 {
                        self.actions.push(Action::IncreaseCurrency { player, amount: work });
                    }
                }
                // RestoreClimate only acts on spreading climates.
                if spreading.contains(&(climate as u16)) {
                    return Err(EngineError::Unimplemented { command: "ActionUtils.RestoreClimate (spreading climate)" });
                }
            } else if owner != 0 && capital_of == owner && has_improvement {
                // CalculateWork for another player's capital: embassy income, zero without an embassy (relations are default here).
            }
        }
        self.check_task(player_index, TASK_PACIFIST)?;
        self.check_tribe_connection(player_index)
    }

    /// The per-tile part of `StartTurnAction`: a city that has the xp to level up queues `CityLevelUpAction`, the improvement's
    /// level is brought up to date (`UpdateImprovementLevel`, which queues level-ups), then the stars it produces
    /// (`TileDataExtensions.CalculateWork` with the calculated level) are returned.
    fn tile_income(&mut self, index: usize) -> Result<i32, EngineError> {
        let tile = &self.state.map.tiles[index];
        let (at, owner) = (tile.coordinates, tile.owner);
        let imp = tile.improvement.clone().expect("improvement checked by the caller");
        let data = self
            .rules
            .improvement_by_game_idx(imp.kind as u32)
            .ok_or_else(|| EngineError::Inconsistent { what: format!("unknown improvement {}", imp.kind) })?;
        use crate::rules::ImprovementAbility as A;
        if data.abilities.contains(A::Attract) {
            return Err(EngineError::Unimplemented { command: "StartTurn: Attract improvements (CreateResourceAction)" });
        }
        if imp.xp > imp.level as i16 && owner == self.current_player_id() {
            self.actions.push(Action::CityLevelUp { player: owner, at });
        }
        self.update_improvement_level(owner, at)?;
        let tile = &self.state.map.tiles[index];
        let level = self.calculate_improvement_level(tile)?;
        if imp.kind != IMPROVEMENT_CITY {
            // IsImprovementDoubled only applies to improvements with the Doubled ability, which the rules do not list.
            return Ok((data.work.unwrap_or(0) * level).max(0));
        }
        // An enemy unit standing on the city stops its income; so does improvement effect 1 (version > 93).
        if tile.unit.as_ref().map(|u| u.owner != tile.owner).unwrap_or(false) || imp.effects.contains(&1) {
            return Ok(0);
        }
        let player = &self.state.players[self.player_index(tile.owner)?];
        let mut income = imp.production as i32 + level - 1;
        if player.start_tile == tile.coordinates {
            income += player.handicap; // the capital: handicap bonus (embassy income is 0 with default relations)
        }
        Ok((income + (imp.xp as i32).min(0)).max(0))
    }

    /// `ActionUtils.CheckTribeConnection`: for every tile the player has explored, a unit or city of a player they have
    /// not met yet queues a `MeetAction` (same rules as `ExploreAction`).
    fn check_tribe_connection(&mut self, player_index: usize) -> Result<(), EngineError> {
        let id = self.state.players[player_index].id;
        if id == 255 {
            return Ok(());
        }
        let mut meets = Vec::new();
        for t in &self.state.map.tiles {
            if !t.explorers.contains(&id) {
                continue;
            }
            if t.unit.as_ref().map(|u| u.effects.contains(&UNIT_EFFECT_INVISIBLE) && u.owner != id).unwrap_or(false) {
                return Err(EngineError::Unimplemented { command: "CheckTribeConnection past an invisible unit (UnitState.IsInvisible)" });
            }
            let unit_owner = t.unit.as_ref().map(|u| u.owner).filter(|o| !self.knows_player(player_index, *o));
            let city_owner = t
                .improvement
                .as_ref()
                .filter(|i| i.kind == IMPROVEMENT_CITY)
                .map(|_| t.owner)
                .filter(|o| *o != 0 && *o != id && !self.knows_player(player_index, *o));
            if let Some(o) = unit_owner {
                meets.push(Action::Meet { player: id, other: o, at: t.coordinates });
                if let Some(c) = city_owner.filter(|c| *c != o) {
                    meets.push(Action::Meet { player: id, other: c, at: t.coordinates });
                }
            } else if let Some(c) = city_owner {
                meets.push(Action::Meet { player: id, other: c, at: t.coordinates });
            }
        }
        self.actions.extend(meets.into_iter().rev());
        Ok(())
    }

    /// `HealAction.ExecuteDefault`: a poisoned unit is cured instead of healed; otherwise it regains `fixed` (or 40 in its
    /// own territory, 20 elsewhere) health, up to its maximum.
    fn heal_action(&mut self, _player: u8, at: Coords, fixed: u16) -> Result<(), EngineError> {
        let tile = self.state.map.tile(at).ok_or_else(|| EngineError::Inconsistent { what: format!("heal at {at}") })?;
        let unit = tile.unit.clone().ok_or_else(|| EngineError::Inconsistent { what: format!("no unit to heal at {at}") })?;
        if unit.passenger.is_some() {
            return Err(EngineError::Unimplemented { command: "HealAction for a unit with a passenger" });
        }
        let max = self.max_health(&unit);
        let own_ground = tile.owner == unit.owner;
        let u = self.state.map.tile_mut(at).expect("tile exists").unit.as_mut().expect("unit exists");
        if u.effects.contains(&UNIT_EFFECT_POISONED) {
            u.effects.retain(|e| *e != UNIT_EFFECT_POISONED && *e != UNIT_EFFECT_CHARMED);
            return Ok(());
        }
        let amount = if fixed != 0 { fixed as i32 } else if own_ground { 40 } else { 20 };
        u.health = (u.health as i32 + amount).min(max) as u16;
        Ok(())
    }

    // ---- ruins ------------------------------------------------------------------------------

    /// `ExamineRuinsCommand.Execute` (version > 119): pick the reward with `GetRuinsRewardDefault`, queue the action.
    fn examine_ruins_command(&mut self, player: u8, at: Coords) -> Result<(), EngineError> {
        let player_index = self.player_index(player)?;
        let tile = self.state.map.tile(at).ok_or_else(|| EngineError::Inconsistent { what: format!("ruins at {at}") })?;
        if tile.unit.as_ref().map(|u| u.effects.contains(&3)).unwrap_or(false) {
            return Err(EngineError::Unimplemented { command: "ExamineRuins by an invisible unit (RevealAction)" });
        }
        let flooded = self.rules.tribe_by_game_idx(self.state.players[player_index].tribe as u32).map(|t| t.abilities.contains(crate::rules::TribeAbility::Flooded)).unwrap_or(false);
        if is_water_terrain(tile.terrain) && flooded {
            return Err(EngineError::Unimplemented { command: "ExamineRuins: Flooded tribe lost-city reward" });
        }
        let mut list = [1u16, 2, 8, 6, 3, 4];
        let count = list.len() as i32;
        let seed = self.state.seed as u32;
        for i in 0..count - 1 {
            let h = crate::scout::xxh32_words(seed, &[at.x as u32, at.y as u32, i as u32]);
            let j = (i + (h % (count - i) as u32) as i32) as usize;
            list.swap(i as usize, j);
        }
        let mut reward = 1;
        for r in list {
            if self.ruins_reward_valid(player_index, at, r)? {
                reward = r;
                break;
            }
        }
        self.actions.push(Action::ExamineRuins { player, reward, at });
        Ok(())
    }

    fn ruins_reward_valid(&self, player_index: usize, at: Coords, reward: u16) -> Result<bool, EngineError> {
        let tile = self.state.map.tile(at).expect("tile exists");
        Ok(match reward {
            1 | 2 => true,
            4 => !self.unlockable_techs(player_index).is_empty(),
            8 => {
                if is_water_terrain(tile.terrain) {
                    false
                } else {
                    let me = self.state.players[player_index].id;
                    self.state.players.iter().any(|p| p.id != me && p.id != 255)
                }
            }
            3 => {
                // Some neighbour the player can walk on, and an unexplored tile within two tiles.
                let unlocked = self.unlocked_terrains(player_index);
                let me = self.state.players[player_index].id;
                let near = self.area(at, 1, false).into_iter().filter(|c| unlocked.contains(&self.state.map.tile(*c).expect("in map").terrain)).count();
                near > 0 && self.area(at, 2, false).into_iter().any(|c| !self.state.map.tile(c).expect("in map").explorers.contains(&me))
            }
            6 => is_water_terrain(tile.terrain) && self.unlocked_terrains(player_index).contains(&(crate::rules::Terrain::Ocean as u16)),
            _ => return Err(EngineError::Unimplemented { command: "ExamineRuins reward validity" }),
        })
    }

    /// `GameLogicData.GetUnlockedMovements`: the terrains the player's technologies let units enter.
    fn unlocked_terrains(&self, player_index: usize) -> Vec<u16> {
        self.state.players[player_index]
            .available_tech
            .iter()
            .filter_map(|t| self.rules.tech_by_game_idx(*t as u32))
            .flat_map(|t| t.movement_unlocks.iter().map(|(terrain, _)| *terrain as u16))
            .collect()
    }

    /// `GameLogicData.GetUnlockableTech` (version > 113): for each known tech in data order, the techs it unlocks that the
    /// player does not have yet, without repeats.
    fn unlockable_techs(&self, player_index: usize) -> Vec<u16> {
        let known = &self.state.players[player_index].available_tech;
        let mut out: Vec<u16> = Vec::new();
        for tech in self.rules.techs() {
            if !known.contains(&(tech.game_idx as u16)) {
                continue;
            }
            for child in &tech.tech_unlocks {
                let idx = self.rules.tech(*child).game_idx as u16;
                if !known.contains(&idx) && !out.contains(&idx) && idx != 12 {
                    out.push(idx);
                }
            }
        }
        out
    }

    /// `ExamineRuinsAction.ExecuteDefault`: the ruins disappear, the unit is exhausted, and the reward is paid.
    fn examine_ruins_action(&mut self, player: u8, reward: u16, at: Coords) -> Result<(), EngineError> {
        let player_index = self.player_index(player)?;
        self.tile_mut(at)?.improvement = None;
        let unit_id = self.state.map.tile(at).and_then(|t| t.unit.as_ref()).map(|u| u.id);
        if let Some(id) = unit_id {
            self.make_exhausted(id);
        }
        match reward {
            1 => {
                for _ in 0..10 {
                    self.actions.push(Action::IncreaseCurrency { player, amount: 1 });
                }
            }
            2 => {
                // PlayerExtensions.GetCurrentCapitalCoordinates: the city that is currently the player's capital.
                let capital = self.state.map.tiles.iter().find(|t| t.capital_of == player && t.improvement.is_some()).map(|t| t.coordinates);
                if let Some(capital) = capital {
                    for _ in 0..3 {
                        self.actions.push(Action::IncreasePopulation { player, at: capital });
                    }
                }
            }
            3 => {
                self.state.current_unit_id += 1;
                let unit_id = self.state.current_unit_id;
                let moves = if self.state.version < 120 { 15 } else { 12 };
                let seed = crate::scout::hash_xy(self.state.seed, at.x, at.y);
                self.actions.push(Action::ScoutMove { player, unit_id, remaining: moves, seed, from: at, path: Vec::new() });
            }
            4 => {
                let techs = self.unlockable_techs(player_index);
                if techs.is_empty() {
                    return Ok(());
                }
                let h = crate::scout::xxh32_words(self.state.seed as u32, &[at.x as u32, at.y as u32, 0]);
                let tech = techs[(h % techs.len() as u32) as usize];
                self.actions.push(Action::Research { player, tech, cost: 0 });
            }
            6 | 8 => {
                // Promote is queued first so it runs last: it promotes whatever stands on the tile once the new unit arrived.
                let kind = if reward == 8 {
                    11
                } else if self.state.version >= 104 {
                    44
                } else {
                    return Err(EngineError::Unimplemented { command: "ExamineRuins reward 6 before game version 104" });
                };
                self.actions.push(Action::Promote { player, at });
                self.train_unit_on_occupied_space(player, kind, at)?;
            }
            _ => return Err(EngineError::Unimplemented { command: "ExamineRuins reward effect (giant / navalon / lost city)" }),
        }
        Ok(())
    }

    // ---- deferred work --------------------------------------------------------------------

    /// `RunDeferredAction.Execute`: if something flagged the routes as stale (exploring does), queue a city-connection
    /// update for the acting player and then a route update (so routes are recomputed first); the flag is always cleared.
    fn run_deferred(&mut self, player: u8) -> Result<(), EngineError> {
        if self.state.has_flagged_need_for_update_routes {
            self.actions.push(Action::UpdateCityConnections { players: vec![player] });
            self.actions.push(Action::UpdateRoutes);
        }
        self.state.has_flagged_need_for_update_routes = false;
        Ok(())
    }

    /// `MapDataExtensions.UpdateRoutes`: clear every tile's route flag, then for each player connect their route openers
    /// (ports, atolls, outposts, ...) pairwise, and openers with the Network ability to every city, with routes found by the
    /// route-builder pathfinder (see [`crate::pathfinder::get_router_path`]). Every tile on a found path gets the route flag
    /// immediately, which makes it cheaper for the paths searched after it.
    fn update_routes(&mut self) -> Result<(), EngineError> {
        for tile in &mut self.state.map.tiles {
            tile.has_route = false;
        }
        for pi in 0..self.state.players.len() {
            let id = self.state.players[pi].id;
            if id == 255 {
                continue;
            }
            let mut allowed: Vec<u16> = Vec::new();
            for tech in &self.state.players[pi].available_tech {
                if let Some(t) = self.rules.tech_by_game_idx(*tech as u32) {
                    allowed.extend(t.movement_unlocks.iter().map(|(terrain, _)| *terrain as u16));
                }
            }
            let mut routers: Vec<Coords> = Vec::new();
            let mut cities: Vec<Coords> = Vec::new();
            for t in &self.state.map.tiles {
                if t.owner != id {
                    continue;
                }
                let Some(imp) = t.improvement.as_ref() else { continue };
                let Some(data) = self.rules.improvement_by_game_idx(imp.kind as u32) else { continue };
                if !data.routes.is_empty() {
                    routers.push(t.coordinates);
                }
                if imp.kind == IMPROVEMENT_CITY {
                    cities.push(t.coordinates);
                }
            }
            for i in 0..routers.len() {
                for j in i + 1..routers.len() {
                    self.find_route_between(pi, routers[i], routers[j], &allowed)?;
                }
                let kind = self.state.map.tile(routers[i]).and_then(|t| t.improvement.as_ref()).map(|i| i.kind).unwrap_or(0);
                if self.rules.improvement_by_game_idx(kind as u32).map(|d| d.abilities.contains(crate::rules::ImprovementAbility::Network)).unwrap_or(false) {
                    for c in &cities {
                        self.find_route_between(pi, routers[i], *c, &allowed)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// `MapDataExtensions.UpdateRoutes.FindPathBetweenRouters`.
    fn find_route_between(&mut self, player_index: usize, origin: Coords, dest: Coords, allowed: &[u16]) -> Result<(), EngineError> {
        let kind_at = |c: Coords| self.state.map.tile(c).and_then(|t| t.improvement.as_ref()).map(|i| i.kind);
        let (Some(ok), Some(dk)) = (kind_at(origin), kind_at(dest)) else { return Ok(()) };
        let (Some(od), Some(dd)) = (self.rules.improvement_by_game_idx(ok as u32), self.rules.improvement_by_game_idx(dk as u32)) else {
            return Ok(());
        };
        let range = od.range.unwrap_or(0);
        if (origin.x - dest.x).abs().max((origin.y - dest.y).abs()) > range {
            return Ok(());
        }
        let bits = |f: &crate::rules::FlagSet<crate::rules::Terrain>| (0u16..16).filter(|i| f.bits() & (1 << i) != 0).collect::<Vec<u16>>();
        let dest_routes = bits(&dd.routes);
        let route_terrain: Vec<u16> =
            bits(&od.routes).into_iter().filter(|t| (dest_routes.contains(t) && allowed.contains(t)) || dk == IMPROVEMENT_CITY).collect();
        let Some(path) =
            crate::pathfinder::get_router_path(&self.state, self.rules, &self.state.players[player_index], origin, dest, range, route_terrain)?
        else {
            return Ok(());
        };
        for c in path {
            if let Some(t) = self.state.map.tile_mut(c) {
                t.has_route = true;
            }
        }
        Ok(())
    }

    /// `UpdateCityConnectionsAction.Execute` / `ActionUtils.UpdateCityConnections`: for each listed player (once, not Nature),
    /// connect the cities that reach their capital over roads / routes, and disconnect those that no longer do.
    fn update_city_connections(&mut self, players: &[u8]) -> Result<(), EngineError> {
        let mut seen: Vec<u8> = Vec::new();
        for id in players {
            if *id == 0 || seen.contains(id) {
                continue;
            }
            seen.push(*id);
            let Ok(pi) = self.player_index(*id) else { continue };
            let connected = self.find_connected_cities(pi);
            let cities: Vec<Coords> = self
                .state
                .map
                .tiles
                .iter()
                .filter(|t| t.owner == *id && t.improvement.as_ref().map(|i| i.kind == IMPROVEMENT_CITY).unwrap_or(false))
                .map(|t| t.coordinates)
                .collect();
            for c in cities {
                let flag = self.state.map.tile(c).and_then(|t| t.improvement.as_ref()).map(|i| i.connected_to_capital_of_player).unwrap_or(0);
                if connected.contains(&c) {
                    if flag == 0 {
                        self.actions.push(Action::ConnectCity { player: *id, at: c });
                    } else if flag != *id {
                        self.actions.push(Action::ChangeCityConnection { player: *id, at: c });
                    }
                } else if flag != 0 {
                    self.actions.push(Action::DisconnectCity { player: *id, at: c });
                }
            }
        }
        Ok(())
    }

    /// `TileData.IsConnectable`: a road, a city, or a route opener.
    fn is_connectable(&self, t: &TileData) -> bool {
        t.has_road
            || t.improvement
                .as_ref()
                .map(|i| i.kind == IMPROVEMENT_CITY || self.rules.improvement_by_game_idx(i.kind as u32).map(|d| !d.routes.is_empty()).unwrap_or(false))
                .unwrap_or(false)
    }

    /// `TileData.HasMatchingTransportPath(a, b)`.
    fn has_matching_transport_path(&self, a: &TileData, b: &TileData) -> bool {
        if b.owner != 0 && b.owner != a.owner {
            let peace = self
                .player_index(a.owner)
                .map(|i| self.state.players[i].relations.iter().any(|(id, r)| *id == b.owner && r.state == 1))
                .unwrap_or(true);
            if !peace {
                return false;
            }
        }
        (self.is_connectable(a) && self.is_connectable(b)) || (a.has_route && b.has_route)
    }

    /// `MapDataExtensions.FindConnectedCities`: breadth-first search from the capital over matching transport paths; every
    /// city reached (other than the capital) is connected.
    fn find_connected_cities(&self, player_index: usize) -> Vec<Coords> {
        let p = &self.state.players[player_index];
        let mut connected = Vec::new();
        let Some(capital) = self.state.map.tile(p.start_tile).filter(|t| t.owner == p.id) else { return connected };
        let mut visited: Vec<Coords> = Vec::new();
        let mut queue = std::collections::VecDeque::from([capital.coordinates]);
        let mut guard = 10_000;
        while let Some(c) = queue.pop_front() {
            if guard == 0 {
                break;
            }
            guard -= 1;
            if !visited.contains(&c) {
                visited.push(c);
            }
            let t = self.state.map.tile(c).expect("tile");
            if c != capital.coordinates && t.improvement.as_ref().map(|i| i.kind == IMPROVEMENT_CITY).unwrap_or(false) {
                connected.push(c);
            }
            for n in self.area(c, 1, false) {
                let nt = self.state.map.tile(n).expect("tile");
                if self.has_matching_transport_path(t, nt) && !visited.contains(&n) {
                    queue.push_back(n);
                }
            }
        }
        connected
    }

    /// `ConnectCityAction.ExecuteDefault` (game version > 98).
    fn connect_city_action(&mut self, player: u8, at: Coords) -> Result<(), EngineError> {
        let pi = self.player_index(player)?;
        self.enable_task(player, TASK_NETWORK);
        let capital = self.state.players[pi].start_tile;
        self.actions.push(Action::IncreasePopulation { player, at: capital });
        self.actions.push(Action::IncreasePopulation { player, at });
        if let Some(i) = self.tile_mut(at)?.improvement.as_mut() {
            i.connected_to_capital_of_player = player;
        }
        self.check_task(pi, TASK_NETWORK)
    }

    /// `DisconnectCityAction.Execute`.
    fn disconnect_city_action(&mut self, player: u8, at: Coords) -> Result<(), EngineError> {
        let Some(connected_to) = self.state.map.tile(at).and_then(|t| t.improvement.as_ref()).map(|i| i.connected_to_capital_of_player) else { return Ok(()) };
        let capital = self.state.players[self.player_index(connected_to)?].start_tile;
        self.actions.push(Action::DecreasePopulation { player, at: capital });
        self.actions.push(Action::DecreasePopulation { player, at });
        if let Some(i) = self.tile_mut(at)?.improvement.as_mut() {
            i.connected_to_capital_of_player = 0;
        }
        Ok(())
    }

    /// `DecreasePopulationAction.Execute` (game version > 114).
    fn decrease_population_action(&mut self, at: Coords) -> Result<(), EngineError> {
        let is_city = self.state.map.tile(at).and_then(|t| t.improvement.as_ref()).map(|i| i.kind == IMPROVEMENT_CITY).unwrap_or(false);
        if !is_city {
            return Ok(());
        }
        let owner = self.state.map.tile(at).expect("tile").owner;
        let before = self.improvement_score(at)?;
        let imp = self.tile_mut(at)?.improvement.as_mut().expect("city");
        imp.xp -= 1;
        imp.population -= 1;
        let after = self.improvement_score(at)?;
        if let Ok(pi) = self.player_index(owner) {
            let p = &mut self.state.players[pi];
            p.score = (p.score as i64 - (before - after) as i64).max(0) as u32;
        }
        Ok(())
    }

    /// `StartMatchAction.ExecuteDefault`.
    fn start_match(&mut self) -> Result<(), EngineError> {
        for index in 0..self.state.players.len() {
            let tribe_idx = self.state.players[index].tribe as u32;
            // A tribe missing from the rules data is skipped by the game too.
            if let Some(tribe) = self.rules.tribe_by_game_idx(tribe_idx) {
                let techs: Vec<u16> = tribe
                    .starting_techs
                    .iter()
                    .map(|t| self.rules.tech(*t).game_idx as u16)
                    .collect();
                for tech in techs {
                    self.learn_tech(index, tech, 0, false)?;
                }
            }
            // Game mode 7 gives every player 6 extra stars (not covered by any fixture).
            if self.state.settings.rules_game_mode == 7 {
                let player = self.state.players[index].id;
                self.actions.push(Action::IncreaseCurrency { player, amount: 6 });
            }
            // ExploreLightHouseTask.TryGetFirstDiscoveredLighthouseTile: lighthouses sit at the four map
            // corners; if the player has already explored one, the game reveals it (not implemented).
            let id = self.state.players[index].id;
            if self.first_explored_corner(id).is_some() {
                return Err(EngineError::Unimplemented { command: "ActionUtils.OnRevealLighthouseTile" });
            }
        }
        self.state.current_state = 2; // Started
        Ok(())
    }

    /// `ExploreLightHouseTask.TryGetFirstDiscoveredLighthouseTile`: the first map corner this player
    /// has explored (never for Nature, id 255). Corner order is not read; only "is there one" matters here.
    fn first_explored_corner(&self, player_id: u8) -> Option<Coords> {
        if player_id == 255 {
            return None;
        }
        let map = &self.state.map;
        let (w, h) = (map.width as i32 - 1, map.height as i32 - 1);
        [Coords { x: 0, y: 0 }, Coords { x: w, y: 0 }, Coords { x: 0, y: h }, Coords { x: w, y: h }]
            .into_iter()
            .find(|c| map.tile(*c).map(|t| t.explorers.contains(&player_id)).unwrap_or(false))
    }

    /// `ActionUtils.LearnTech(state, player, tech, cost, shouldUseActions)`.
    /// Does nothing if the player knows the tech; otherwise adds it, pays `cost` stars, scores
    /// `tech.cost * 100`, enables the tasks the tech unlocks, reveals capitals for techs that grant
    /// capital vision, and finally re-checks the Genius task.
    fn learn_tech(&mut self, player_index: usize, tech: u16, cost: i32, use_actions: bool) -> Result<(), EngineError> {
        if self.state.players[player_index].available_tech.contains(&tech) {
            return Ok(());
        }
        let (score, tasks, capital_vision) = match self.rules.tech_by_game_idx(tech as u32) {
            Some(t) => (
                t.cost * 100,
                t.task_unlocks.iter().map(|id| self.rules.task(*id).game_idx as u16).collect::<Vec<_>>(),
                t.ability_unlocks.iter().any(|a| a == "capitalvision"),
            ),
            None => (0, Vec::new(), false),
        };
        let player = &mut self.state.players[player_index];
        player.available_tech.push(tech);
        player.currency -= cost;
        let player_id = player.id;
        if use_actions {
            self.actions.push(Action::IncreaseScore { player: player_id, amount: score });
        } else {
            self.add_score(player_index, score)?;
        }
        for kind in tasks {
            self.enable_task(player_id, kind);
        }
        if capital_vision {
            self.try_reveal_all_capitals(player_index)?;
        }
        self.check_task(player_index, TASK_GENIUS)
    }

    /// `ActionUtils.AddScore`: `player.score += amount`. The game then ends the match if the score limit is
    /// reached; that is not implemented, so a game with a score limit stops here instead of diverging.
    fn add_score(&mut self, player_index: usize, amount: i32) -> Result<(), EngineError> {
        let limit = self.state.settings.rules.score_limit;
        let p = &mut self.state.players[player_index];
        p.score = (p.score as i64 + amount as i64) as u32;
        if limit > 0 && p.score as i64 >= limit as i64 && self.state.current_state == 2 {
            return Err(EngineError::Unimplemented { command: "score-limit game over (TryGetWinner/GameOverAction)" });
        }
        Ok(())
    }

    /// `ActionUtils.TryRevealAllCapitals`: a player with capital vision reveals the capital of every other
    /// player they know. Revealing a capital itself (`TryRevealCapital`) is not implemented.
    fn try_reveal_all_capitals(&mut self, player_index: usize) -> Result<(), EngineError> {
        if !self.player_has_capital_vision(player_index) {
            return Ok(());
        }
        let known = self.state.players[player_index].known_players.clone();
        for other_index in 0..self.state.players.len() {
            let id = self.state.players[other_index].id;
            if id != 255 && known.contains(&id) {
                self.try_reveal_capital(player_index, other_index);
            }
        }
        Ok(())
    }

    fn player_has_capital_vision(&self, player_index: usize) -> bool {
        self.state.players[player_index].available_tech.iter().any(|t| {
            self.rules
                .tech_by_game_idx(*t as u32)
                .map(|t| t.ability_unlocks.iter().any(|a| a == "capitalvision"))
                .unwrap_or(false)
        })
    }

    /// `ActionUtils.EnableTask`: queue an `EnableTaskAction` unless the player already has that task.
    fn enable_task(&mut self, player_id: u8, kind: u16) {
        if let Some(p) = self.state.players.iter().find(|p| p.id == player_id) {
            if p.tasks.iter().any(|t| t.kind == kind) {
                return;
            }
        }
        self.actions.push(Action::EnableTask { player: player_id, kind });
    }

    /// `EnableTaskAction.Execute`: create the task (`TaskBase.GetTask`), mark it started, add it,
    /// then check whether it is already complete.
    fn enable_task_action(&mut self, player_id: u8, kind: u16) -> Result<(), EngineError> {
        let index = self.player_index(player_id)?;
        let p = &mut self.state.players[index];
        if p.tasks.iter().all(|t| t.kind != kind) {
            // KillerTask.EnableTask also counts the kill that created the task.
            let counter = task_has_counter(kind).then_some(if kind == TASK_KILLER { 1 } else { 0 });
            p.tasks.push(Task { kind, is_started: true, is_completed: false, counter });
        }
        self.check_task(index, kind)
    }

    /// `GameState.CheckTask(state, player, type)`: if the player has that task, ask it whether it is
    /// complete (`IsTaskCompleted`, which also sets the task's own completed flag) and, if so,
    /// queue a `TaskCompletedAction`. Only the tasks whose rules were read are implemented.
    fn check_task(&mut self, player_index: usize, kind: u16) -> Result<(), EngineError> {
        let Some(task_index) = self.state.players[player_index].tasks.iter().position(|t| t.kind == kind) else {
            return Ok(());
        };
        if self.state.players[player_index].tasks[task_index].is_completed {
            return Ok(()); // every IsTaskCompleted starts with "already completed -> false"
        }
        let done = match kind {
            // PacifistTask: more than 4 pacifist turns.
            TASK_PACIFIST => self.state.players[player_index].tasks[task_index].counter.unwrap_or(0) > 4,
            // GeniusTask: the player knows every tech of their tribe's tree.
            TASK_GENIUS => self.knows_all_tribe_techs(player_index),
            // WealthTask: more than 99 stars.
            TASK_WEALTH => self.state.players[player_index].currency > 99,
            // KillerTask: ten kills.
            TASK_KILLER => self.state.players[player_index].tasks[task_index].counter.unwrap_or(0) >= 10,
            // NetworkTask: at least five cities other than the capital are connected to it.
            TASK_NETWORK => {
                let p = &self.state.players[player_index];
                self.state
                    .map
                    .tiles
                    .iter()
                    .filter(|t| t.owner == p.id && t.coordinates != p.start_tile)
                    .filter_map(|t| t.improvement.as_ref().filter(|i| i.kind == IMPROVEMENT_CITY))
                    .filter(|i| i.connected_to_capital_of_player == p.id)
                    .count()
                    >= 5
            }
            // MetropolisTask: the player owns a city of level 5 or more.
            TASK_METROPOLIS => {
                let id = self.state.players[player_index].id;
                self.state.map.tiles.iter().any(|t| {
                    t.owner == id && t.improvement.as_ref().map(|i| i.kind == IMPROVEMENT_CITY && i.level >= 5).unwrap_or(false)
                })
            }
            // ExploreLightHouseTask: all four map corners have been explored by the player.
            TASK_EXPLORE_LIGHTHOUSES => {
                let id = self.state.players[player_index].id;
                let (w, h) = (self.state.map.width as i32 - 1, self.state.map.height as i32 - 1);
                [(0, 0), (w, 0), (0, h), (w, h)]
                    .iter()
                    .filter(|(x, y)| self.state.map.tile(Coords { x: *x, y: *y }).map(|t| t.explorers.contains(&id)).unwrap_or(false))
                    .count()
                    == 4
            }
            _ => return Err(EngineError::Unimplemented { command: "CheckTask for this task type" }),
        };
        if done {
            let p = &mut self.state.players[player_index];
            p.tasks[task_index].is_completed = true;
            let player = p.id;
            self.actions.push(Action::TaskCompleted { player, kind });
        }
        Ok(())
    }

    /// `GameLogicData.GetAllTechForTribe`: the techs reachable from the root tech (`basic`, 0) through
    /// each tech's unlocks, with the tribe's tech replacements applied. (?) The exact order in which
    /// replacements and unlocks are applied was not read; only the set matters here.
    fn knows_all_tribe_techs(&self, player_index: usize) -> bool {
        let player = &self.state.players[player_index];
        let Some(tribe) = self.rules.tribe_by_game_idx(player.tribe as u32) else {
            return true; // the game returns "complete" when the tribe is unknown
        };
        let replace = |t: crate::rules::TechId| {
            tribe.tech_overrides.iter().find(|(from, _)| *from == t).map(|(_, to)| *to).unwrap_or(t)
        };
        let Some(root) = self.rules.tech_id("basic") else {
            return false;
        };
        let mut seen: Vec<crate::rules::TechId> = Vec::new();
        let mut queue = std::collections::VecDeque::from([root]);
        while let Some(t) = queue.pop_front() {
            let t = replace(t);
            if seen.contains(&t) {
                continue;
            }
            seen.push(t);
            queue.extend(self.rules.tech(t).tech_unlocks.iter().copied());
        }
        seen.iter().all(|t| player.available_tech.contains(&(self.rules.tech(*t).game_idx as u16)))
    }

    fn player_index(&self, id: u8) -> Result<usize, EngineError> {
        self.state
            .players
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| EngineError::Inconsistent { what: format!("unknown player {id}") })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::savestate::load_state_file;

    /// Engine at the start of `start_game_1`'s command list (before StartMatch has run).
    fn engine<'r>(rules: &'r Rules) -> Engine<'r> {
        let save = load_state_file(include_bytes!("../../testdata/savestates/start_game_1.state")).unwrap();
        let mut start = save.initial.clone();
        start.commands = save.current.commands.clone();
        start.current_command = 0;
        Engine::new(start, rules)
    }

    fn tech(rules: &Rules, name: &str) -> u16 {
        rules.tech(rules.tech_id(name).unwrap()).game_idx as u16
    }

    fn index_of(e: &Engine, id: u8) -> usize {
        e.player_index(id).unwrap()
    }

    #[test]
    fn learning_a_known_tech_changes_nothing() {
        let rules = Rules::bundled();
        let mut e = engine(&rules);
        let climbing = tech(&rules, "climbing");
        e.learn_tech(0, climbing, 0, false).unwrap();
        let (score, techs) = (e.state.players[0].score, e.state.players[0].available_tech.clone());
        e.learn_tech(0, climbing, 0, false).unwrap();
        assert_eq!((e.state.players[0].score, &e.state.players[0].available_tech), (score, &techs));
    }

    #[test]
    fn learning_pays_the_cost_and_scores_tech_cost_times_100() {
        let rules = Rules::bundled();
        let mut e = engine(&rules);
        let (stars, score) = (e.state.players[0].currency, e.state.players[0].score);
        let t = tech(&rules, "hunting"); // cost 1
        e.learn_tech(0, t, 1, false).unwrap();
        assert_eq!(e.state.players[0].currency, stars - 1);
        assert_eq!(e.state.players[0].score, score + 100);
    }

    #[test]
    fn learning_with_actions_defers_the_score_to_an_action() {
        let rules = Rules::bundled();
        let mut e = engine(&rules);
        e.state.commands.clear(); // only the queued action should run, not the recorded StartMatch
        let score = e.state.players[0].score;
        e.learn_tech(0, tech(&rules, "hunting"), 0, true).unwrap();
        assert_eq!(e.state.players[0].score, score, "not applied yet");
        assert_eq!(e.actions, vec![Action::IncreaseScore { player: e.state.players[0].id, amount: 100 }]);
        e.run().unwrap();
        assert_eq!(e.state.players[0].score, score + 100);
    }

    #[test]
    fn genius_completes_once_every_tribe_tech_is_known_and_then_stops_at_the_unread_completion_action() {
        let rules = Rules::bundled();
        let mut e = engine(&rules);
        let i = index_of(&e, 4); // aimo
        e.run().unwrap(); // StartMatch: aimo starts the Genius task
        assert!(!e.state.players[i].tasks[0].is_completed);
        // Give aimo every tech in the data, then check the task again.
        e.state.players[i].available_tech = rules.techs().iter().map(|t| t.game_idx as u16).collect();
        e.check_task(i, TASK_GENIUS).unwrap();
        assert!(e.state.players[i].tasks[0].is_completed);
        assert_eq!(e.actions, vec![Action::TaskCompleted { player: 4, kind: TASK_GENIUS }]);
        assert_eq!(e.run(), Ok(()));
        // A completed task is never completed twice.
        e.actions.clear();
        e.check_task(i, TASK_GENIUS).unwrap();
        assert!(e.actions.is_empty());
    }

    #[test]
    fn wealth_needs_more_than_99_stars_and_pacifist_more_than_4_turns() {
        let rules = Rules::bundled();
        let mut e = engine(&rules);
        e.state.players[0].tasks.push(Task { kind: TASK_WEALTH, is_started: true, is_completed: false, counter: None });
        e.state.players[0].tasks.push(Task { kind: TASK_PACIFIST, is_started: true, is_completed: false, counter: Some(4) });
        e.state.players[0].currency = 99;
        e.check_task(0, TASK_WEALTH).unwrap();
        e.check_task(0, TASK_PACIFIST).unwrap();
        assert!(e.actions.is_empty());
        e.state.players[0].currency = 100;
        e.state.players[0].tasks[1].counter = Some(5);
        e.check_task(0, TASK_WEALTH).unwrap();
        e.check_task(0, TASK_PACIFIST).unwrap();
        assert_eq!(e.actions.len(), 2);
    }

    #[test]
    fn increase_currency_adds_stars_then_rechecks_wealth() {
        let rules = Rules::bundled();
        let mut e = engine(&rules);
        e.state.players[0].tasks.push(Task { kind: TASK_WEALTH, is_started: true, is_completed: false, counter: None });
        e.state.players[0].currency = 95;
        e.actions.push(Action::IncreaseCurrency { player: 1, amount: 6 });
        e.update().unwrap();
        assert_eq!(e.state.players[0].currency, 101);
        assert!(e.state.players[0].tasks[0].is_completed);
    }

    #[test]
    fn task_types_without_read_rules_stop_loudly() {
        let rules = Rules::bundled();
        let mut e = engine(&rules);
        e.state.players[0].tasks.push(Task { kind: 9, is_started: true, is_completed: false, counter: None }); // Convert
        assert!(matches!(e.check_task(0, 9), Err(EngineError::Unimplemented { .. })));
    }

    #[test]
    fn game_mode_7_gives_six_extra_stars() {
        let rules = Rules::bundled();
        let mut base = engine(&rules);
        base.run().unwrap();
        let mut e = engine(&rules);
        e.state.settings.rules_game_mode = 7;
        e.run().unwrap();
        for (a, b) in base.state.players.iter().zip(&e.state.players) {
            assert_eq!(b.currency, a.currency + 6, "player {}", a.id);
        }
    }

    #[test]
    fn a_corner_already_explored_by_a_player_stops_the_match_start() {
        let rules = Rules::bundled();
        let mut e = engine(&rules);
        e.state.map.tile_mut(Coords { x: 17, y: 0 }).unwrap().explorers.push(2);
        assert_eq!(e.run(), Err(EngineError::Unimplemented { command: "ActionUtils.OnRevealLighthouseTile" }));
        // Nature (255) never counts.
        let mut e = engine(&rules);
        e.state.map.tile_mut(Coords { x: 0, y: 0 }).unwrap().explorers = vec![255];
        e.run().unwrap();
    }

    #[test]
    fn a_score_limit_stops_the_match_instead_of_silently_diverging() {
        let rules = Rules::bundled();
        let mut e = engine(&rules);
        e.state.settings.rules.score_limit = 1000;
        e.state.current_state = 2;
        assert!(matches!(e.add_score(0, 600), Err(EngineError::Unimplemented { .. })));
        let mut e = engine(&rules);
        e.state.settings.rules.score_limit = 100_000;
        assert!(e.add_score(0, 600).is_ok());
    }

    #[test]
    fn capital_vision_only_matters_when_the_player_knows_other_players() {
        let rules = Rules::bundled();
        let diplomacy = tech(&rules, "diplomacy");
        let mut e = engine(&rules);
        e.learn_tech(0, diplomacy, 0, false).unwrap(); // knows nobody: nothing to reveal
        let mut e = engine(&rules);
        e.state.players[0].known_players = vec![2];
        e.learn_tech(0, diplomacy, 0, false).unwrap();
        assert!(e.actions.iter().any(|a| matches!(a, Action::Explore { .. })), "the known player's capital is explored");
    }

    // ---- Move / Train / deferred work ----

    /// Engine with `StartMatch` already run, so every player has techs, and the recorded commands cleared.
    fn started<'r>(rules: &'r Rules) -> Engine<'r> {
        let mut e = engine(rules);
        e.run().unwrap();
        e.state.commands.clear();
        e.state.current_command = 0;
        e
    }

    /// (position, unit id) of player 1's starting unit.
    fn first_unit(e: &Engine, owner: u8) -> (Coords, u32) {
        e.state.map.tiles.iter().find_map(|t| t.unit.as_ref().filter(|u| u.owner == owner).map(|u| (t.coordinates, u.id))).unwrap()
    }

    /// Makes the first in-map neighbour of `c` an empty, plain field tile and returns it.
    fn free_field_next_to(e: &mut Engine, c: Coords) -> Coords {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)] {
            let n = Coords { x: c.x + dx, y: c.y + dy };
            if let Some(t) = e.state.map.tile_mut(n) {
                t.terrain = crate::rules::Terrain::Field as u16;
                (t.unit, t.improvement, t.resource) = (None, None, None);
                t.effects.clear();
                return n;
            }
        }
        panic!("no neighbour of {c}");
    }

    #[test]
    fn directions_follow_the_game_table() {
        let o = Coords { x: 5, y: 5 };
        let at = |dx, dy| unit_direction(o, Coords { x: 5 + dx, y: 5 + dy });
        // SW 0, W 1, NW 2, N 3, NE 4, E 5, SE 6, S 7; flipped for directions below 4.
        assert_eq!(at(-1, -1), (0, true));
        assert_eq!(at(-1, 0), (1, true));
        assert_eq!(at(-1, 1), (2, true));
        assert_eq!(at(0, 1), (3, true));
        assert_eq!(at(1, 1), (4, false)); // the train_1 move
        assert_eq!(at(1, 0), (5, false));
        assert_eq!(at(1, -1), (6, false));
        assert_eq!(at(0, -1), (7, false));
        // Longer vectors pick the best-aligned direction.
        assert_eq!(at(5, 0).0, 5);
        assert_eq!(at(0, -4).0, 7);
        assert_eq!(at(3, 3).0, 4);
    }

    #[test]
    fn a_mountain_sees_two_tiles_other_tiles_see_their_sight_range() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        for t in &mut e.state.map.tiles {
            t.explorers.clear();
        }
        e.actions.clear();
        let c = Coords { x: 9, y: 9 };
        e.state.map.tile_mut(c).unwrap().terrain = crate::rules::Terrain::Field as u16;
        e.explore_from_tile(0, c, 1);
        assert_eq!(e.actions.len(), 9, "radius 1 = 3x3");
        e.actions.clear();
        e.state.map.tile_mut(c).unwrap().terrain = crate::rules::Terrain::Mountain as u16;
        e.explore_from_tile(0, c, 1);
        assert_eq!(e.actions.len(), 25, "a mountain sees radius 2 = 5x5");
        // The nearest tile runs first: it was queued last.
        assert_eq!(e.actions.last(), Some(&Action::Explore { player: 1, at: c }));
    }

    #[test]
    fn a_one_step_move_relocates_the_unit_and_sets_its_flags() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let (from, id) = first_unit(&e, 1);
        let to = free_field_next_to(&mut e, from);
        e.state.commands = vec![Command::Move { player: 1, from, to, unit_id: id }];
        e.validate_commands = true;
        e.run().unwrap();
        assert!(e.state.map.tile(from).unwrap().unit.is_none());
        let u = e.state.map.tile(to).unwrap().unit.as_ref().unwrap();
        assert_eq!((u.id, u.coordinates, u.moved, u.attacked), (id, to, true, false));
        assert_eq!(u.home, from, "home does not change");
        assert!(!e.state.has_flagged_need_for_update_routes, "the deferred action cleared it");
        assert!(e.actions.is_empty());
    }

    #[test]
    fn move_legality() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let (from, id) = first_unit(&e, 1);
        let to = free_field_next_to(&mut e, from);
        let ok = Command::Move { player: 1, from, to, unit_id: id };
        assert!(e.command_is_valid(&ok).is_ok());
        // someone else's turn
        assert!(e.command_is_valid(&Command::Move { player: 2, from, to, unit_id: id }).is_err());
        // wrong unit id / wrong tile
        assert!(e.command_is_valid(&Command::Move { player: 1, from, to, unit_id: id + 100 }).is_err());
        // already moved
        e.tile_unit_mut(from, id).unwrap().moved = true;
        assert!(e.command_is_valid(&ok).is_err());
        e.tile_unit_mut(from, id).unwrap().moved = false;
        // frozen
        e.tile_unit_mut(from, id).unwrap().effects = vec![UNIT_EFFECT_FROZEN];
        assert!(e.command_is_valid(&ok).is_err());
        e.tile_unit_mut(from, id).unwrap().effects.clear();
        // a pending command trigger blocks the player
        e.state.triggers.push(CommandTrigger { player_id: 1, kind: 0, coordinates: from, opponent_id: 0 });
        assert!(e.command_is_valid(&ok).is_err());
    }

    #[test]
    fn terrain_the_player_has_no_tech_for_is_not_enterable() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let (from, id) = first_unit(&e, 1);
        let to = free_field_next_to(&mut e, from);
        // Make the destination a mountain: player 1 (xinxi) starts with climbing, so strip it.
        e.state.map.tile_mut(to).unwrap().terrain = crate::rules::Terrain::Mountain as u16;
        let cmd = Command::Move { player: 1, from, to, unit_id: id };
        assert!(e.command_is_valid(&cmd).is_ok(), "xinxi can climb");
        let climbing = tech(&rules, "climbing");
        e.state.players[0].available_tech.retain(|t| *t != climbing);
        assert!(e.command_is_valid(&cmd).is_err(), "no climbing, no mountain");
    }

    /// A row of explored, empty field tiles starting next to `from`, for pathfinding tests.
    fn field_row(e: &mut Engine, from: Coords, len: i32) -> Vec<Coords> {
        let mut row = Vec::new();
        let step = if from.x + len < e.state.map.width as i32 { 1 } else { -1 };
        for i in 1..=len {
            let c = Coords { x: from.x + i * step, y: from.y };
            let t = e.state.map.tile_mut(c).unwrap();
            t.terrain = crate::rules::Terrain::Field as u16;
            (t.unit, t.improvement, t.resource) = (None, None, None);
            t.effects.clear();
            t.has_road = false;
            if !t.explorers.contains(&1) {
                t.explorers.push(1);
            }
            row.push(c);
        }
        row
    }

    #[test]
    fn a_unit_reaches_only_as_far_as_its_movement_and_roads_stretch_it() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let (from, id) = first_unit(&e, 1);
        e.state.map.tile_mut(from).unwrap().unit.as_mut().unwrap().kind = 2; // warrior: movement 1
        let row = field_row(&mut e, from, 3);
        let unit = e.state.map.tile(from).unwrap().unit.clone().unwrap();
        assert_eq!(e.find_path(&unit, from, row[0]).unwrap(), Some(vec![row[0], from]));
        assert_eq!(e.find_path(&unit, from, row[2]).unwrap(), None, "three tiles is out of reach for movement 1");
        // Roads halve the cost of each step (5 instead of 10), so two road tiles are reachable.
        for c in [from, row[0], row[1]] {
            e.state.map.tile_mut(c).unwrap().has_road = true;
        }
        assert_eq!(e.find_path(&unit, from, row[1]).unwrap(), Some(vec![row[1], row[0], from]));
        let _ = id;
    }

    #[test]
    fn an_adjacent_enemy_stops_a_unit_on_its_first_step() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let (from, _) = first_unit(&e, 1);
        e.state.map.tile_mut(from).unwrap().unit.as_mut().unwrap().kind = 3; // rider: movement 2
        let row = field_row(&mut e, from, 3);
        let unit = e.state.map.tile(from).unwrap().unit.clone().unwrap();
        assert!(e.find_path(&unit, from, row[1]).unwrap().is_some());
        // An enemy next to the first tile makes entering it cost everything: the second step is out of reach.
        let mut enemy = unit.clone();
        enemy.owner = 2;
        let beside = if row[0].y + 1 < e.state.map.height as i32 { row[0].y + 1 } else { row[0].y - 1 };
        e.state.map.tile_mut(Coords { x: row[0].x, y: beside }).unwrap().unit = Some(enemy);
        assert_eq!(e.find_path(&unit, from, row[1]).unwrap(), None);
    }

    #[test]
    fn occupied_destinations_and_unexplored_tiles_have_no_path() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let (from, id) = first_unit(&e, 1);
        // Moving onto a tile that holds a unit: the destination is not a legal single step.
        let other = free_field_next_to(&mut e, from);
        e.state.map.tile_mut(other).unwrap().unit = Some(e.state.map.tile(from).unwrap().unit.clone().unwrap());
        assert!(matches!(
            e.command_is_valid(&Command::Move { player: 1, from, to: other, unit_id: id }),
            Err(EngineError::Invalid { .. })
        ));
    }

    #[test]
    fn research_price_scales_with_cities_and_literacy_and_charges_the_player() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let archery = tech(&rules, "archery");
        let tier = rules.tech_by_game_idx(archery as u32).unwrap().cost;
        e.state.players[0].cities = 3;
        assert_eq!(e.tech_price(0, archery).unwrap(), tier * 3 + 4);
        e.state.players[0].currency = 100;
        e.state.players[0].available_tech.retain(|t| *t != archery);
        e.state.commands = vec![Command::Research { player: 1, tech: archery }];
        e.state.current_command = 0;
        e.update().unwrap(); // the command queues the ResearchAction (and RunDeferred)
        while e.update().is_ok() && !e.actions.is_empty() {}
        assert!(e.state.players[0].available_tech.contains(&archery));
        assert_eq!(e.state.players[0].currency, 100 - (tier * 3 + 4));
    }

    #[test]
    fn train_charges_the_cost_numbers_the_unit_and_scores_it() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let (city, _) = first_unit(&e, 1);
        e.state.map.tile_mut(city).unwrap().unit = None; // free the city tile
        let (stars, score, next_id) = (e.state.players[0].currency, e.state.players[0].score, e.state.current_unit_id);
        e.state.commands = vec![Command::Train { player: 1, kind: 2, at: city }]; // warrior, cost 2
        e.run().unwrap();
        let u = e.state.map.tile(city).unwrap().unit.as_ref().unwrap();
        assert_eq!((u.id, u.owner, u.kind, u.health, u.home), (next_id + 1, 1, 2, 100, city));
        assert!(u.moved && u.attacked, "a new unit cannot act this turn");
        assert_eq!(e.state.current_unit_id, next_id + 1);
        assert_eq!(e.state.players[0].currency, stars - 2);
        assert_eq!(e.state.players[0].score, score + 10, "unit score is 5 x cost");
    }

    #[test]
    fn training_on_an_occupied_tile_stops_loudly() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let (city, _) = first_unit(&e, 1); // the starting unit stands on the capital
        e.state.commands = vec![Command::Train { player: 1, kind: 2, at: city }];
        assert!(matches!(e.run(), Err(EngineError::Unimplemented { .. })));
    }

    #[test]
    fn run_deferred_clears_the_flag_and_queues_connections_then_routes() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        e.actions.clear();
        e.state.has_flagged_need_for_update_routes = true;
        e.run_deferred(3).unwrap();
        assert!(!e.state.has_flagged_need_for_update_routes);
        // routes run first (last queued), then the connections for the acting player
        assert_eq!(e.actions, vec![Action::UpdateCityConnections { players: vec![3] }, Action::UpdateRoutes]);
        // unflagged: nothing queued
        e.actions.clear();
        e.run_deferred(3).unwrap();
        assert!(e.actions.is_empty());
    }

    #[test]
    fn update_routes_resets_flags_and_connects_ports_over_water() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        e.state.map.tiles[10].has_route = true;
        e.update_routes().unwrap();
        assert!(e.state.map.tiles.iter().all(|t| !t.has_route));
        // Two of player 1's ports four tiles apart with explored, owned water between them.
        let port = rules.improvement(rules.improvement_id("port").unwrap()).game_idx as u16;
        e.state.players[0].available_tech.push(tech(&rules, "fishing"));
        let (y, x0) = (3, 4);
        for x in x0..=x0 + 4 {
            let t = e.state.map.tile_mut(Coords { x, y }).unwrap();
            t.terrain = crate::rules::Terrain::Water as u16;
            (t.owner, t.unit, t.improvement, t.resource, t.has_road) = (1, None, None, None, false);
            t.explorers = vec![1];
            if x == x0 || x == x0 + 4 {
                t.improvement = Some(ImprovementState {
                    kind: port, level: 0, founded: 0, xp: 0, population: 0, production: 1, base_score: 0, border_size: 0,
                    upgrade: 0, connected_to_capital_of_player: 0, name: String::new(), founder: 1, rewards: vec![], effects: vec![],
                });
            }
        }
        e.update_routes().unwrap();
        for x in x0..=x0 + 4 {
            assert!(e.state.map.tile(Coords { x, y }).unwrap().has_route, "route flag at x={x}");
        }
        assert_eq!(e.state.map.tiles.iter().filter(|t| t.has_route).count(), 5);
    }

    #[test]
    fn a_city_joined_to_the_capital_by_road_is_connected_and_both_grow() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        e.update_city_connections(&[1]).unwrap();
        assert!(e.actions.is_empty(), "one capital: nothing to connect");
        // give player 1 a second city two tiles from the capital with a road between them
        let capital = e.state.players[0].start_tile;
        let city = e.state.map.tiles.iter().find(|t| t.owner == 2 && t.improvement.as_ref().map(|i| i.kind == 1).unwrap_or(false)).unwrap().coordinates;
        let other = e.state.map.tile(city).unwrap().clone();
        let spot = Coords { x: capital.x + 2, y: capital.y };
        let mut moved = other;
        moved.coordinates = spot;
        moved.owner = 1;
        let mid = Coords { x: capital.x + 1, y: capital.y };
        e.state.map.tile_mut(city).unwrap().improvement = None;
        let keep = e.state.map.tile(spot).unwrap().clone();
        moved.terrain = keep.terrain;
        e.state.map.tile_mut(spot).unwrap().improvement = moved.improvement.clone();
        e.state.map.tile_mut(spot).unwrap().owner = 1;
        e.state.map.tile_mut(mid).unwrap().has_road = true;
        e.state.map.tile_mut(mid).unwrap().owner = 1;
        e.update_city_connections(&[1]).unwrap();
        assert_eq!(e.actions, vec![Action::ConnectCity { player: 1, at: spot }]);
        e.actions.clear();
        // without the road it is not connected
        e.state.map.tile_mut(mid).unwrap().has_road = false;
        e.update_city_connections(&[1]).unwrap();
        assert!(e.actions.is_empty());
    }

    // ---- EndTurn / StartTurn ----

    fn city_of(e: &Engine, owner: u8) -> Coords {
        e.state.map.tiles.iter().find(|t| t.owner == owner && t.improvement.as_ref().map(|i| i.kind == 1).unwrap_or(false)).unwrap().coordinates
    }

    /// A crop tile next to player 1's capital, owned by them, ready for a farm.
    fn crop_tile_next_to_capital(e: &mut Engine) -> (Coords, Coords) {
        let city = city_of(e, 1);
        let at = free_field_next_to(e, city);
        let t = e.state.map.tile_mut(at).unwrap();
        t.owner = 1;
        t.ruling_city = city;
        t.resource = Some(2); // crop
        (city, at)
    }

    #[test]
    fn building_a_farm_pays_scores_and_grows_the_city_which_levels_up_and_asks_for_a_reward() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let (city, at) = crop_tile_next_to_capital(&mut e);
        e.state.players[0].currency = 20;
        let score = e.state.players[0].score;
        let (level, xp) = {
            let c = e.state.map.tile(city).unwrap().improvement.as_ref().unwrap();
            (c.level, c.xp)
        };
        e.state.commands = vec![Command::Build { player: 1, kind: 5, at }];
        e.run().unwrap();
        let farm = e.state.map.tile(at).unwrap().improvement.as_ref().expect("farm built");
        assert_eq!((farm.kind, farm.level, farm.production, farm.founder), (5, 0, 1, 1));
        assert_eq!(e.state.players[0].currency, 20 - 5);
        // farm: +2 population on the ruling city. With level 1, xp 0 -> xp 2 > 1 -> level 2, xp 0.
        let c = e.state.map.tile(city).unwrap().improvement.as_ref().unwrap();
        assert_eq!((c.level, c.xp), (level + 1, xp + 2 - (level as i16 + 1)));
        assert!(e.state.triggers.iter().any(|t| t.player_id == 1 && t.kind == 1 && t.coordinates == city), "city reward pending");
        assert!(e.state.players[0].score > score);
    }

    #[test]
    fn a_city_reward_consumes_the_trigger_and_workshop_adds_production() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let (city, at) = crop_tile_next_to_capital(&mut e);
        e.state.players[0].currency = 20;
        e.state.commands = vec![Command::Build { player: 1, kind: 5, at }, Command::CityReward { player: 1, at: city, reward: 3 }];
        let production = e.state.map.tile(city).unwrap().improvement.as_ref().unwrap().production;
        e.run().unwrap();
        assert!(e.state.triggers.is_empty());
        let c = e.state.map.tile(city).unwrap().improvement.as_ref().unwrap();
        assert_eq!((c.production, c.rewards.clone()), (production + 1, vec![3]));
    }

    #[test]
    fn building_without_a_pending_reward_is_refused_for_city_reward() {
        let rules = Rules::bundled();
        let e = started(&rules);
        let city = city_of(&e, 1);
        assert!(e.command_is_valid(&Command::CityReward { player: 1, at: city, reward: 3 }).is_err());
    }

    #[test]
    fn hunting_consumes_the_resource_and_builds_no_improvement() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let (_, at) = crop_tile_next_to_capital(&mut e);
        e.state.map.tile_mut(at).unwrap().resource = Some(1); // game
        e.state.players[0].currency = 20;
        e.state.commands = vec![Command::Build { player: 1, kind: 9, at }];
        e.run().unwrap();
        let t = e.state.map.tile(at).unwrap();
        assert!(t.improvement.is_none() && t.resource.is_none());
        assert_eq!(e.state.players[0].currency, 20 - 2);
    }

    #[test]
    fn a_road_marks_the_tile_without_an_improvement() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let (_, at) = crop_tile_next_to_capital(&mut e);
        e.state.map.tile_mut(at).unwrap().resource = None;
        e.state.players[0].currency = 20;
        e.state.commands = vec![Command::Build { player: 1, kind: 3, at }];
        e.run().unwrap();
        let t = e.state.map.tile(at).unwrap();
        assert!(t.has_road && t.improvement.is_none());
    }

    #[test]
    fn the_explorer_reward_takes_a_unit_id_and_starts_a_scouting_chain() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let (city, _) = crop_tile_next_to_capital(&mut e);
        e.state.triggers.push(CommandTrigger { player_id: 1, kind: 1, coordinates: city, opponent_id: 0 });
        let id = e.state.current_unit_id;
        e.state.commands = vec![Command::CityReward { player: 1, at: city, reward: 4 }];
        e.update().unwrap(); // the command
        e.update().unwrap(); // CityRewardAction
        assert_eq!(e.state.current_unit_id, id + 1);
        let steps = e.actions.iter().filter(|a| matches!(a, Action::ScoutMove { remaining: 12, .. })).count();
        assert_eq!(steps, 1);
    }

    #[test]
    fn meeting_pays_stars_by_the_other_players_score_and_records_the_meeting() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        e.state.current_turn = 3;
        e.state.players[1].score = 2_600; // round(2.6) = 3
        let stars = e.state.players[0].currency;
        e.meet_action(1, 2, Coords { x: 0, y: 0 }).unwrap();
        while e.update().unwrap() != Progress::Idle {}
        assert!(e.state.players[0].known_players.contains(&2));
        let relation = &e.state.players[0].relations.iter().find(|(id, _)| *id == 2).unwrap().1;
        assert_eq!(relation.first_meet, 3);
        assert_eq!(e.state.players[0].currency, stars + 7, "2 * 3 + 1 stars");
        // Once met, a second MeetAction is no longer valid and is skipped.
        assert!(!e.action_is_valid(&Action::Meet { player: 1, other: 2, at: Coords { x: 0, y: 0 } }));
    }

    #[test]
    fn the_turn_counter_goes_up_only_when_the_player_order_wraps() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        assert_eq!((e.state.current_player_index, e.state.current_turn), (0, 0));
        let mut seen = Vec::new();
        for _ in 0..6 {
            e.end_player_turn();
            seen.push((e.state.current_player_index, e.state.current_turn));
        }
        // players 1..5 then Nature (index 5), then back to player 1 with turn 1
        assert_eq!(seen, vec![(1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (0, 1)]);
    }

    #[test]
    fn dead_players_are_skipped_and_nature_never_dies() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        // kill player 2 (index 1) by removing its cities
        let city = city_of(&e, 2);
        e.state.map.tile_mut(city).unwrap().improvement = None;
        assert!(!e.player_is_alive(1));
        e.end_player_turn();
        assert_eq!(e.state.current_player_index, 2, "player 2 is skipped");
        // Nature (255) has no cities or units and is still alive
        assert!(e.player_is_alive(5));
        // under the "units" rule a player lives while it has a unit
        e.state.settings.rules.player_death_condition = 1;
        assert!(e.player_is_alive(0));
        for t in &mut e.state.map.tiles {
            if t.unit.as_ref().map(|u| u.owner == 1).unwrap_or(false) {
                t.unit = None;
            }
        }
        assert!(!e.player_is_alive(0));
    }

    #[test]
    fn end_turn_gives_the_players_units_a_fresh_turn_and_heals_only_idle_hurt_ones() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let (city, id) = first_unit(&e, 1);
        // second unit next to the capital
        let other = free_field_next_to(&mut e, city);
        let mut u2 = e.state.map.tile(city).unwrap().unit.clone().unwrap();
        u2.id = 999;
        u2.coordinates = other;
        e.state.map.tile_mut(other).unwrap().unit = Some(u2);
        // unit 1: hurt, idle; unit 999: hurt but it moved, and frozen + petrified
        e.tile_unit_mut(city, id).unwrap().health = 60;
        {
            let u = e.tile_unit_mut(other, 999).unwrap();
            u.health = 60;
            u.moved = true;
            u.attacked = true;
            u.effects = vec![UNIT_EFFECT_FROZEN, UNIT_EFFECT_PETRIFIED, UNIT_EFFECT_POISONED];
        }
        e.end_turn_command(1).unwrap();
        assert_eq!(e.actions, vec![Action::EndTurn { player: 1 }, Action::Heal { player: 1, at: city, fixed: 0 }]);
        let u = e.tile_unit_mut(other, 999).unwrap();
        assert!(!u.moved && !u.attacked, "a fresh turn");
        assert_eq!(u.effects, vec![UNIT_EFFECT_POISONED], "frozen and petrified are lifted, poison stays");
        // other players' units are untouched
        let (_, id2) = first_unit(&e, 2);
        assert_ne!(id2, 0);
    }

    #[test]
    fn healing_is_40_at_home_20_away_capped_at_max_and_cures_poison_instead() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let (city, id) = first_unit(&e, 1); // warrior, max 100, standing on its own city tile
        e.tile_unit_mut(city, id).unwrap().health = 50;
        e.heal_action(1, city, 0).unwrap();
        assert_eq!(e.tile_unit_mut(city, id).unwrap().health, 90, "own territory heals 40");
        e.heal_action(1, city, 0).unwrap();
        assert_eq!(e.tile_unit_mut(city, id).unwrap().health, 100, "capped at the maximum");
        // away from home: 20
        e.state.map.tile_mut(city).unwrap().owner = 0;
        e.tile_unit_mut(city, id).unwrap().health = 50;
        e.heal_action(1, city, 0).unwrap();
        assert_eq!(e.tile_unit_mut(city, id).unwrap().health, 70);
        // a fixed amount overrides
        e.heal_action(1, city, 5).unwrap();
        assert_eq!(e.tile_unit_mut(city, id).unwrap().health, 75);
        // poison is cured and nothing else happens
        e.tile_unit_mut(city, id).unwrap().effects = vec![UNIT_EFFECT_POISONED];
        e.heal_action(1, city, 0).unwrap();
        let u = e.tile_unit_mut(city, id).unwrap();
        assert_eq!((u.health, u.effects.is_empty()), (75, true));
    }

    #[test]
    fn promotion_raises_max_health_by_50() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let (city, id) = first_unit(&e, 1);
        e.tile_unit_mut(city, id).unwrap().promotion_level = 1;
        let u = e.state.map.tile(city).unwrap().unit.clone().unwrap();
        assert_eq!(e.max_health(&u), 150);
    }

    #[test]
    fn end_turn_action_passes_the_turn_queues_start_turn_and_bumps_pacifist() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        e.actions.clear();
        e.state.players[0].tasks.push(Task { kind: TASK_PACIFIST, is_started: true, is_completed: false, counter: Some(2) });
        e.end_turn_action(1).unwrap();
        assert_eq!(e.state.players[0].tasks[0].counter, Some(3));
        assert_eq!(e.state.current_player_index, 1);
        assert_eq!(e.actions, vec![Action::StartTurn { player: 2 }]);
        // an end-turn for someone who is not the current player does not pass the turn
        e.actions.clear();
        e.end_turn_action(4).unwrap();
        assert_eq!(e.state.current_player_index, 1);
        assert_eq!(e.actions, vec![Action::StartTurn { player: 2 }]);
        // a completed Pacifist task stops counting
        e.state.players[0].tasks[0].is_completed = true;
        e.end_turn_action(1).unwrap();
        assert_eq!(e.state.players[0].tasks[0].counter, Some(3));
    }

    #[test]
    fn end_turn_action_clears_improvement_effect_1_on_the_players_tiles() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let city = city_of(&e, 1);
        e.state.map.tile_mut(city).unwrap().improvement.as_mut().unwrap().effects = vec![1, 7];
        e.end_turn_action(1).unwrap();
        assert_eq!(e.state.map.tile(city).unwrap().improvement.as_ref().unwrap().effects, vec![7]);
    }

    #[test]
    fn the_match_ending_queues_game_over() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        for owner in [2, 3, 4, 5] {
            let city = city_of(&e, owner);
            e.state.map.tile_mut(city).unwrap().improvement = None;
        }
        e.end_turn_action(1).unwrap();
        assert!(e.actions.iter().any(|a| matches!(a, Action::GameOver { .. })));
    }

    #[test]
    fn a_city_earns_production_plus_level_minus_one_plus_handicap_when_the_capital() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        let city = city_of(&e, 1);
        let idx = e.state.map.index(city).unwrap();
        let imp = e.state.map.tiles[idx].improvement.as_mut().unwrap();
        imp.production = 2;
        imp.level = 3;
        imp.xp = 0;
        e.state.players[0].handicap = 0;
        assert_eq!(e.tile_income(idx).unwrap(), 4, "2 + 3 - 1");
        e.state.players[0].handicap = 1;
        assert_eq!(e.tile_income(idx).unwrap(), 5, "capital handicap bonus");
        e.state.map.tiles[idx].improvement.as_mut().unwrap().xp = -2;
        assert_eq!(e.tile_income(idx).unwrap(), 3, "negative xp is subtracted");
        // an enemy unit standing on the city blocks the income
        let mut enemy = e.state.map.tiles.iter().find_map(|t| t.unit.clone().filter(|u| u.owner == 2)).unwrap();
        enemy.coordinates = city;
        e.state.map.tiles[idx].unit = Some(enemy);
        assert_eq!(e.tile_income(idx).unwrap(), 0);
    }

    #[test]
    fn start_turn_queues_income_from_turn_one_but_not_on_turn_zero() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        e.actions.clear();
        e.start_turn_action(1).unwrap();
        assert!(e.actions.is_empty(), "turn 0: no income");
        e.state.current_turn = 1;
        e.start_turn_action(1).unwrap();
        let income: i32 = e.actions.iter().map(|a| if let Action::IncreaseCurrency { amount, .. } = a { *amount } else { 0 }).sum();
        assert!(income > 0, "the capital pays out on turn 1");
        let stars = e.state.players[0].currency;
        e.actions.retain(|a| matches!(a, Action::IncreaseCurrency { .. }));
        while e.update().unwrap() != Progress::Idle {}
        assert_eq!(e.state.players[0].currency, stars + income);
    }

    #[test]
    fn start_turn_queues_a_meeting_for_an_explored_unmet_player() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        e.state.current_turn = 1;
        let enemy_city = city_of(&e, 2);
        e.state.map.tile_mut(enemy_city).unwrap().explorers.push(1); // player 1 has explored player 2's city
        e.start_turn_action(1).unwrap();
        assert!(e.actions.iter().any(|a| matches!(a, Action::Meet { player: 1, other: 2, .. })));
        e.actions.clear();
        e.state.players[0].known_players.push(2);
        e.start_turn_action(1).unwrap();
        assert!(!e.actions.iter().any(|a| matches!(a, Action::Meet { .. })), "once met nothing is queued");
    }

    #[test]
    fn start_turn_stops_on_embassy_relations_and_spreading_climates() {
        let rules = Rules::bundled();
        let mut e = started(&rules);
        e.state.current_turn = 1;
        let mut relation = DiplomacyRelation::default_relation();
        relation.embassy_level = 1;
        e.state.players[0].relations.push((2, relation));
        assert!(matches!(e.start_turn_action(1), Err(EngineError::Unimplemented { .. })));
        e.state.players[0].relations.clear();
        let polaris = rules.tribe(rules.tribe_id("polaris").unwrap()).game_idx as i16;
        let city = city_of(&e, 1);
        e.state.map.tile_mut(city).unwrap().climate = polaris;
        assert!(matches!(e.start_turn_action(1), Err(EngineError::Unimplemented { .. })));
    }
}
