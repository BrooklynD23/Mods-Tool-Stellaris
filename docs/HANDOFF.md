# Handoff — 2026-10-29

Where the work stopped, what to do next, what was never tested, and what is waiting on
the owner. Written for the next agent session. The dev environment was shut down
deliberately with nothing in flight.

## State at handoff

| Item | Value |
| --- | --- |
| Branch | `claude/m0.6-review-remediation` at `1b15a34`, pushed |
| PR | #3, base `main`, CI green (lint, `windows-latest`, `ubuntu-latest`) |
| Superseded PRs | #1 (M0 skeleton) and #2 (M0.5 prior art). Their commits are merged into #3's branch, so merging #3 makes them redundant. The owner decides whether to close them |
| Running work | None. No workflow, agent, or build is paused mid-step |
| Local tests | 275 pass on Linux. CI runs the same suite on Windows |
| Repo visibility | Public. The owner changed it from private so that Actions minutes are free |

Read first: `docs/REVIEW-2026-10.md` for every finding, gate H1–H50, and the owner
decisions; `docs/architecture/15-roadmap.md` for milestone exits; this file for what
changed after the register was written.

## Ground rules the next session inherits

- **Model routing** is set in `../CLAUDE.md` (the parent "Github Repo" directory):
  - Opus orchestrates and reviews;
  - Sonnet implements;
  - Haiku at `medium` effort runs test scripts only;
  - Haiku handles read-only exploration.

  Set `model` explicitly on every agent.
- **Windows is the target platform. macOS is deferred** until the owner approves it
  (REVIEW-2026-10, decision 2a). Don't add macOS CI, fixes, or packaging work.
- **No severity in M1** (owner decision). Ranking arrives at M5.
- **Never write to the game, launcher, mods, or saves.** Real-data checks are read-only.
- **Worktree caveat.** `isolation: worktree` agents start from `main`'s initial commit,
  not the current branch. Every implementer brief must begin with
  `git reset --hard <branch tip>`.

### Environment, if rebuilt

```bash
# Rust stable via rustup into ~/.cargo (removable with `rustup self uninstall`)
export PATH=$HOME/.cargo/bin:$PATH CARGO_TARGET_DIR=$HOME/.cache/smd-target
cargo fmt --all --check && cargo test --workspace && \
  cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo xtask lint
```

Type-checking for Windows from Linux, without a MinGW C compiler, needs no-op `cc` and
`ar` stubs. Both are throwaway scripts kept outside the repo:

```bash
mkdir -p ~/fakecc
printf '#!/bin/sh\nout=""; prev=""; for a in "$@"; do [ "$prev" = "-o" ] && out="$a"; prev="$a"; done\n[ -n "$out" ] && : > "$out"; exit 0\n' > ~/fakecc/cc
printf '#!/bin/sh\nfor a in "$@"; do case "$a" in *.a) : > "$a";; esac; done; exit 0\n' > ~/fakecc/ar
chmod +x ~/fakecc/cc ~/fakecc/ar
rustup target add x86_64-pc-windows-gnu
CC_x86_64_pc_windows_gnu=~/fakecc/cc AR_x86_64_pc_windows_gnu=~/fakecc/ar \
  cargo clippy --workspace --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings
```

Real Windows evidence comes from CI. `ci.yml` has `workflow_dispatch`, so push a
throwaway branch, run `gh workflow run CI --ref <branch>`, and delete the branch
afterwards.

The implementation workflow that built the M1 slice was `smd-implement-m1`. It plans,
then works in waves (Sonnet implements, Opus reviews, Sonnet fixes), then integrates.
It completed. The script was saved outside the repo at
`~/.claude/projects/-mnt-e-Programming-Github-Repo-Mods-Tool-Stellaris/1c7aa16b-b682-40d8-b8f8-b5ad4f4a9572/workflows/scripts/smd-implement-m1-wf_3ff10c10-e2c.js`.
Reuse its shape for the next wave, with the corrected model routing.

## Next steps, in order

Windows-first. Each item names its gate.

1. **H22, Windows half of the secure walk.** This is the top priority now that Windows
   is the target. `crates/smd-fs/src/sys_other.rs` is type-checked only:
   - no final-path check on the opened handle;
   - no reparse-tag allow-list (junctions, symlinks, OneDrive cloud placeholders: list
     them, never hydrate them);
   - mapped network drives are treated as local, because detecting them needs
     `GetDriveTypeW`, which needs `unsafe`, and the crate forbids it. Decide on a
     reviewed exception.

   The `smd-fs` walk tests are `cfg(unix)`, so they don't run on Windows at all. Port
   them.
2. **H50, refused directory link vs `Deleted`.** `crates/smd-index/src/stage_a.rs`
   never reads `content_incomplete`. So a refused directory link does not stop a
   `Deleted` verdict in its source's scope. That is a definitive negative on
   incomplete evidence, the defect class this whole design exists to prevent. It
   becomes live once M2 adds `core.symbol_deleted`.
3. **H6, strict profile loader.** `crates/smd-profiles/src/profile.rs` ignores unknown
   keys and doesn't reject overlapping globs of equal specificity. Doc 10 says both are
   load errors. Add the precedence goldens.
4. **Steam library enumeration inside `smd`.** Parse `libraryfolders.vdf` (H24 caps,
   plus a fuzz target) and find the OS default paths, so that `smd scan` works without
   `--game-dir`, `--user-dir` and `--workshop-dir`.
   - Today only `tools/windows/ModDoctor.ps1` detects paths.
   - `smd` accepts only one `--workshop-dir`.
