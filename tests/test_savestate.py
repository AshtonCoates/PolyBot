"""Parser checks against every fixture in testdata/savestates (skipped if there are none)."""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "src"))

from polybot.parsing.savestate import parse_savestate  # noqa: E402

FIXTURES = sorted((ROOT / "testdata" / "savestates").glob("*.state"))


def test_early_game_parses_exactly():
    g = parse_savestate((ROOT / "testdata" / "savestates" / "early_game_turn2.state").read_bytes())
    assert g.version == 122
    assert g.initial.head.current_turn == 0 and g.initial.commands == []
    assert g.current.head.seed == g.initial.head.seed
    assert len(g.current.commands) == g.last_seen_command == 41
    assert len(g.current.players) == len(g.initial.players) == 6
    assert g.current.head.map.width * g.current.head.map.height == len(g.current.head.map.tiles)


def test_every_fixture_parses():
    for path in FIXTURES:
        g = parse_savestate(path.read_bytes())
        assert len(g.current.commands) == g.last_seen_command, path.name


if __name__ == "__main__":
    test_early_game_parses_exactly()
    test_every_fixture_parses()
    print("ok")
