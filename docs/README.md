# polybot docs

Notes on the project: a fast headless Polytopia re-implementation (Rust) plus RL training (Python).

| File | What's in it |
|---|---|
| [overview.md](overview.md) | Goal, architecture, planned repo layout, phases |
| [re-pipeline.md](re-pipeline.md) | How the game was reverse engineered, step by step, with commands |
| [external-artifacts.md](external-artifacts.md) | Every directory *outside* the repo that holds tools or dumps, and how to regenerate it |
| [game-findings.md](game-findings.md) | What we know about the game's internals (observed vs. inferred) |
| [rules/move-and-endturn.md](rules/move-and-endturn.md) | How commands/actions execute; Move and EndTurn rules read from the decompile |
| [replay-testing.md](replay-testing.md) | Rust loader, state diff, replay runner, test savestates and the `import-savestate` tool |
| [game-updates.md](game-updates.md) | What each Steam game update changed, and how to check a new build |
| [rules/move-and-train.md](rules/move-and-train.md) | Move, Train, exploration, RunDeferred: implemented and replay-verified |
| [rules/end-turn.md](rules/end-turn.md) | EndTurn and StartTurn (income, healing, turn order): implemented from the code, unit-tested, not yet replay-verified |
| [rules/build-and-city-reward.md](rules/build-and-city-reward.md) | Build, CityReward, population and level-up: implemented from the code, unit-tested, not yet replay-verified |
| [rules/pathfinder.md](rules/pathfinder.md) | A* pathfinder, movement costs, MinHeap tie-breaking: implemented and replay-verified (`pass_turn_1`) |
| [rules/explorer.md](rules/explorer.md) | Explorer (verified), Border Growth, Super Unit city rewards; XXHash, FastPath fog search |
| [rules/meet.md](rules/meet.md) | MeetAction, first contact, lighthouse corners: implemented |
| [rules/capture-and-attack.md](rules/capture-and-attack.md) | Capture, Attack, city names, city connections: implemented and replay-verified |
| [rules/start-match.md](rules/start-match.md) | StartMatch, LearnTech, EnableTask: implemented and replay-verified |
| [status.md](status.md) | Current state, open questions, next steps, lessons learned |

Related: [`../re/README.md`](../re/README.md) is the short version of the pipeline, next to the scripts.
