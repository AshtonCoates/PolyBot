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

## Tools: `~/src/tools/`
| Path | What | Source |
|---|---|---|
| `cpp2il/Cpp2IL` | IL2CPP dumper, release `2022.1.0-pre-release.21` (Linux binary) | GitHub SamboyCoding/Cpp2IL releases. SHA-256 `526998e5...c500349` |
| `cpp2il-src/` | Cpp2IL source at the same tag; used to build the address-map tool | `git clone --branch 2022.1.0-pre-release.21` |
| `ghidra_12.1.4/` | Ghidra (pre-existing) | n/a |
| `jdk-25.0.4.1+1/` | JDK (pre-existing) | n/a |

## Dumps: `~/src/polybot_re_out/`
| Path | What | Regenerate with |
|---|---|---|
| `diffable-cs/DiffableCs/` | C#-style declarations for every type (empty bodies). Rules live in `GameLogicAssembly/` (242 types) | `Cpp2IL --output-as diffable-cs` (see re-pipeline.md) |
| `isil/IsilDump/` | Per-method x86 disassembly and ISIL; no addresses. 356 MB, probably not needed | `Cpp2IL --output-as isil` |
| `method_addresses.tsv` | 112,964 rows: va, rva, file offset, type, method, signature | `re/scripts/addrmap` |
| `unity_assets/` | 38 text assets from `data.unity3d`, including `GameLogicData1..28_*.txt` (rules JSON) | `re/scripts/extract_unity_assets.py` |
| `ghidra_proj/` | Persistent Ghidra project `polytopia` (analysis + applied names) | headless job in re-pipeline.md |
| `ghidra_headless.log` | Log of that job | n/a |

## Scratchpad (temporary, session-scoped)
`/tmp/claude-1000/-home-eaglehawkinator-src-polybot/<session id>/scratchpad/`
- UnityPy virtualenv, build output of the address-map tool, `meta_strings.txt` (string dump of the
  metadata). Can disappear; anything worth keeping was copied into `re/`.
