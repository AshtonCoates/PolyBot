# Reverse-engineering pipeline

Target: Polytopia 2.17.2 (Steam, Linux), Unity 6000.3.17f1, IL2CPP, metadata version 39.

## Background: what IL2CPP gives us
C# is translated to C++ and compiled to native code (`GameAssembly.so`), which loses names.
A separate `global-metadata.dat` keeps type/method/field names and signatures. Cpp2IL reads both:
metadata gives names, and registration tables inside the binary map each method to its native
address. Result: exact declarations and addresses, but not method bodies. Bodies still need a
decompiler (Ghidra).

## Steps (all done unless noted)
1. **Declarations**
   ```
   ~/src/tools/cpp2il/Cpp2IL --game-path "<game dir>" --exe-name Polytopia.x86_64 \
     --output-as diffable-cs --output-to ~/src/polybot_re_out/diffable-cs
   ```
2. **Method addresses**: `re/scripts/addrmap/` is a small console app referencing Cpp2IL's
   `LibCpp2IL` project. It prints `method_addresses.tsv`. VA = RVA + 0x1000 for this binary.
   Build needs a .NET SDK; LibCpp2IL's own `global.json` asks for SDK 9, so the app is built from
   a directory outside the Cpp2IL checkout.
3. **Rule tables**: `re/scripts/extract_unity_assets.py` (UnityPy, run in an isolated venv with
   `python -I`) pulls TextAssets out of `data.unity3d`. Latest rules copy: `re/data/GameLogicData28.json`.
4. **Decompilation (running)**: headless Ghidra imports `GameAssembly.so`, auto-analyses, then
   `re/scripts/ghidra/ApplyIl2cppNames.java` names functions from the TSV. Project saved to
   `~/src/polybot_re_out/ghidra_proj`.
   ```
   GHIDRA_HEADLESS_MAXMEM=6G ~/src/tools/ghidra_12.1.4/support/analyzeHeadless \
     ~/src/polybot_re_out/ghidra_proj polytopia -import "<game dir>/GameAssembly.so" \
     -scriptPath re/scripts/ghidra -postScript ApplyIl2cppNames.java ~/src/polybot_re_out/method_addresses.tsv
   ```
   Assumption not yet checked: Ghidra address = image base + Cpp2IL VA. The script prints the image
   base and a named/created/noCode/failed count to the log; if most fail, fix the offset and rerun
   only the naming step on the saved project.
5. **Querying**: once the project is saved, decompile target methods from it (REA or headless
   scripts). Not done yet.

## Decompilation targets, in order
`GameState.Serialize/Deserialize` (savestate format) -> `BattleHelpers`, `MoveCommand`,
`AttackCommand.ExecuteDefault`, `EndTurnCommand`, other commands' `IsValid`/`Execute` ->
`MapGenerator` (last; exact map parity is not required for training).
