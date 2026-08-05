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
glob = "common/traits/**/*.txt"        ; domain = "trait"        ; merge = "unknown"      # VERIFY
[[dir]]
glob = "common/governments/civics/**"  ; domain = "civic"        ; merge = "unknown"      # VERIFY
[[dir]]
glob = "common/technology/**/*.txt"    ; domain = "technology"   ; merge = "unknown"      # VERIFY
[[dir]]
glob = "common/defines/**/*.txt"       ; domain = "define"       ; merge = "unknown"      # VERIFY ; level = 1
[[dir]]
glob = "common/scripted_effects/**"    ; domain = "scripted_effect" ; merge = "unknown"   # VERIFY
[[dir]]
glob = "common/scripted_triggers/**"   ; domain = "scripted_trigger"; merge = "unknown"   # VERIFY
[[dir]]
glob = "events/**/*.txt"               ; domain = "event"        ; extractor = "pdx_event"
[[dir]]
glob = "localisation/**/*.yml"         ; domain = "loc"          ; parser = "pdx_locyml"
[[dir]]
glob = "interface/**/*.gui"            ; domain = "gui"          ; merge = "file_replace"
[[dir]]
glob = "gfx/**"                        ; domain = "asset"        ; parser = "none"

# on_action-shaped: the key resolves to one winner, but list-valued children accumulate
# across every source that declares the key.
[[dir]]
glob              = "common/on_actions/**/*.txt"
domain            = "on_action"
merge             = "merge_by_key_additive_body"
additive_children = ["events", "effect", "random_events"]

[benign]      # never worth reporting
paths = ["thumbnail.png", "descriptor.mod", "readme*", ".git/**", "*.md"]
```

(TOML written with `;` separators above purely for compactness in this doc; the real
files use one key per line.)

Roughly 60 `[[dir]]` entries cover Stellaris's `common/` tree. Writing them is
mechanical, and a `cargo xtask profile-audit <game-dir>` command lists directories
present in the vanilla install that the profile does not mention — so a new game patch
that adds a directory is caught by a tool run, not by a user's bug report.

## Merge mode is profile data, and most of it is unverified

`MergeMode` (§ `03`) is the lever the whole severity model turns on: the same raw
observation is a hard conflict under `FileReplace` and a non-event under `MergeByKey`.
Six values, and two of them exist because the first four were not enough:

| Value | Meaning | Reported as |
| --- | --- | --- |
| `file_replace` | A same-named file later in load order replaces the whole file | Hard conflict — unrelated entries are discarded too |
| `merge_by_key` | Entries merge across files; last definition of a key wins | Conflict only where keys actually collide |
| `merge_by_key_additive_body` | The key resolves to one winner, but the children named in `additive_children` **accumulate** from every source | Not a conflict at all, unless a non-additive child differs |
| `additive` | Duplicates coexist; nothing overrides anything | Never a conflict — reporting one would be a false positive |
| `single` | The directory is read as exactly one logical file | Conflict at directory granularity |
| `unknown` | No answer yet, or no glob matched | Detected, reported with reduced confidence, never asserted |

**`merge_by_key_additive_body` is not a refinement, it is a correctness fix.**
`on_game_start` is hooked by nearly every content mod that exists. Treating that
directory as replacement produces a single finding with ~200 participants, at High, which
is 100% false and would be the loudest thing in the report — the tool's own headline
would be its worst output. `additive_children` names the keys whose list bodies
concatenate rather than replace, so the finding fires only when two mods disagree about a
child that genuinely does not accumulate.

**`unknown` is a first-class answer.** The coverage target below is 95%, so ~5% of paths
match no glob by design; those get `unknown` rather than a default, because a default is
an assertion and this layer is where an unexamined assertion becomes a confident wrong
verdict. Findings on an `unknown` path are still emitted — the file and symbol layers do
not depend on merge semantics — but they carry `basis = unknown`, take the −15 modifier
(§ `09`), and the explanation says the outcome depends on how the game merges that
directory rather than naming a winner.

### Why the entries above say `unknown`

Every `merge` value marked `# VERIFY` is a claim nobody has tested against a running
game. The widely-repeated community answer for `common/` is same-filename replacement;
the equally widely-repeated answer for `common/defines/` is key-by-key merging; the two
cannot both be the general rule, and the design has at various points assumed each. Both
`common/defines` and `common/traits` are load-bearing — `pdx.defines_overlap` carries a
base weight of 75, and `core.symbol_collision` on a trait is the tool's canonical example.

