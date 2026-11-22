# Stellaris Mod Doctor — agent notes

- **Progress board:** [`KANBAN.md`](KANBAN.md). When you finish a slice, update the board in
  the same commit: move its card, add the commit sha, bump "Last updated". Cards reach
  **Verified** only with named evidence.
- **Where work stopped and why:** [`docs/HANDOFF.md`](docs/HANDOFF.md). Read it before
  starting a slice; add a short section when you end one.
- **Active branch:** `work`, not `main`. Worktree agents start from `main`'s first commit,
  so begin with `git reset --hard <work tip>`.
- **Read-only invariant:** the tool and every check you run only read the game, launcher,
  mods and saves.
