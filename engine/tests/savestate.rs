//! Tests against the captured sample savestate in `captures/` (git-ignored; tests skip if absent).

use polytopia_engine::diff::{diff_game_states, summarize};
use polytopia_engine::savestate::{load_state_file, unpickle, write_local_game_data, read_local_game_data};

fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../testdata/savestates")
}

/// Every `*.state` fixture, sorted by name.
fn all_fixtures() -> Vec<(String, Vec<u8>)> {
    let mut paths: Vec<_> = std::fs::read_dir(fixtures_dir())
        .expect("testdata/savestates exists")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "state").unwrap_or(false))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read(&p).unwrap()))
        .collect()
}

/// The small early-game fixture the detailed tests below are written against.
fn sample() -> Option<Vec<u8>> {
    std::fs::read(fixtures_dir().join("early_game_turn2.state")).ok()
}

macro_rules! need_sample {
    () => {
        match sample() {
            Some(b) => b,
            None => panic!("fixture testdata/savestates/early_game_turn2.state is missing"),
        }
    };
}

/// Fixtures are added with `import-savestate`. A fixture that uses a command/action we cannot decode
/// yet fails here with the reason; that is the to-do list.
#[test]
fn every_fixture_loads_and_round_trips_exactly() {
    let fixtures = all_fixtures();
    assert!(!fixtures.is_empty(), "no fixtures in testdata/savestates");
    let mut failures = Vec::new();
    for (name, bytes) in &fixtures {
        let payload = match unpickle(bytes) {
            Ok(p) => p,
            Err(e) => {
                failures.push(format!("{name}: {e}"));
                continue;
            }
        };
        match read_local_game_data(&payload) {
            Err(e) => failures.push(format!("{name}: {e}")),
            Ok(save) => {
                if write_local_game_data(&save) != payload {
                    failures.push(format!("{name}: re-serialized bytes differ from the original"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "fixtures that do not load/round-trip:\n  {}", failures.join("\n  "));
}

#[test]
fn parses_the_sample() {
    let save = load_state_file(&need_sample!()).expect("parse");
    assert_eq!(save.version, 122);
    assert_eq!(save.initial.current_turn, 0);
    assert!(save.initial.commands.is_empty());
    assert_eq!(save.current.commands.len(), save.last_seen_command as usize);
    assert_eq!(save.current.map.tiles.len(), save.current.map.width as usize * save.current.map.height as usize);
    assert_eq!(save.initial.seed, save.current.seed);
    assert_eq!(save.current.players.len(), 6);
    assert_eq!(save.current.players.last().unwrap().id, 255); // Nature
}

#[test]
fn write_is_the_exact_inverse_of_read() {
    let payload = unpickle(&need_sample!()).expect("unpickle");
    let save = read_local_game_data(&payload).expect("parse");
    assert_eq!(write_local_game_data(&save), payload, "re-serialized bytes differ from the original");
}

#[test]
fn diffing_a_state_with_itself_is_empty() {
    let save = load_state_file(&need_sample!()).unwrap();
    assert!(diff_game_states(&save.current, &save.current).is_empty());
}

#[test]
fn initial_and_current_differ_in_the_expected_places() {
    let save = load_state_file(&need_sample!()).unwrap();
    let diffs = diff_game_states(&save.current, &save.initial);
    let cats = summarize(&diffs);
    assert!(cats.contains_key("header.turn"));
    assert!(cats.contains_key("commands"));
    assert!(cats.contains_key("tile.unit"));
    assert!(cats.contains_key("player.available_tech"));
    // The map terrain never changes during the sample.
    assert!(!cats.contains_key("tile.terrain"));
}

#[test]
fn truncated_input_is_an_error_not_a_panic() {
    let mut payload = unpickle(&need_sample!()).unwrap();
    payload.truncate(payload.len() - 5);
    assert!(read_local_game_data(&payload).is_err());
}
