# Handoff — 2026-10-29 (M1 remainder appended 2026-11-19)

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

## M1 code remainder (m1/remaining, 2026-11-19)

Owner deferred testing (the device with the game is constrained). Every item below is
built and type-checked for Linux and Windows, and carries no new test beyond the minimum
noted.

| Item | What changed | Verified by |
| --- | --- | --- |
| H22 (Windows) | `sys_other.rs`: every opened dir and file is canonicalised and must start with the canonical root, else `foreign` → `fs.escaped_root`. Path-based, not handle-based (`GetFinalPathNameByHandleW` needs `unsafe`; `ponytail:` comment). Walk tests now compile on Windows; only symlink, FIFO and Unix-permission cases stay `cfg(unix)` | Windows clippy only; the ported tests have never run on Windows |
| H50 | `IndexBuilder::mark_content_incomplete`; the engine marks every key in `ScanCompleteness.content_incomplete` (refused dir link, bound, unopened root, unread archive). A `replace_path`-only path is then `Unknown` + `ParseIncomplete`, never `Deleted`, while such a source is not `Inactive` | one Stage A test (`h50_content_incomplete_source_blocks_deleted`) |
| H6 | Every profile table rejects unknown keys (`ProfileError::UnknownKey`); later-stage keys are listed in `smd-profiles/src/strict.rs`. Tied overlapping `[[dir]]` globs are `ProfileError::OverlappingGlobs` (`glob::overlaps`). Shipped profile unchanged | existing tests; the old "unknown keys ignored" test now asserts rejection |
| H12 | Per-axis qualifier sentence: `note:` lines (table, md), `qualifiers` array (JSON). Constants in `smd-report/src/qualifier.rs` until the M5 catalogue | goldens re-blessed |
| Doc drift | doc 05 diagnostic-code table, and why `PlaysetEntry.enabled` stays `bool`; doc 11 Markdown and redaction rules as built; doc 03 rule 2. Exit code 3 was already in doc 04 | `xtask lint` (doc-link) |
| H36 (part) | `shared-enum-wildcard` xtask gate, live. One hit fixed (`VersionSpec::check`) | it fired on that hit |
| H24 (part) | `fuzz/` (own workspace): `steam_vdf`, `descriptor`, `dlc_load_json`; `.github/workflows/fuzz.yml` nightly, 60 s each; seam `smd_ingest::parse_dlc_load_json` | `cargo check` on stable; never run |

Not done here:
- struct-literal ban: existing literals of `SourceMeta`, `PlaysetEntry`, `Site` and
  others outside `smd-core` need named constructors first;
- rule ↔ `explain/*` and placeholder lints: no `explain/*.toml` exists until smd-policy
  (M5);
- `[[dir]]` golden coverage, `--selftest`/`registry.lock`, the Mermaid diagram,
  `dir-audit`;
- precedence goldens for H6;
- mapped network drives: still need `GetDriveTypeW`, which needs `unsafe`.

## Cleanup and game-version drift (chore/cleanup-version)

- Deleted the five 12-line stub crates (`smd-gui`, `smd-py`, `smd-policy`, `smd-parse`,
  `smd-testkit`). Nothing used them; docs mark them planned (M2/M5/M7). Re-add each with
  its `xtask/layers.toml` entry when its milestone starts.
- `[version]` is now typed (`Profile::version()`, unknown keys rejected) with a new
  `verified_range` key. `smd-ingest` reads `launcher-settings.json` (`rawVersion`, then
  `version`; user dir, then install dir) and warns `ingest.game_version_unverified` when
  out of range or unreadable. It shows in `smd scan` diagnostics and `smd locate`
  (`game_version` JSON field, schema 1 additive). `verified_range = ["4.*"]` is a guess,
  not an observation.

## Perf change on `perf/lazy-hash`

