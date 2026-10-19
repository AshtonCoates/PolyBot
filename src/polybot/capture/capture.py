"""Interactively capture Polytopia savestate snapshots for reverse engineering.

Usage:
    uv run capture-gamestate

Flow:
    1. Pick which Polytopia savestate ("game id") to watch.
    2. Loop: play a move in-game, save to the main menu (the state file is
       only written to disk on save), then hit Enter here to capture the
       current raw bytes along with a description of what you just did.
    3. Type 'q' at the description prompt (or Ctrl+C) to stop. Captures are
       written to disk after every step, so nothing is lost if you quit.

Output:
    ./captures/<game_id>_<timestamp>.pkl (relative to the current working
    directory) -- a pickled list of dicts:
        {
            "index": int,
            "description": str,
            "timestamp": str (ISO 8601),
            "size": int,
            "sha256": str,
            "raw": bytes,
        }
    Raw bytes are stored rather than a hex string so downstream analysis can
    derive hex, ints, etc. as needed without re-reading the original file.
"""

import hashlib
import pathlib
import pickle
import sys
from datetime import datetime, timezone

POLYTOPIA_ROOT = pathlib.Path.home() / ".config/unity3d/Midjiwan/Polytopia"
OUTPUT_DIR = pathlib.Path.cwd() / "captures"


def find_singleplayer_dir() -> pathlib.Path:
    candidates = sorted(POLYTOPIA_ROOT.glob("*/Data/Singleplayer"))
    if not candidates:
        sys.exit(f"No Singleplayer save directory found under {POLYTOPIA_ROOT}")
    if len(candidates) == 1:
        return candidates[0]

    print("Multiple Polytopia profiles found:")
    for i, path in enumerate(candidates):
        print(f"  [{i}] {path}")
    choice = int(input("Select profile index: ").strip())
    return candidates[choice]


def choose_state_file(singleplayer_dir: pathlib.Path) -> pathlib.Path:
    state_files = sorted(singleplayer_dir.glob("*.state"))
    if not state_files:
        sys.exit(f"No .state files found in {singleplayer_dir}")

    print("Available savestates:")
    for i, path in enumerate(state_files):
        size_kb = path.stat().st_size / 1024
        print(f"  [{i}] {path.stem} ({size_kb:.1f} KB)")

    raw = input(
        "Enter savestate id, an index above, or leave blank to pick [0]: "
    ).strip()

    if raw == "":
        return state_files[0]
    if raw.isdigit() and int(raw) < len(state_files):
        return state_files[int(raw)]

    candidate = singleplayer_dir / f"{raw}.state"
    if not candidate.exists():
        sys.exit(f"No savestate found matching '{raw}' in {singleplayer_dir}")
    return candidate


def capture_loop(state_path: pathlib.Path) -> list[dict]:
    captures: list[dict] = []
    last_hash = None

    print(f"\nWatching: {state_path}")
    print("Play a move, save to the main menu, then press Enter here to capture.")
    print("Type 'q' instead of a description at any point to stop.\n")

    while True:
        input(f"[{len(captures)}] Press Enter once you've saved to the menu...")

        raw = state_path.read_bytes()
        digest = hashlib.sha256(raw).hexdigest()

        if digest == last_hash:
            print("  (warning: bytes are identical to the last capture -- "
                  "did you forget to save to the main menu?)")

        description = input(f"[{len(captures)}] What move did you just make? ('q' to quit): ").strip()
        if description.lower() == "q":
            break

        captures.append({
            "index": len(captures),
            "description": description,
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "size": len(raw),
            "sha256": digest,
            "raw": raw,
        })
        last_hash = digest

        save_captures(captures, state_path)
        print(f"  captured {len(raw)} bytes (total captures: {len(captures)})\n")

    return captures


def save_captures(captures: list[dict], state_path: pathlib.Path) -> pathlib.Path:
    OUTPUT_DIR.mkdir(exist_ok=True)
    if not captures:
        return OUTPUT_DIR

    if not hasattr(save_captures, "_out_path"):
        game_id = state_path.stem
        ts = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        save_captures._out_path = OUTPUT_DIR / f"{game_id}_{ts}.pkl"

    out_path = save_captures._out_path
    with open(out_path, "wb") as f:
        pickle.dump(captures, f)
    return out_path


def main():
    singleplayer_dir = find_singleplayer_dir()
    state_path = choose_state_file(singleplayer_dir)
    captures = capture_loop(state_path)

    if not captures:
        print("No captures recorded, nothing saved.")
        return

    out_path = save_captures(captures, state_path)
    print(f"\nSaved {len(captures)} captures to {out_path}")


if __name__ == "__main__":
    main()
