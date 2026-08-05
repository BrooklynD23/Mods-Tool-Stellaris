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

- **Engine:** Rust — content-addressed parse cache, parallel pipeline, ~20 s cold /
  ~2 s warm on a 300-mod playset ([budget](docs/architecture/12-performance.md)).
- **GUI:** Tauri + Svelte — a ~12 MB installer instead of Electron's 120 MB
  ([rationale](docs/adr/0001-tech-stack.md)).
- **CLI:** the same engine, static binary, no runtime dependencies.
- **Extension:** add a rule, a parser, or a game — each is one self-registering file;
  severity and game knowledge are TOML ([contract](docs/adr/0002-extensibility-contract.md)).

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
