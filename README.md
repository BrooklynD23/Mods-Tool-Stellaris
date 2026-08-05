# Stellaris Mod Doctor

A local desktop tool that diagnoses **what each mod takes away from the others** in a
Stellaris playset — which files get overwritten, which game objects get redefined, which
localisation keys collide, which mod wins by load order, and which of those outcomes are
actually likely to matter.

Designed to expand to other Paradox titles and then other Steam games through a
data-driven game-profile seam.

> **Status: design phase.** No implementation yet. The complete architecture lives in
> [`docs/`](docs/README.md) — start with [`docs/README.md`](docs/README.md).

## What it does

| Layer | Detects |
| --- | --- |
| Files | Two mods shipping the same path; `replace_path` directory shadowing |
| Objects | Two mods defining the same trait, civic, building, tech, event… |
| Localisation | Duplicate keys, split by whether the values differ |
| Load order | Who wins, and — via the reference index — who was relying on the loser |
| Metadata | Missing dependencies, dependencies loading too late, version drift |

Every finding is scored, ranked, explained in plain language, and backed by the exact
bytes that produced it.

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

## Repository layout (planned)

```
crates/     the workspace — see docs/architecture/02-modules-and-crates.md
ui/         Svelte front-end
profiles/   per-game knowledge as TOML
policy/     severity weights and explanation catalogue
docs/       this design
xtask/      lint gates, fixture generation, release tooling
```

## Prior art

[Irony Mod Manager](https://github.com/bcssov/IronyModManager) (conflict solving and
patch generation), [CWTools](https://github.com/cwtools/cwtools) (Clausewitz parsing and
validation), and the `tiger` validators. This tool aims at the gap they leave: a
**cross-mod, whole-playset, ranked diagnosis** readable by someone who does not write
mods.

## Licence

TBD before first release.
