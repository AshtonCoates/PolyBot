# Reverse-engineering notes

Target: The Battle of Polytopia 2.17.2 (Steam, Linux), Unity 6000.3.17f1, IL2CPP,
global-metadata version 39. Large generated artifacts live outside the repo in
`~/src/polybot_re_out/` and are reproducible with the steps below.

## Pipeline
1. **Type/method dump** - Cpp2IL `2022.1.0-pre-release.21` (Linux build, in `~/src/tools/cpp2il/`):
   `Cpp2IL --game-path <game dir> --exe-name Polytopia.x86_64 --output-as diffable-cs --output-to <dir>`
   Gives C# declarations (empty bodies). Game rules live in the `GameLogicAssembly` namespace dir.
2. **Method addresses** - `scripts/addrmap/` (builds against Cpp2IL's LibCpp2IL source, checked out
   at `~/src/tools/cpp2il-src`). Writes `method_addresses.tsv` (va, rva, file offset, type, method).
3. **Rule tables** - `scripts/extract_unity_assets.py <data.unity3d> <out dir>` (needs UnityPy).
   Extracts the `GameLogicData*` TextAssets (JSON). `data/GameLogicData28.json` is the highest-numbered
   file (the numeric suffix in the original file name is a Unity path id, not a version).
4. **Decompilation** - headless Ghidra, no auto-analysis; see `docs/re-pipeline.md`.

## Open questions
- Real version number of each GameLogicData file, and which one a new game uses.
- Savestate encoding (see `GameState.Deserialize`).