The walk no longer hashes every file. `ReadOnlyRoot::walk` is stat-only; the engine then
hashes only paths shipped by two or more sources (`smd-engine/src/hash_pass.rs`, doc 12
"Lazy content hashing"). An oversize or unreadable file at a path only one source ships
no longer makes that source incomplete, and raises no diagnostic. Item 5 below (24
oversize files) should shrink accordingly; re-run the 71-mod scan natively to confirm.

## M5 policy, M8 second game, RE workflow (`m5/policy-m8-re`)

- **`smd-policy`** is back (layer 4, deps `smd-core` + `smd-profiles` for its glob). It
  reads `policy/weights.toml` and `policy/explain/en.toml` (embedded; `Policy::from_dir`
  reads the same layout). Missing `[caps]` rows or `[certainty.*]` qualifiers are load
  errors. Every weight is UNCALIBRATED.
- **Scoring is opt-in:** `smd scan --score`. Without it, output is byte-identical to M1
  (the report goldens still pass). With it, each finding prints band, score, a 16-hex
  finding id (blake3 of rule + subject), the explanation and the certainty qualifiers.
- **Suppressions:** `smd suppress <id> [--note ...]` writes a versioned
  `suppressions.toml` under `%APPDATA%\StellarisModDoctor\` or
  `$XDG_CONFIG_HOME/stellaris-mod-doctor/`. A suppressed finding is still printed,
  marked. The directory name is the working product title (doc 14 § Product name is
  open); it lives in `crates/smd-policy/src/appdir.rs`, the one file with a game-name
  allowance.
- **`cargo xtask calibrate` / `ground-truth`** are stubs that fail with "needs labelled
  ground truth". There is no labelled set yet.
- **M8:** `profiles/ck3.toml` exists. Every value is UNVERIFIED, there is no VERIFY.md
  section for it, and it has never been run against a real install. `--game <id>`
  (default: the first shipped profile) selects it for scan and locate. A new
  `[locations] content_subdir` key handles vanilla content under `<install>/game/`.
- **RE:** `.github/workflows/release.yml` builds `smd.exe` and a Linux binary on a `v*`
  tag, writes `SHA256SUMS`, and drafts a release. It has never been triggered. Signing is
  a placeholder guarded on a `SIGNING_CERT` secret, blocked on the certificate (H33). The
  H33 release gate (selftest, goldens, ground-truth diff) is not in it yet.

## Next steps, in order

Windows-first. Each item names its gate.

1. **H22, Windows half of the secure walk.** This is the top priority now that Windows
   is the target. `crates/smd-fs/src/sys_other.rs` is type-checked only:
   - ~~no final-path check on the opened handle~~ DONE on `m1/remaining`: a path-based
     canonicalise-under-root check. The handle-based check still needs `unsafe`;
   - reparse handling is now attribute-based (`smd-fs/src/winattrs.rs`, unit-tested on
     Linux): symlinks and junctions are refused; dedup and hydrated placeholder files
     are regular; offline placeholders are listed, never read, `fs.cloud_placeholder`.
     The exact reparse tag still needs `unsafe`, so unknown tags are not distinguished.
     Unverified on real Windows;
   - mapped network drives are treated as local, because detecting them needs
     `GetDriveTypeW`, which needs `unsafe`, and the crate forbids it. Decide on a
     reviewed exception.

   ~~The `smd-fs` walk tests are `cfg(unix)`~~ DONE on `m1/remaining`: the portable ones
   now compile on Windows. Run them on a Windows runner to close.
2. ~~**H50, refused directory link vs `Deleted`.**~~ DONE on `m1/remaining`, with
   whole-source scope (doc 05: "for any key and any directory"). The original note: `crates/smd-index/src/stage_a.rs`
   never reads `content_incomplete`. So a refused directory link does not stop a
   `Deleted` verdict in its source's scope. That is a definitive negative on
   incomplete evidence, the defect class this whole design exists to prevent. It
   becomes live once M2 adds `core.symbol_deleted`.
3. **H6, strict profile loader.** The loader half is DONE on `m1/remaining`; the
   precedence goldens are still open. The original note: `crates/smd-profiles/src/profile.rs` ignores unknown
   keys and doesn't reject overlapping globs of equal specificity. Doc 10 says both are
   load errors. Add the precedence goldens.
4. **Steam library enumeration inside `smd`.** DONE on `feat/locate` (needs Windows
   evidence): `smd locate` and `smd scan` without path flags, N workshop roots, GOG and
   Xbox probes (doc 05 § Discovery).
   - Still open: the `libraryfolders.vdf` fuzz target; running it on a real Windows box
     (registry, OneDrive Documents, GOG, Game Pass); the Game Pass user-directory
     candidate in `profiles/stellaris.toml` is a placeholder guess.
   - `tools/windows/ModDoctor.ps1` can now call `smd locate --format json --no-redact`.
   - DONE on `feat/discovery-sources` (needs a real install to verify): discovery also
     finds unregistered `mod/<dir>`, loose `mod/*.zip` and launcher-DB-only mods, each with
     an `origin`; the launcher playset join also uses `gameRegistryId`, `pdxId` and
     `archivePath`, and an unmatched entry makes other mods `Unknown`, not `Inactive`.
     Unverified: real `launcher-v2.sqlite` column contents (what `pdxId`, `dirPath` hold
     for Paradox Mods downloads).
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
   - ~~`PlaysetEntry.enabled` is still a bool~~ kept by design, now explained in doc 05;
   - ~~new ingest codes doc 05 doesn't list~~ DONE: doc 05 § Diagnostic codes;
   - discovery lists directories with `std::fs::read_dir` rather than through `smd-fs`;
   - ~~discovery-incomplete is recorded under a fake `"(discovery)"` `ModKey`~~ DONE on
     `feat/discovery-sources`: `ScanCompleteness::discovery_incomplete`, lowers certainty;
   - ~~`ModKey` was the raw `remote_file_id` (Steam/Paradox collision), hashed `version`, and
     the launcher DB was read twice~~ DONE on `fix/mod-identity`: keys are `steam:`/`pdx:`/
     `h-`, `SourceRole::ParadoxMods`, one read per scan (doc 05, "Mod sources and origin").
     Unverified: real launcher `pdxId`/`source` contents (VERIFY V03);
   - per-rule certainty ceilings live in an engine table (`smd-engine/src/certainty.rs`);
   - ~~CLI exit code 3 is not documented in doc 04~~ it already was;
   - ~~the Markdown code-span and account-redaction rules aren't written in doc 11~~ DONE;
   - ~~doc 03 rule 2 (serde)~~ DONE. Still open: doc 03 sort key 7 and the gap tags,
     discovery's `std::fs::read_dir`, and the engine certainty table.

   Fix the code or the docs, item by item.
7. ~~**H12 qualifier sentence.**~~ DONE on `m1/remaining`. Move the sentences into the
   M5 catalogue's `[certainty.*]`. The original note: Reports print per-axis certainty tags but not the
   per-axis qualifier sentence (doc 09, "The qualifier").
8. **Lint gates H35–H39.** The ledger lives in `13-testing.md`, and most gates are
   unbuilt:
   - ~~the shared-enum wildcard ban~~ live as `shared-enum-wildcard` (text-level);
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
| `smd-fs` walk tests on Windows | The portable ones now compile on Windows but have never run there | Run `cargo test -p smd-fs` on `windows-latest` |
| H50, H6, H12 (`m1/remaining`) | Owner deferred testing | Only one H50 Stage A test, the existing H6 tests and the re-blessed H12 goldens exist. Add a scan-level H50 case once `core.symbol_deleted` exists |
| Fuzzing (H24) | Targets and the nightly job exist (`fuzz/`, `fuzz.yml`) but have never run | Trigger `fuzz.yml` with `workflow_dispatch`; add an archive target with H23 |
| Performance budget | Never measured. The 1.5 s resolve budget leaves 0.1 s warm headroom on paper | M0.5 reference measurement (H32) |
| Probe mods in game | No Stellaris ≥ 4.5.2 install | See blockers |
| M4 meta/dep rules scaffolded, untested (`dep.missing`, `dep.order`, `meta.version_drift`, `meta.descriptor_mismatch`, branch `m4/deps-meta`) | Testing deferred by the owner. No `RuleCase`s; the case harness is path-only (`Expectation::Fires { path }`) and `PlaysetBuilder` cannot set deps, descriptors or a game version | Extend the harness to `Subject::Mod`/`ModPair` and the fixture with deps/meta, then one case per `CaseKind` per rule and the `const _` coverage assertion; plus an ingest test for name resolution and the inner `descriptor.mod` read |
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

## Review round 2 (fix/review-round2)

- Windows open: entries classified first (no-follow); links and offline refused; everything else (dedup/WOF/hydrated) opened WITHOUT FILE_FLAG_OPEN_REPARSE_POINT, re-verified on the handle. Offline directories are fs.cloud_placeholder, never listed, source content-incomplete. UNVERIFIED on real Windows.
- Launcher join: ids carry `evidenced`; an unevidenced steam:X also matches a row pdxId. A DB-only row whose dir resolves to an already-found root is skipped.
- Profile [version] source_dir is honoured (read first, other is fallback).
- Unreadable/out-of-range game version sets ScanCompleteness::game_version_unverified: semantics capped at Unmodelled, header prints it, scan_complete false.
- Hash pass sorts by path and reuses the parent directory chain.

## M2 (scaffolded, untested)

Branch `m2/parser`. Builds green on Linux and the Windows type-check; only smoke checks
exist. Testing was deferred by the owner, so nothing below is evidence for the M2 exit.

What exists:

| Piece | Where |
| --- | --- |
| `smd-parse` (layer 3): iterative push parser, depth cap 256, EOF/brace recovery, BOM + Windows-1252, 32-per-file diagnostic cap | `crates/smd-parse/src/{lexer,script,text}.rs` |
| `FileParser` registry by `FileKind`; `parse_file`; `ParseOutput` (= `CachedParse`) | `crates/smd-parse/src/registry.rs` |
| Extractors from the profile `DirClass`: `depth_key` (+ `AdditiveSplit` when `additive_children` is set), `pdx_event` | `crates/smd-parse/src/extract/` |
| Parse-cache key (all six terms) and an in-memory `ParseCache` | `crates/smd-parse/src/cache.rs` |
| Profile reads `domain`, `merge`, `level`, `parser`, `extractor`, `order`, `additive_children`, and the Stage D `[load_order]` readings | `crates/smd-profiles/src/dirclass.rs` |
| `DefinitionTable`, `resolve_symbol` (Stages C/D, every unknown reading enumerated), `no_live_site` | `crates/smd-index/src/defs/` |
| `core.symbol_collision`, `core.symbol_deleted`, five cases each | `crates/smd-rules/src/rules/` |
| Engine parses script files after the walk | `crates/smd-engine/src/parse_pass.rs` |

Deliberate limits (each marked `ponytail:` in code where it cuts a corner):

1. The merge scope is the domain; `scope` and `tier` are not read yet.
2. `@`-variables are not definitions, so `common/scripted_variables` yields no
   definitions until a variables extractor exists.
3. `.gui`, `.gfx` and `common/inline_scripts` are `parser = "none"` in the shipped
   profile: no extractor models their definitions yet.
4. `pdx_event` emits `Namespace`/`EventId` facts but the index has no `EventTable`; the
   facts are dropped at the index. `DefineKey` is not emitted (defines are `Definition`s
   named `Group.KEY`).
5. The cache lives for one scan, in memory, behind one mutex. No SQLite store.
6. A `Deleted` is blocked by any non-`Inactive` content-incomplete source, which closes
   the `Deleted` half of H50 from the definition layer. Stage A's own use of
   `content_incomplete` (`stage_a.rs`) is still the H50 owner's.

TODO, in order:

1. **H30 golden extraction corpus** — synthetic files for every `[[dir]]` shape with
   checked-in expected facts. Out of scope for this pass.
2. Fuzz target for `smd_parse::script::parse` (10k-deep nesting, arbitrary bytes).
3. `locyml` module + one line in `smd_parse::PARSERS`; `LocTable` in the index.
4. Rule-case and resolver tests beyond the mandatory five cases: FIOS vs LIOS, enumerated
   `filename_key` disagreement, additive-body residuals, cache H10 fixture (same bytes
   under two domains → two entries).
5. Determinism sweep and cancel-under-load stress (M2 exit 3).

## M4 loc/additive (scaffolded, untested)

Branch `m4/loc-additive`, off the M2 merge. Builds green on Linux and the Windows
type-check. Only the five mandatory `core.loc_collision` RuleCases and one `locyml` smoke
check exist. Testing was deferred by the owner, so nothing below is evidence for the M4
exit.

What exists:

| Piece | Where |
| --- | --- |
| `locyml` parser (`pdx_locyml`): `l_<lang>:` headers (repeatable), `key:N "value"`, optional ordinal, BOM, comments, escaped quotes, Windows-1252 fallback, 1024-byte key cap, `Partial` on any unreadable line | `crates/smd-parse/src/locyml.rs` |
| Profile `[syntax] languages` (position = `LangId`), `[[dir]] tier` read into `DirClass` | `crates/smd-profiles/src/dirclass.rs`, `profiles/stellaris.toml` |
| Engine parses every kind with a registered parser (script and localisation) | `crates/smd-engine/src/parse_pass.rs` |
| `LocTable` keyed `(LangId, key)`; `resolve_loc`: tier first (H3), then the definition Stage D under every unknown reading; no winner over an incomplete file at or above the winner's tier; no live entry is `Unknown`, never `Deleted` | `crates/smd-index/src/loc/` |
| H4: equal residuals no longer state `Coexist` over an incomplete scope (`ParseIncomplete`); `PartiallyMerged` emits `residual_lost = true` from `core.symbol_collision` | `crates/smd-index/src/defs/resolve.rs`, `crates/smd-rules/src/rules/symbol_collision.rs` |
| `core.loc_collision` (five cases), `agreement = all_identical` when every live value matches | `crates/smd-rules/src/rules/loc_collision.rs` |
| `[base]` row and explanations for `core.loc_collision` | `policy/weights.toml`, `policy/explain/en.toml` |

Deliberate limits:

1. Shipped `localisation/**` is still `merge = "unknown"` (V20, V39 not observed), so in a
   real scan every loc collision is reported with `merge_mode_unknown` and no winner. The
   tier only takes effect once `merge = "merge_by_key"` is set for the `loc` scope.
2. `common/on_actions` stays `merge = "unknown"` (V18). The additive-body path is only
   reachable from a profile that names `additive_children`.
3. Only `merge_by_key` is resolved for localisation; a group spanning two domains
   (`loc` + `loc_synced`) is `Unknown` + `merge_mode_unknown`.
4. The definition resolver still ignores `tier` (only loc reads it).
5. Doc 08 has no separate v1 additive rule (`pdx.on_action_overlap` is post-v1), so
   `core.symbol_collision` carries H4.
6. `core.symbol_collision` and `core.symbol_deleted` still have no `[base]` row.

TODO, in order:

1. Probe P2/P7 (V20, V39) and set the loc `merge`; then check the doc 07 worked case
   against the game.
2. Probe V18, then ship `merge_by_key_additive_body` + `additive_children` for
   `common/on_actions` and an `on_actions`-shaped RuleCase.
3. `locyml` golden corpus (multi-header, unterminated values, CRLF, non-BOM UTF-8) and a
   fuzz target.
4. Resolver tests: tier vs incomplete files at each tier, mixed-domain groups,
   `Coexist` blocked by `ParseIncomplete`.
