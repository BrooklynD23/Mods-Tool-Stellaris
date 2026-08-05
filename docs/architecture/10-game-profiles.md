# 10 — Game profiles: the multi-game seam

The roadmap is Stellaris → other Paradox titles → other Steam games. That expansion
either happens through one clean seam or it turns the codebase into a pile of
`if game == "stellaris"`. This document defines the seam.

## Principle

**Nothing above `smd-profiles` may name a game.** Grep for `stellaris` in
`smd-index`, `smd-rules`, `smd-policy`, or `smd-report` should return zero hits outside
of tests. `cargo xtask lint` enforces exactly that.

## Three tiers of support

| Tier | What is needed | Effort |
| --- | --- | --- |
| **1 — Declarative** | A TOML file only | An afternoon |
| **2 — Adapter** | TOML + one syntax adapter or extractor | A few days |
| **3 — Native** | TOML + parser crate + custom load-order source | A sprint |

Stellaris, CK3, EU4, HOI4, Victoria 3, Imperator are all tier 1–2: same Clausewitz
syntax, same descriptor format, same launcher. That is the point of doing Stellaris
first — the second Paradox game is nearly free, and getting it working is the proof that
the abstraction is real rather than aspirational.

## The profile file

```toml
# profiles/stellaris.toml
[game]
id       = "stellaris"
name     = "Stellaris"
steam_id = 281990

[locations]
user_dir.windows = "{DOCUMENTS}/Paradox Interactive/Stellaris"
user_dir.linux   = "{XDG_DATA_HOME}/Paradox Interactive/Stellaris"
user_dir.macos   = "{HOME}/Documents/Paradox Interactive/Stellaris"
workshop         = "{STEAM_LIB}/steamapps/workshop/content/281990"
install          = "{STEAM_LIB}/steamapps/common/Stellaris"
mods_subdir      = "mod"

[load_order]
sources     = ["launcher_v2_sqlite", "dlc_load_json", "directory_scan"]
later_wins  = true

[syntax]
script       = "clausewitz"
localisation = "pdx_locyml"
descriptor   = "clausewitz_kv"

[version]
source  = "launcher-settings.json"
pattern = "wildcard_dotted"    # 3.13.*

[[dir]]
glob = "common/traits/**/*.txt"        ; domain = "trait"        ; merge = "merge_by_key"
[[dir]]
glob = "common/governments/civics/**"  ; domain = "civic"        ; merge = "merge_by_key"
[[dir]]
glob = "common/technology/**/*.txt"    ; domain = "technology"   ; merge = "merge_by_key"
[[dir]]
glob = "common/defines/**/*.txt"       ; domain = "define"       ; merge = "merge_by_key" ; level = 1
[[dir]]
glob = "common/on_actions/**/*.txt"    ; domain = "on_action"    ; merge = "merge_by_key"
[[dir]]
glob = "common/scripted_effects/**"    ; domain = "scripted_effect" ; merge = "merge_by_key"
[[dir]]
glob = "common/scripted_triggers/**"   ; domain = "scripted_trigger"; merge = "merge_by_key"
[[dir]]
glob = "events/**/*.txt"               ; domain = "event"        ; extractor = "pdx_event"
[[dir]]
glob = "localisation/**/*.yml"         ; domain = "loc"          ; parser = "pdx_locyml"
[[dir]]
glob = "interface/**/*.gui"            ; domain = "gui"          ; merge = "file_replace"
[[dir]]
glob = "gfx/**"                        ; domain = "asset"        ; parser = "none"

[benign]      # never worth reporting
paths = ["thumbnail.png", "descriptor.mod", "readme*", ".git/**", "*.md"]
```

(TOML written with `;` separators above purely for compactness in this doc; the real
files use one key per line.)

Roughly 60 `[[dir]]` entries cover Stellaris's `common/` tree. Writing them is
mechanical, and a `cargo xtask profile-audit <game-dir>` command lists directories
present in the vanilla install that the profile does not mention — so a new game patch
that adds a directory is caught by a tool run, not by a user's bug report.

### Candidate shortcut: CWTools config

CWTools maintains `.cwt` config files describing Stellaris's type system in far more
detail than the above — every type, its path, its key rules. Importing those instead of
hand-writing `[[dir]]` entries is worth investigating: it would give richer domain
coverage for free and stay current with their maintenance. **Check the licence before
vendoring or deriving**, and keep the importer behind `xtask` so the generated profile is
a checked-in artefact rather than a runtime dependency on their repo.

## The trait, for tiers 2 and 3

```rust
pub trait GameProfile: Sync {
    fn id(&self) -> &'static str;
    fn locations(&self, os: Os) -> LocationSpec;
    fn order_sources(&self) -> Vec<Box<dyn LoadOrderSource>>;
    fn classify(&self, path: &NormPath) -> Classification;   // domain + merge + extractor
    fn descriptor_reader(&self) -> &dyn DescriptorReader;
    fn version_reader(&self) -> &dyn VersionReader;
}
```

`TomlProfile` implements this generically for tier 1. Tier 2/3 games implement it
directly and reuse whatever parts of `TomlProfile` fit — usually all of `classify` and
none of `order_sources`.

## Beyond Paradox

Honest assessment of what generalises:

| Layer | Transfers to any game? |
| --- | --- |
| Discovery, Steam libraries, VFS, hashing | **Yes**, unchanged |
| File-overlay conflict detection | **Yes** — any game where mods are file trees |
| Load-order resolution | Yes, given an order source |
| Report, policy, GUI | **Yes**, entirely game-agnostic |
| Symbol-level analysis | **No** — needs a syntax adapter per format |

So a new game gets the file layer, load order, scoring, and the whole UI for the cost of
a TOML file. Symbol-level analysis — the layer with the real diagnostic value — costs a
parser. That is a fair trade and it is worth stating plainly rather than pretending
every game is a config edit.

Sketches for likely tier-2 targets:

| Game | Mod format | Adapter needed |
| --- | --- | --- |
| Crusader Kings III / EU4 / HOI4 | Clausewitz | none — TOML only |
| RimWorld | XML with XPath patch operations | XML adapter; patch ops are a *new fact kind*, `Patch { xpath, op }` |
| Factorio | Lua + `info.json` | Lua adapter; dependency graph is explicit and machine-readable, so `dep.*` rules get *better* |
| Bethesda (Skyrim/FO4) | Loose files + ESP/ESM records | Tier 3; binary record parser, but the domain model (records = symbols) fits well |
| Minecraft / Forge | JARs + JSON resources | Archive VFS source + JSON adapter |

The `FactPayload` enum being `#[non_exhaustive]` is what lets RimWorld's `Patch` variant
be added without breaking a single existing rule.

## Profile testing

Every shipped profile must pass:

1. **Round-trip** — the TOML loads and produces a `TomlProfile` with no unknown keys.
2. **Coverage** — against a fixture vanilla tree, ≥95% of files classify to a non-`Unknown`
   domain.
3. **Golden classification** — a fixed list of ~200 paths maps to expected
   `(domain, merge)` pairs; snapshot-tested so a profile edit shows its blast radius in
   the diff.