Rather than pick, the shipped profile says `unknown` and **`docs/VERIFY.md` § Directory
merge behaviour is the single place the question is answered**. M0.5 (§ `15`) exists to
answer it empirically — two probe mods, both load orders, observe in game — and its exit
criterion is those values landing in `stellaris.toml` with the finding recorded. Until
then no document asserts a merge mode for these directories, and any doc that appears to
is out of date rather than authoritative.

The cost of being wrong is asymmetric and worth stating: a directory wrongly marked
`file_replace` inflates every finding in it; wrongly marked `merge_by_key` it suppresses
real breakage. `unknown` costs a hedge in the prose and nothing else.

### Candidate shortcut: CWTools config

CWTools maintains `.cwt` config files describing Stellaris's type system in far more
detail than the above — every type, its path, its key rules. Importing those instead of
hand-writing `[[dir]]` entries is worth investigating: it would give richer domain
coverage for free and stay current with their maintenance.

**This is blocked on a licence answer, and the answer is not the engine's.** The `.cwt`
config repository may carry different terms from the CWTools engine repository, so the
config repo's own licence is the one to read. Running an importer over those files and
checking the output into this repository produces a **derivative work** — that the
transformation is mechanical and the output is generated does not change it. So: read the
config repo's licence first, record the finding, and only then decide. If the terms do
not permit it, the fallback is hand-written entries informed by the wiki and by M0.5's own
observations, which is slower and is not blocked on anyone.

Keep the importer behind `xtask` regardless, so the generated profile is a checked-in
artefact rather than a runtime dependency on their repo.

## The trait, for tiers 2 and 3

```rust
pub trait GameProfile: Sync {
    fn id(&self) -> &'static str;
    fn locations(&self, os: Os) -> LocationSpec;
    fn order_sources(&self) -> &[SourceSpec];                // names, not impls
    fn classify(&self, path: &NormPath) -> Classification;
    fn descriptor_spec(&self) -> DescriptorSpec;
    fn version_spec(&self) -> VersionSpec;
}

pub struct Classification {
    pub domain:            Option<DomainId>,
    pub merge:             MergeMode,          // Unknown when no glob matched
    pub extractor:         ExtractorId,
    pub level:             u8,
    pub additive_children: &'static [SymbolName],  // empty unless MergeByKeyAdditiveBody
}
```

`order_sources` returns **names** (`"launcher_v2_sqlite"`, `"dlc_load_json"`,
`"directory_scan"`) exactly as the TOML already does, and `smd-ingest` owns the
name→impl registry. A profile that returned `Box<dyn LoadOrderSource>` would make
`smd-profiles` depend on `smd-ingest` while `smd-ingest` depends on `smd-profiles`, and
Cargo rejects that outright — it is not a lint failure, it is a build failure. The trait
objects live below, in `smd-core` (§ `02`).

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
   domain. The shortfall is not a rounding error to ignore: the uncovered paths are
   printed, and they are the ones whose findings carry `basis = unknown`.
3. **Golden classification** — a fixed list of ~200 paths maps to expected
   `(domain, merge, level, additive_children)` tuples; snapshot-tested so a profile edit
   shows its blast radius in the diff. `merge` is in the tuple because changing it changes
   severity across a whole directory, which is precisely the edit that should never pass
   unnoticed.
4. **`unknown` is accounted for** — every `[[dir]]` carrying `merge = "unknown"` has a
   matching open item in `docs/VERIFY.md`. A profile may ship unknowns; it may not ship
   unknowns nobody is tracking.
