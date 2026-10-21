"""Tests for the import-savestate tool, using a fake game directory (never touches Steam)."""

import os
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "src"))

from polybot.capture import import_state as imp  # noqa: E402

SAMPLE = (ROOT / "testdata" / "savestates" / "early_game_turn2.state").read_bytes()


def make_source(tmp: Path) -> Path:
    src = tmp / "Singleplayer"
    src.mkdir()
    (src / "older-game.state").write_bytes(b"\x00junk")
    newest = src / "newest-game.state"
    newest.write_bytes(SAMPLE)
    os.utime(src / "older-game.state", (1_000_000, 1_000_000))
    os.utime(newest, (2_000_000, 2_000_000))
    return src


def test_imports_newest_by_default_and_verifies_bytes():
    with tempfile.TemporaryDirectory() as t:
        tmp = Path(t)
        src, dest = make_source(tmp), tmp / "out"
        assert imp.main(["my_game", "--source-dir", str(src), "--dest", str(dest)]) == 0
        assert (dest / "my_game.state").read_bytes() == SAMPLE


def test_game_selection_by_id_and_index():
    with tempfile.TemporaryDirectory() as t:
        src = make_source(Path(t))
        assert imp.pick_save(src, None).stem == "newest-game"
        assert imp.pick_save(src, "older-game").stem == "older-game"
        assert imp.pick_save(src, "1").stem == "older-game"  # index 1 = second newest


def test_refuses_to_overwrite_without_force():
    with tempfile.TemporaryDirectory() as t:
        tmp = Path(t)
        src, dest = make_source(tmp), tmp / "out"
        args = ["x", "--source-dir", str(src), "--dest", str(dest)]
        assert imp.main(args) == 0
        assert imp.main(args) == 1
        assert imp.main(args + ["--force"]) == 0


def test_name_validation():
    assert imp.clean_name("full_game_1") == "full_game_1.state"
    assert imp.clean_name("full_game_1.state") == "full_game_1.state"
    for bad in ["", "../escape", "a/b", " spaced", ".hidden"]:
        try:
            imp.clean_name(bad)
        except ValueError:
            continue
        raise AssertionError(f"{bad!r} should be rejected")


def test_undecodable_file_is_still_kept_with_a_warning():
    with tempfile.TemporaryDirectory() as t:
        tmp = Path(t)
        src = tmp / "Singleplayer"
        src.mkdir()
        (src / "weird.state").write_bytes(b"\x00not a real save")
        dest = tmp / "out"
        assert imp.main(["weird", "--source-dir", str(src), "--dest", str(dest)]) == 0
        assert (dest / "weird.state").exists()
        assert imp.describe(b"\x00not a real save")[0].startswith("WARNING")


if __name__ == "__main__":
    for name, fn in list(globals().items()):
        if name.startswith("test_"):
            fn()
    print("ok")
