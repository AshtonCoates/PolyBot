//! Inspect Polytopia `.state` files and replay their commands with the engine.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use polytopia_engine::diff::{diff_game_states, summarize, Diff};
use polytopia_engine::replay::replay;
use polytopia_engine::rules::Rules;
use polytopia_engine::savestate::load_state_file;
use polytopia_engine::state::*;

const DEFAULT_DIR: &str = "testdata/savestates";

const EXAMPLES: &str = "\
Examples:
  replay                                  replay every file in testdata/savestates
  replay testdata/savestates/full_game_1.state
  replay --info testdata/savestates/*.state
  replay --diff-initial --all full_game_1.state    every difference between initial and current state

Exit status: 0 if every file loaded and (unless --info/--diff-initial) replayed to exactly the saved
state; 1 if any file failed to load or its replay did not match; 2 for usage or I/O errors.";

/// Replay saved Polytopia games with the engine and compare against the game's own result.
///
/// A .state file holds the initial game state, the full command history and the final state. This
/// tool loads each file, re-runs the commands from the initial state, and reports whether the engine
/// reproduces the saved final state.
#[derive(Parser, Debug)]
#[command(name = "replay", version, after_help = EXAMPLES)]
struct Cli {
    /// .state files to process [default: every *.state in testdata/savestates]
    files: Vec<PathBuf>,

    /// Only print a summary of each save (no replay)
    #[arg(long, conflicts_with = "diff_initial")]
    info: bool,

    /// Print how the saved initial and current states differ instead of replaying
    #[arg(long)]
    diff_initial: bool,

    /// Maximum number of differences to list
    #[arg(long, value_name = "N", default_value_t = 20, conflicts_with = "all")]
    limit: usize,

    /// List every difference
    #[arg(long)]
    all: bool,

    /// Use rule tables from this JSON file instead of the bundled GameLogicData28
    #[arg(long, value_name = "PATH")]
    rules: Option<PathBuf>,

    /// Skip the save summary when replaying
    #[arg(short, long)]
    quiet: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let rules = match &cli.rules {
        None => Rules::bundled(),
        Some(path) => match std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|s| Rules::from_json(&s).map_err(|e| e.to_string())) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error: cannot load rules from {}: {e}", path.display());
                return ExitCode::from(2);
            }
        },
    };

    let files = if cli.files.is_empty() {
        match default_files() {
            Ok(f) => f,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
        }
    } else {
        cli.files.clone()
    };

    let limit = if cli.all { usize::MAX } else { cli.limit };
    let (mut ok, mut failed) = (0usize, 0usize);
    for (i, path) in files.iter().enumerate() {
        if i > 0 {
            println!();
        }
        println!("== {}", path.display());
        match process(&cli, path, &rules, limit) {
            Outcome::Ok => ok += 1,
            Outcome::Failed => failed += 1,
            Outcome::Io(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
        }
    }
    if files.len() > 1 {
        println!("\n{} files: {ok} ok, {failed} failed", files.len());
    }
    if failed > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn default_files() -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(DEFAULT_DIR)
        .map_err(|e| format!("no files given and cannot read {DEFAULT_DIR} (run from the repo root): {e}"))?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "state").unwrap_or(false))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!("no .state files in {DEFAULT_DIR}"));
    }
    Ok(files)
}

enum Outcome {
    Ok,
    Failed,
    Io(String),
}

fn process(cli: &Cli, path: &Path, rules: &Rules, limit: usize) -> Outcome {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return Outcome::Io(format!("cannot read {}: {e}", path.display())),
    };
    let save = match load_state_file(&bytes) {
        Ok(s) => s,
        Err(e) => {
            println!("cannot load: {e}");
            return Outcome::Failed;
        }
    };

    // --quiet only applies to a plain replay; --info and --diff-initial exist to show things.
    if cli.info || cli.diff_initial || !cli.quiet {
        summarize_save(&save, rules);
    }
    if cli.info {
        return Outcome::Ok;
    }

    if cli.diff_initial {
        let diffs = diff_game_states(&save.current, &save.initial);
        println!("\ninitial vs current: {} differences", diffs.len());
        print_diffs(&diffs, limit);
        return Outcome::Ok;
    }

    let outcome = replay(&save, rules);
    println!("\nreplay: {}/{} commands executed", outcome.executed, outcome.total);
    if let Some(e) = &outcome.error {
        println!("stopped at command #{}: {e}", outcome.executed);
        if let Some(command) = save.current.commands.get(outcome.executed) {
            println!("  {command:?}");
        }
        return Outcome::Failed;
    }
    if outcome.diffs.is_empty() {
        println!("final state matches the saved state exactly");
        return Outcome::Ok;
    }
    println!("final state differs from the saved state: {} differences", outcome.diffs.len());
    print_diffs(&outcome.diffs, limit);
    Outcome::Failed
}

fn print_diffs(diffs: &[Diff], limit: usize) {
    for (category, n) in summarize(diffs) {
        println!("  {category}: {n}");
    }
    if diffs.is_empty() {
        return;
    }
    println!();
    for d in diffs.iter().take(limit) {
        println!("  {d}");
    }
    if diffs.len() > limit {
        println!("  ... and {} more (use --all)", diffs.len() - limit);
    }
}

fn summarize_save(save: &LocalGameData, rules: &Rules) {
    let cur = &save.current;
    let s = &cur.settings;
    println!("version {} | game \"{}\" | map {}x{} | seed {}", save.version, s.game_name, cur.map.width, cur.map.height, cur.seed);
    println!(
        "initial: turn {} | current: turn {}, player index {}, command {}, next unit id {}",
        save.initial.current_turn, cur.current_turn, cur.current_player_index, cur.current_command, cur.current_unit_id
    );
    println!("players:");
    for p in &cur.players {
        let tribe = rules.tribe_by_game_idx(p.tribe as u32).map(|t| t.name.as_str()).unwrap_or("?");
        println!(
            "  [{}] {:<16} {:<10} stars {:>3} score {:>4} cities {} techs {:?}",
            p.id, p.user_name, tribe, p.currency, p.score, p.cities, p.available_tech
        );
    }
    let mut hist: BTreeMap<&str, usize> = BTreeMap::new();
    for c in &cur.commands {
        *hist.entry(c.name()).or_insert(0) += 1;
    }
    println!("commands ({}): {:?}", cur.commands.len(), hist);
    let units = cur.map.tiles.iter().filter(|t| t.unit.is_some()).count();
    let cities = cur
        .map
        .tiles
        .iter()
        .filter(|t| t.improvement.as_ref().map(|i| i.kind == 1).unwrap_or(false))
        .count();
    println!("map: {} tiles, {} units, {} cities", cur.map.tiles.len(), units, cities);
}
