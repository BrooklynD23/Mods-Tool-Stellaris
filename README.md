# Stellaris Mod Doctor

A local desktop tool that diagnoses **what each mod takes away from the others** in a
Stellaris playset — which files get overwritten, which game objects get redefined, which
localisation keys collide, which mod wins by load order, and which of those outcomes are
actually likely to matter.

Designed to expand to other Paradox titles and then other Steam games through a
data-driven game-profile seam.

> **Status: M0 — skeleton.** The workspace, the `smd-core` data contracts, and the
> `cargo xtask lint` gates exist; every other crate is a documented stub. The complete
> architecture lives in [`docs/`](docs/README.md) — start with
> [`docs/README.md`](docs/README.md), and with
> [`docs/REVIEW-2026-08.md`](docs/REVIEW-2026-08.md) for what changed after review.

## What it does

| Layer | Detects |
| --- | --- |
| Files | Two mods shipping the same path; `replace_path` directory shadowing |
| Objects | Two mods defining the same trait, civic, building, tech, event… |
| Localisation | Duplicate keys, split by whether the values differ |
| Load order | Who wins, and — via the reference index — who was relying on the loser |
| Metadata | Missing dependencies, dependencies loading too late, version drift |

Every finding is explained in plain language and backed by the exact bytes that produced
it. From M5, findings are also scored and ranked; the first release (M1) lists them
unscored, with how certain each one is.

## What it is not

Not a mod manager, not a validator, not an oracle. It never edits a mod, never launches
the game, and never claims a mod is "broken" — it reports what gets discarded and lets
you see for yourself.

## Design at a glance

- **Engine:** Rust — two-layer parse cache, parallel pipeline, targeting ≤20 s cold /
  ≤5 s warm on a 300-mod playset. Those are *targets against a provisional workload*, not
  measurements; nothing is built yet ([budget](docs/architecture/12-performance.md)).
- **CLI:** the same engine, static binary, no runtime dependencies. The compatibility
  floor and the first shippable release.
- **GUI:** provisional. Tauri + Svelte is the working assumption, re-decided at M7
  against a browser-first alternative ([rationale](docs/adr/0001-tech-stack.md)).
- **Extension:** add a rule, a parser, or a game. A rule is four small single-purpose
  edits — the rule, its cases, a weight, an explanation — and CI fails if any is missing
  ([contract](docs/adr/0002-extensibility-contract.md)).

## Repository layout

```
crates/     the workspace — see docs/architecture/02-modules-and-crates.md
xtask/      lint gates (layers.toml, allowances.toml, game-names.toml)
docs/       this design
ui/         Svelte front-end                    (M7)
profiles/   per-game knowledge as TOML          (M0.5)
policy/     severity weights and explanations   (M5)
```

## Building

A stable Rust toolchain is the only prerequisite; `rust-toolchain.toml` pins the channel
and `rustup` installs it on first build.

```sh
cargo test  --workspace                                  # unit tests and doctests
cargo clippy --workspace --all-targets -- -D warnings     # includes the 60-line and
                                                          # 5-parameter function gates
cargo fmt --all --check
cargo xtask lint                                          # the structural gates
```

`cargo xtask lint` fails the build on a file over 300 **code** lines, a dependency edge
that contradicts [`xtask/layers.toml`](xtask/layers.toml), a game name outside
`smd-profiles`, a broken relative link under `docs/`, and any drift in
`[profile.release]` — where `panic = "unwind"` is load-bearing for `catch_unwind` rule
isolation and must not be traded for binary size.

Exemptions are per-file and need both halves: an inline
`// xtask:allow(<rule>) reason: …` and an entry in
[`xtask/allowances.toml`](xtask/allowances.toml). There is deliberately no
workspace-wide counter to increment.

## Prior art

[Irony Mod Manager](https://github.com/bcssov/IronyModManager) (conflict solving and
patch generation), [CWTools](https://github.com/cwtools/cwtools) (Clausewitz parsing and
validation), and the `tiger` validators. This tool aims at the gap they leave: a
**cross-mod, whole-playset, ranked diagnosis** readable by someone who does not write
mods.

## Licence

Dual-licensed under either of

- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE))
- MIT license ([`LICENSE-MIT`](LICENSE-MIT))

at your option — the Rust ecosystem norm. Contributions are accepted under the same terms.

The inbound source-reading policy that goes with this choice (REVIEW-2026-08 § G43) is
still owed: prior-art source under a copyleft licence must not be read while writing
equivalent Rust here.
