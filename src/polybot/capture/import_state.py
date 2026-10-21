"""Copy a Polytopia savestate from the game's directory into the repo's test fixtures.

Usage:
    uv run import-savestate NAME [--game ID_OR_INDEX] [--dest DIR] [--force]
    uv run import-savestate --list

NAME is the descriptive file name to give the fixture (e.g. `full_game_1`; `.state` is added). By default the
most recently modified save is used, so run it right after saving to the main menu. Fixtures go to
./testdata/savestates relative to the current directory.

After copying, the file is parsed with the Python savestate parser and a short summary is printed. A file the
parser cannot decode yet is still kept (that is what the fixture is for), with a warning.
"""

from __future__ import annotations

import argparse
import hashlib
import pathlib
import re
import sys
from collections import Counter
from datetime import datetime

from polybot.capture.capture import POLYTOPIA_ROOT, find_singleplayer_dir

DEFAULT_DEST = pathlib.Path("testdata") / "savestates"
NAME_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]*$")
ZERO_GUID = "00000000-0000-0000-0000-000000000000"


def clean_name(name: str) -> str:
    """Validate a fixture name and return it with the `.state` extension."""
    stem = name[: -len(".state")] if name.endswith(".state") else name
    if not NAME_RE.match(stem):
        raise ValueError(
            f"bad name {name!r}: use letters, digits, '_', '-', '.' (must start with a letter or digit)"
        )
    return f"{stem}.state"


def list_saves(source_dir: pathlib.Path) -> list[pathlib.Path]:
    """Savestates in the directory, newest first."""
    return sorted(source_dir.glob("*.state"), key=lambda p: p.stat().st_mtime, reverse=True)


def pick_save(source_dir: pathlib.Path, game: str | None) -> pathlib.Path:
    saves = list_saves(source_dir)
    if not saves:
        raise FileNotFoundError(f"no .state files in {source_dir}")
    if game is None:
        return saves[0]
    if game.isdigit() and int(game) < len(saves):
        return saves[int(game)]
    for path in saves:
        if path.stem == game or path.name == game:
            return path
    raise FileNotFoundError(f"no save matching {game!r} in {source_dir} (try --list)")


def describe(raw: bytes) -> list[str]:
    """Lines summarising the save, or a warning if it cannot be decoded."""
    from polybot.parsing.savestate import parse_savestate

    try:
        g = parse_savestate(raw)
    except Exception as e:  # unsupported commands/actions, other versions, corrupt files
        return [f"WARNING: the parser cannot decode this file yet ({type(e).__name__}: {e})"]
    cur = g.current
    head = cur.head
    names = Counter(c.name for c in cur.commands)
    lines = [
        f"version {g.version} | game {head.settings.game_name!r} | map {head.map.width}x{head.map.height} "
        f"| turn {head.current_turn}",
        f"players: {', '.join(p.user_name for p in cur.players if p.id != 255)}",
        f"commands: {len(cur.commands)} {dict(names)}",
    ]
    if any(p.account_id not in ("", ZERO_GUID) for p in cur.players):
        lines.append("WARNING: contains real account ids; think before committing this file")
    return lines


def import_state(source: pathlib.Path, dest_dir: pathlib.Path, name: str, force: bool = False) -> pathlib.Path:
    dest = dest_dir / clean_name(name)
    if dest.exists() and not force:
        raise FileExistsError(f"{dest} already exists (use --force to overwrite)")
    raw = source.read_bytes()
    dest_dir.mkdir(parents=True, exist_ok=True)
    tmp = dest.with_suffix(".state.tmp")
    tmp.write_bytes(raw)
    tmp.replace(dest)
    if hashlib.sha256(dest.read_bytes()).digest() != hashlib.sha256(raw).digest():
        raise OSError(f"copy to {dest} did not verify")
    return dest


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Copy a Polytopia savestate into testdata/savestates.")
    ap.add_argument("name", nargs="?", help="descriptive fixture name, e.g. full_game_1")
    ap.add_argument("--game", help="save to import: game id or index from --list (default: newest)")
    ap.add_argument("--list", action="store_true", help="list the saves in the game directory and exit")
    ap.add_argument("--dest", type=pathlib.Path, default=DEFAULT_DEST, help=f"destination (default {DEFAULT_DEST})")
    ap.add_argument("--source-dir", type=pathlib.Path, help=f"directory of .state files (default: auto-detect under {POLYTOPIA_ROOT})")
    ap.add_argument("--force", action="store_true", help="overwrite an existing fixture")
    args = ap.parse_args(argv)

    try:
        source_dir = args.source_dir or find_singleplayer_dir()
        if args.list:
            for i, path in enumerate(list_saves(source_dir)):
                st = path.stat()
                when = datetime.fromtimestamp(st.st_mtime).strftime("%Y-%m-%d %H:%M:%S")
                print(f"[{i}] {path.stem}  {st.st_size / 1024:.1f} KB  modified {when}")
            return 0
        if not args.name:
            ap.error("a NAME is required (or use --list)")
        source = pick_save(source_dir, args.game)
        dest = import_state(source, args.dest, args.name, args.force)
    except (ValueError, FileNotFoundError, FileExistsError, OSError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1

    print(f"copied {source.name} -> {dest} ({dest.stat().st_size} bytes)")
    for line in describe(dest.read_bytes()):
        print(f"  {line}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
