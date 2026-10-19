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
4. **Ghidra project (done, "lean" approach)**: full auto-analysis was abandoned: after 31 min it was still inside
   `CallFixupAnalyzer` with no progress indicator (and tens of thousands of harmless exception-table errors).
   Instead, import with analysis off and let a script create + name a function at every Cpp2IL address:
   ```
   GHIDRA_HEADLESS_MAXMEM=6G ~/src/tools/ghidra_12.1.4/support/analyzeHeadless \
     ~/src/polybot_re_out/ghidra_proj_lean polytopia -import "<game dir>/GameAssembly.so" -noanalysis \
     -scriptPath re/scripts/ghidra -postScript ApplyIl2cppNames.java ~/src/polybot_re_out/method_addresses.tsv
   ```
   Takes about 5 minutes. Ghidra image base is 0x100000, so address = 0x100000 + Cpp2IL VA (verified).
   Result: 112,961 methods named, 112,731 functions created, 0 failures. Names look like `Type__Method`
   (non-alphanumerics become `_`; nested types use `Outer_Inner`; duplicate names get a `_<va>` suffix).
5. **Decompile on demand**: `re/scripts/ghidra/DecompileFunctions.java` writes `<name>.c` for every function whose
   name matches one of the given regexes:
   ```
   GHIDRA_HEADLESS_MAXMEM=4G ~/src/tools/ghidra_12.1.4/support/analyzeHeadless \
     ~/src/polybot_re_out/ghidra_proj_lean polytopia -process GameAssembly.so -noanalysis -readOnly \
     -scriptPath re/scripts/ghidra -postScript DecompileFunctions.java ~/src/polybot_re_out/decomp "AttackCommand__.*"
   ```
   Output goes to `~/src/polybot_re_out/decomp/`. Reading tips: `param_N + 0xNN` is a field at that offset (compare with
   the offsets in `diffable-cs`); `func_0x02b05f20(...)` calls are il2cpp runtime boilerplate (metadata init, casts,
   null/type checks) and can be skipped; calls to named methods are the real logic.

## Decompilation targets, in order
`GameState.Serialize/Deserialize` (savestate format) -> `BattleHelpers`, `MoveCommand`,
`AttackCommand.ExecuteDefault`, `EndTurnCommand`, other commands' `IsValid`/`Execute` ->
`MapGenerator` (last; exact map parity is not required for training).
