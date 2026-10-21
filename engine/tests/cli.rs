//! Runs the real `replay` binary against the committed fixtures.

use std::process::{Command, Output};

fn replay(args: &[&str]) -> Output {
    // Run from the repo root so the default testdata/savestates path resolves.
    Command::new(env!("CARGO_BIN_EXE_replay"))
        .args(args)
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
        .output()
        .expect("run replay")
}

fn text(o: &Output) -> (String, String) {
    (String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

#[test]
fn help_lists_every_option_and_exits_zero() {
    let o = replay(&["--help"]);
    assert_eq!(o.status.code(), Some(0));
    let (out, _) = text(&o);
    for opt in ["--info", "--diff-initial", "--limit", "--all", "--rules", "--quiet", "--version", "Examples:", "Exit status"] {
        assert!(out.contains(opt), "--help is missing {opt}:\n{out}");
    }
}

#[test]
fn unknown_option_is_a_usage_error() {
    let o = replay(&["--diff-intial"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(text(&o).1.contains("unexpected argument"), "{:?}", text(&o));
}

#[test]
fn conflicting_options_are_rejected() {
    assert_eq!(replay(&["--info", "--diff-initial"]).status.code(), Some(2));
    assert_eq!(replay(&["--limit", "5", "--all"]).status.code(), Some(2));
}

#[test]
fn info_summarizes_and_succeeds() {
    let o = replay(&["--info", "testdata/savestates/early_game_turn2.state"]);
    assert_eq!(o.status.code(), Some(0));
    let (out, _) = text(&o);
    assert!(out.contains("version 122") && out.contains("commands (41)"), "{out}");
    assert!(!out.contains("replay:"), "--info must not replay:\n{out}");
}

#[test]
fn replay_succeeds_for_an_exact_fixture() {
    let o = replay(&["testdata/savestates/full_game_1.state"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(text(&o).0.contains("matches the saved state exactly"), "{}", text(&o).0);
}

#[test]
fn quiet_hides_the_summary_but_not_the_result() {
    let o = replay(&["-q", "testdata/savestates/full_game_1.state"]);
    let (out, _) = text(&o);
    assert!(!out.contains("players:") && out.contains("/370 commands executed"), "{out}");
}

#[test]
fn diff_initial_lists_differences_and_honours_limit() {
    let o = replay(&["--diff-initial", "--limit", "3", "testdata/savestates/early_game_turn2.state"]);
    assert_eq!(o.status.code(), Some(0));
    let (out, _) = text(&o);
    assert!(out.contains("98 differences") && out.contains("and 95 more (use --all)"), "{out}");
}

#[test]
fn no_arguments_runs_all_fixtures() {
    let o = replay(&["-q"]);
    let (out, _) = text(&o);
    assert!(out.contains("== testdata/savestates/early_game_turn2.state"), "{out}");
}

#[test]
fn missing_file_is_an_io_error() {
    let o = replay(&["does_not_exist.state"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(text(&o).1.contains("cannot read"));
}

#[test]
fn garbage_file_is_a_failure_not_a_crash() {
    let dir = std::env::temp_dir().join(format!("replay_cli_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("bad.state");
    std::fs::write(&bad, b"\x00not a save").unwrap();
    let o = replay(&[bad.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(1));
    assert!(text(&o).0.contains("cannot load"));
    std::fs::remove_file(&bad).unwrap();
    std::fs::remove_dir(&dir).unwrap();
}
