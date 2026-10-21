# Directories outside the repo

Big or third-party files are kept out of git. Everything here can be regenerated.

## Game install (read-only input)
`~/.local/share/Steam/steamapps/common/The Battle of Polytopia/`
- `GameAssembly.so` (107 MB): the compiled game code (IL2CPP output)
- `Polytopia_Data/il2cpp_data/Metadata/global-metadata.dat`: names/types for that code
- `Polytopia_Data/data.unity3d`: Unity asset archive; holds the rule-table JSON
- Never modified by this project.

## Savestates
`~/.config/unity3d/Midjiwan/Polytopia/<profile id>/Data/Singleplayer/*.state`
- Written only when the game is saved to the main menu. Read by `src/polybot/capture`.
- Captures go to `./captures/` in the repo (git-ignored, `*.pkl`).
- Saves worth keeping as test fixtures are copied to `testdata/savestates/` (committed) with `uv run import-savestate NAME`.

## Tools: `~/src/tools/`
| Path | What | Source |
|---|---|---|
| `cpp2il/Cpp2IL` | IL2CPP dumper, release `2022.1.0-pre-release.21` (Linux binary) | GitHub SamboyCoding/Cpp2IL releases. SHA-256 `526998e5...c500349` |
| `cpp2il-src/` | Cpp2IL source at the same tag; used to build the address-map tool | `git clone --branch 2022.1.0-pre-release.21` |
| `ghidra_12.1.4/` | Ghidra (pre-existing) | n/a |
| `jdk-25.0.4.1+1/` | JDK (pre-existing) | n/a |

## Game versions and artifact naming
Steam auto-updates the game, which changes `GameAssembly.so` and invalidates every address-based artifact. Artifacts for the
original analysis (game **2.17.2**, savestate/game version **122**) have no suffix; ones for **2.17.3** (version **123**) end in
`-2.17.3`. Check `version.txt` in the game folder and the SHA-256 of `GameAssembly.so` before trusting an old artifact.
Do not mix them: addresses in `method_addresses.tsv` and the `ghidra_proj_lean` project only match the 2.17.2 binary.

## Dumps: `~/src/polybot_re_out/`
| Path | What | Regenerate with |
|---|---|---|
| `diffable-cs/DiffableCs/` | C#-style declarations for every type (empty bodies). Rules live in `GameLogicAssembly/` (242 types) | `Cpp2IL --output-as diffable-cs` (see re-pipeline.md) |
| `isil/IsilDump/` | Per-method x86 disassembly and ISIL; no addresses. 356 MB, probably not needed | `Cpp2IL --output-as isil` |
| `method_addresses.tsv` | 112,964 rows: va, rva, file offset, type, method, signature | `re/scripts/addrmap` |
| `unity_assets/` | 38 text assets from `data.unity3d`, including `GameLogicData1..28_*.txt` (rules JSON) | `re/scripts/extract_unity_assets.py` |
| `ghidra_proj_lean/` | Persistent Ghidra project `polytopia`: no auto-analysis, 112k functions created and named. **Use this one.** | re-pipeline.md step 4 |
| `ghidra_lean.log` | Log of that job | n/a |
| `decomp/` | Decompiled functions (`Type__Method.c`), produced on demand | re-pipeline.md step 5 |
| `diffable-cs-2.17.3/`, `isil-2.17.3/`, `method_addresses-2.17.3.tsv`, `unity_assets-2.17.3/`, `ghidra_proj_lean-2.17.3/`, `ghidra_lean-2.17.3.log` | Same artifacts rebuilt for 2.17.3 (v123) | same commands as above, pointed at the new install |
| `isil_changes_*.txt` | Method-level differences 2.17.2 -> 2.17.3 | `re/scripts/compare_isil.py` |
| `venv/` | Python venv with UnityPy for `extract_unity_assets.py` | `uv venv venv && uv pip install --python venv/bin/python UnityPy` |
| `addrmap_build/` | Build directory for the address-map tool | copy of `re/scripts/addrmap`, `dotnet build -c Release` |
| `ghidra_proj/`, `ghidra_headless.log` | Abandoned first attempt (full analysis, killed); empty project, safe to delete | n/a |

## Scratchpad (temporary, session-scoped)
`/tmp/claude-1000/-home-eaglehawkinator-src-polybot/<session id>/scratchpad/`
- UnityPy virtualenv, build output of the address-map tool, `meta_strings.txt` (string dump of the
  metadata). Can disappear; anything worth keeping was copied into `re/`.