5. **Real-playset noise follow-ups.** A scan of the owner's 71-mod playset reported
   2,432 same-path conflicts. It is still marked incomplete:
   - 24 oversize files inside mods;
   - 1 archive-only mod;
   - 1 directory that wasn't read completely.

   Triage whether the 32 MB cap is right for mod `gfx/` assets, or whether asset
   directories should be inventory-only the way vanilla DLC zips are now. Recheck the
   remaining same-path findings for other file types the game never loads.
6. **Reconcile code with docs.** The integration review listed these drifts:
   - doc 03 rule 2: some enums now derive serde for the parse cache;
   - doc 03 sort key 7 also folds gap tags into the digest;
   - `PlaysetEntry.enabled` is still a bool, not `Activation`;
   - new ingest codes (`ingest.location_missing`, `ingest.discovery_incomplete`,
     `scan.asset_archive_unread`) that doc 05 doesn't fully list;
   - discovery lists directories with `std::fs::read_dir` rather than through `smd-fs`;
   - discovery-incomplete is recorded under a fake `"(discovery)"` `ModKey`, so it
     never lowers any finding's certainty. Add a first-class field to
     `ScanCompleteness`;
   - per-rule certainty ceilings live in an engine table (`smd-engine/src/certainty.rs`);
   - CLI exit code 3 is not documented in doc 04;
   - the Markdown code-span and account-redaction rules aren't written in doc 11.

   Fix the code or the docs, item by item.
7. **H12 qualifier sentence.** Reports print per-axis certainty tags but not the
   per-axis qualifier sentence (doc 09, "The qualifier").
8. **Lint gates H35–H39.** The ledger lives in `13-testing.md`, and most gates are
   unbuilt:
   - the shared-enum wildcard ban;
   - the struct-literal ban;
   - rule ↔ `explain/*`;
   - placeholders;
   - `[[dir]]` golden coverage;
   - `--selftest` / `registry.lock`;
   - the Mermaid diagram generated from `layers.toml`;
   - `xtask dir-audit`.

   Rule self-registration (via `inventory`) arrives with H36.
9. **Then M2.** The parser, symbols and the concurrency shape, per `15-roadmap.md`,
   including the AdditiveSplit extractor (H4) and the cache key with `ExtractionClass`
   (H10).

## Testing not done

| What | Why not | How to close |
| --- | --- | --- |
| Native Windows build of `smd.exe` via `ModDoctor.ps1` (Setup, Build, Scan and Report actions) | Rust and the VS C++ Build Tools are not installed on the owner's Windows. Installing needs the owner's approval and admin rights | The owner runs menu option 10, then 4 and 5. Only Detect, Mods and Saves were run for real (read-only), plus a PowerShell 5.1 parse check |
| `smd scan` natively on Windows against a real playset | Same as above | Scan with the launcher closed and with it open. Compare with the WSL result below |
| Real-playset scan | Done only over WSL, using a path-translated copy of the launcher files. Took ~10 min because drvfs is slow | Native run. Measure time and memory against `12-performance.md` |
| H20 path A against a live launcher | Only synthetic WAL fixtures exist, though they now pass on a real Windows runner | Run with the launcher open and then just-reordered (H20/H29 live halves, H48) |
| `smd-fs` walk tests on Windows | Tests are `cfg(unix)` | Part of next step 1 |
| Fuzzing (H24) | No nightly fuzz job | Add `cargo-fuzz` targets and a nightly CI job |
| Performance budget | Never measured. The 1.5 s resolve budget leaves 0.1 s warm headroom on paper | M0.5 reference measurement (H32) |
| Probe mods in game | No Stellaris ≥ 4.5.2 install | See blockers |
| Windows mapped network drives, OneDrive placeholders, junctions | No test fixture yet | Part of next step 1 |

## Blockers on the owner

1. **Update Stellaris to ≥ 4.5.2.** The installed game is 4.4.6. Every M0.5 probe
   (H40–H48, V01–V42) is specified on 4.5.2+, and M0.5 blocks M1's exit.
2. **Run the probe mods** in `tools/probe-mods/` in game. Use one isolated playset per
   probe, following the README. P4 needs vanilla blocks pasted from your install first.
   Record each result in `VERIFY.md` with game version, checksum, launcher version, OS
   and date.
3. **Launcher evidence (H43, H48).** Two things only a live install can provide:
   - **H43:** dump the `launcher-v2.sqlite` schema, including `knex_migrations`,
     stamped with the launcher version. The tables are already observed to match the
     assumed model: `mods`, `playsets` (`isActive`), `playsets_mods`.
   - **H48:** observe whether `-wal`/`-shm` exist with the launcher open and closed.
4. **Approve installing Rust and the VS Build Tools on Windows**, or run
   `tools/windows/ModDoctor.cmd -Action Setup` yourself. This unblocks every native
   Windows test above.
5. **Three real playsets for the M1 exit**, one scanned with the launcher open, each
   with at least 20 same-path conflicts checked by hand under `tests/release/`.
6. **Release prerequisites (H33).** A Windows code-signing certificate and a
   release-pipeline decision.
7. **Decisions still open** in the register: whether D1 ships the GUI or a report;
   macOS approval (deferred); whether to close PRs #1 and #2 once #3 merges.
