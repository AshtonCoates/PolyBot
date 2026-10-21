"""Compare two Cpp2IL ISIL dumps method by method to find what changed between game builds.

    python3 re/scripts/compare_isil.py OLD/IsilDump NEW/IsilDump [Assembly ...] [--out report.txt]

Absolute addresses (global slots, unresolved call targets, `...h` operands of instructions Cpp2IL could not translate)
shift between builds, so any hex number of 5+ digits is normalized away before comparing; call targets that Cpp2IL resolved to method names are kept. Only the machine-independent
"ISIL:" part of each method is compared. Output: methods added, removed and changed.
"""

from __future__ import annotations

import argparse
import difflib
import pathlib
import re
import sys

HEX = re.compile(r"0x[0-9A-Fa-f]{5,}|\b[0-9A-F]{5,}h\b")


def methods(path: pathlib.Path) -> dict[str, list[str]]:
    """Method signature -> normalized ISIL lines (duplicate signatures get a #n suffix)."""
    out: dict[str, list[str]] = {}
    sig, in_isil, lines = None, False, []

    def flush():
        nonlocal sig, lines
        if sig is not None:
            key, n = sig, 1
            while key in out:
                n += 1
                key = f"{sig}#{n}"
            out[key] = lines
        sig, lines = None, []

    for raw in path.read_text(errors="replace").splitlines():
        if raw.startswith("Method: "):
            flush()
            sig, in_isil = raw[len("Method: "):].strip(), False
        elif raw.startswith("ISIL:"):
            in_isil = True
        elif raw.startswith("Disassembly:"):
            in_isil = False
        elif in_isil and raw.strip():
            lines.append(HEX.sub("0xH", raw.strip()))
    flush()
    return out


def compare_assembly(old: pathlib.Path, new: pathlib.Path):
    added, removed, changed = [], [], []
    old_files = {p.name: p for p in old.glob("*.txt")}
    new_files = {p.name: p for p in new.glob("*.txt")}
    for name in sorted(set(old_files) | set(new_files)):
        type_name = name[:-4]
        if name not in new_files:
            removed.append(f"{type_name} (whole type)")
            continue
        if name not in old_files:
            added.append(f"{type_name} (whole type)")
            continue
        a, b = methods(old_files[name]), methods(new_files[name])
        for sig in sorted(set(a) | set(b)):
            if sig not in b:
                removed.append(f"{type_name}::{sig}")
            elif sig not in a:
                added.append(f"{type_name}::{sig}")
            elif a[sig] != b[sig]:
                ratio = difflib.SequenceMatcher(None, a[sig], b[sig], autojunk=False).ratio()
                changed.append((f"{type_name}::{sig}", len(a[sig]), len(b[sig]), ratio))
    return added, removed, changed


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("old", type=pathlib.Path)
    ap.add_argument("new", type=pathlib.Path)
    ap.add_argument("assemblies", nargs="*", default=["GameLogicAssembly", "PolytopiaAssembly"])
    ap.add_argument("--out", type=pathlib.Path, help="write the full report here")
    args = ap.parse_args()
    report = []
    for asm in args.assemblies:
        added, removed, changed = compare_assembly(args.old / asm, args.new / asm)
        report.append(f"## {asm}: {len(changed)} changed, {len(added)} added, {len(removed)} removed")
        report += [f"changed  {n}  (isil lines {a} -> {b}, similarity {r:.2f})" for n, a, b, r in sorted(changed, key=lambda c: c[3])]
        report += [f"added    {n}" for n in added] + [f"removed  {n}" for n in removed]
        report.append("")
    text = "\n".join(report)
    if args.out:
        args.out.write_text(text)
    print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
