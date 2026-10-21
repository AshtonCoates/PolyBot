//! Engine behaviour against the fixtures: replays that must match exactly, and the execution model.

use polytopia_engine::actions::Action;
use polytopia_engine::engine::{Engine, Progress};
use polytopia_engine::replay::replay;
use polytopia_engine::rules::Rules;
use polytopia_engine::savestate::load_state_file;

fn load(name: &str) -> polytopia_engine::state::LocalGameData {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../testdata/savestates").join(name);
    load_state_file(&std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

/// Fixtures whose commands are all implemented must replay to exactly the saved state.
#[test]
fn start_game_1_replays_exactly() {
    let out = replay(&load("start_game_1.state"), &Rules::bundled());
    assert!(out.matches(), "error {:?}, {} diffs: {:#?}", out.error, out.diffs.len(), out.diffs.iter().take(10).collect::<Vec<_>>());
    assert_eq!((out.executed, out.total), (1, 1));
}

/// `Move` (onto a mountain, with fog of war, explore scoring and the deferred route update) and `Train`.
#[test]
fn train_1_replays_exactly() {
    let out = replay(&load("train_1.state"), &Rules::bundled());
    assert!(out.matches(), "error {:?}, {} diffs: {:#?}", out.error, out.diffs.len(), out.diffs.iter().take(10).collect::<Vec<_>>());
    assert_eq!((out.executed, out.total), (3, 3));
}

/// A six-turn, three-AI game: Research, Build, CityReward, Train, multi-step Move (A*), EndTurn / StartTurn for several players.
#[test]
fn pass_turn_1_replays_exactly() {
    let out = replay(&load("pass_turn_1.state"), &Rules::bundled());
    assert!(out.matches(), "error {:?}, {} diffs: {:#?}", out.error, out.diffs.len(), out.diffs.iter().take(10).collect::<Vec<_>>());
    assert_eq!((out.executed, out.total), (19, 19));
}

/// A 12-turn v122 game that includes the Explorer city reward (ScoutMoveAction, FastPath fog search, XXHash).
#[test]
fn early_game_turn2_replays_exactly() {
    let out = replay(&load("early_game_turn2.state"), &Rules::bundled());
    assert!(out.matches(), "error {:?}, {} diffs: {:#?}", out.error, out.diffs.len(), out.diffs.iter().take(10).collect::<Vec<_>>());
    assert_eq!((out.executed, out.total), (41, 41));
}

/// Capturing villages: city names, border claims, scores, city connections and route building.
#[test]
fn capture_1_replays_exactly() {
    let out = replay(&load("capture_1.state"), &Rules::bundled());
    assert!(out.matches(), "error {:?}, {} diffs: {:#?}", out.error, out.diffs.len(), out.diffs.iter().take(10).collect::<Vec<_>>());
    assert_eq!((out.executed, out.total), (67, 67));
}

/// Combat, retaliation, kills, moving into the defeated tile, border growth and the Super Unit reward.
#[test]
fn attack_1_replays_exactly() {
    let out = replay(&load("attack_1.state"), &Rules::bundled());
    assert!(out.matches(), "error {:?}, {} diffs: {:#?}", out.error, out.diffs.len(), out.diffs.iter().take(10).collect::<Vec<_>>());
    assert_eq!((out.executed, out.total), (99, 99));
}

/// Building (mines, forges, roads), upgrading units and one attack.
#[test]
fn build_mine_upgrade_replays_exactly() {
    let out = replay(&load("build_mine_upgrade.state"), &Rules::bundled());
    assert!(out.matches(), "error {:?}, {} diffs: {:#?}", out.error, out.diffs.len(), out.diffs.iter().take(10).collect::<Vec<_>>());
    assert_eq!((out.executed, out.total), (75, 75));
}

#[test]
fn start_match_gives_each_player_their_tribes_starting_techs_and_score() {
    let save = load("start_game_1.state");
    let rules = Rules::bundled();
    let mut start = save.initial.clone();
    start.commands = save.current.commands.clone();
    start.current_command = 0;
    assert!(start.players.iter().all(|p| p.available_tech.is_empty()));
    let scores: Vec<u32> = start.players.iter().map(|p| p.score).collect();

    let mut engine = Engine::new(start, &rules);
    engine.run().unwrap();

    for (i, p) in engine.state.players.iter().enumerate() {
        let tribe = rules.tribe_by_game_idx(p.tribe as u32).unwrap();
        let expected: Vec<u16> = tribe.starting_techs.iter().map(|t| rules.tech(*t).game_idx as u16).collect();
        assert_eq!(p.available_tech, expected, "player {}", p.id);
        let gained: i32 = expected.iter().map(|t| rules.tech_by_game_idx(*t as u32).unwrap().cost * 100).sum();
        assert_eq!(p.score as i32 - scores[i] as i32, gained, "score of player {}", p.id);
    }
    assert_eq!(engine.state.current_state, 2, "game is Started");
    // aimo's starting tech unlocks the Genius task (task 2)
    let aimo = engine.state.players.iter().find(|p| rules.tribe_by_game_idx(p.tribe as u32).unwrap().name == "aimo").unwrap();
    assert_eq!(aimo.tasks.len(), 1);
    assert_eq!((aimo.tasks[0].kind, aimo.tasks[0].is_started, aimo.tasks[0].is_completed), (2, true, false));
}

/// Commands only queue actions; actions run one per update, last queued first. `CommandBase.Execute` queues a
/// `RunDeferred` action first, so it sits at the bottom of the stack and runs after the command's own actions.
#[test]
fn execution_model_command_then_actions() {
    let save = load("start_game_1.state");
    let rules = Rules::bundled();
    let mut start = save.initial.clone();
    start.commands = save.current.commands.clone();
    start.current_command = 0;
    let mut engine = Engine::new(start, &rules);

    assert_eq!(engine.update().unwrap(), Progress::RanCommand);
    assert_eq!(engine.state.current_command, 1);
    assert_eq!(engine.actions, vec![Action::RunDeferred { player: 1 }, Action::StartMatch { player: 1 }]);
    assert_eq!(engine.state.current_state, 0, "the command itself does not change the state");

    assert_eq!(engine.update().unwrap(), Progress::RanAction); // StartMatch runs (last queued), queues EnableTask(aimo, genius)
    assert_eq!(engine.state.current_state, 2);
    assert_eq!(engine.actions.len(), 2);
    assert!(matches!(engine.actions[0], Action::RunDeferred { .. }));
    assert!(matches!(engine.actions[1], Action::EnableTask { kind: 2, .. }));

    assert_eq!(engine.update().unwrap(), Progress::RanAction); // EnableTask runs
    assert_eq!(engine.update().unwrap(), Progress::RanAction); // RunDeferred runs last
    assert!(engine.actions.is_empty());
    assert_eq!(engine.update().unwrap(), Progress::Idle);
}

#[test]
fn replaying_twice_gives_the_same_result() {
    let save = load("start_game_1.state");
    let rules = Rules::bundled();
    let (a, b) = (replay(&save, &rules), replay(&save, &rules));
    assert!(a.matches() && b.matches());
}

/// A full 1v1 game (370 commands): boats, ruins, upgrades, stomp, wiping out a player and the game ending.
#[test]
fn full_game_1_replays_exactly() {
    let out = replay(&load("full_game_1.state"), &Rules::bundled());
    assert!(out.matches(), "error {:?}, {} diffs: {:#?}", out.error, out.diffs.len(), out.diffs.iter().take(10).collect::<Vec<_>>());
    assert_eq!((out.executed, out.total), (370, 370));
}

/// A five-player game (430 commands): ruins rewards, meetings, adjacency levels, free techs.
#[test]
fn metropolis_replays_exactly() {
    let out = replay(&load("metropolis.state"), &Rules::bundled());
    assert!(out.matches(), "error {:?}, {} diffs: {:#?}", out.error, out.diffs.len(), out.diffs.iter().take(10).collect::<Vec<_>>());
    assert_eq!((out.executed, out.total), (430, 430));
}
