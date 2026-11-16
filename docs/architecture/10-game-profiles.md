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
# profiles/stellaris.toml (abridged; the shipped file is the record)
[game]
id       = "stellaris"
name     = "Stellaris"
steam_id = 281990

[locations]
user_dir.windows = "{DOCUMENTS}/Paradox Interactive/Stellaris"
user_dir.linux   = "{XDG_DATA_HOME}/Paradox Interactive/Stellaris"
# macOS deferred (owner decision 2026-10-29); kept as reference.
user_dir.macos   = "{HOME}/Documents/Paradox Interactive/Stellaris"
workshop         = "{STEAM_LIB}/steamapps/workshop/content/281990"
install          = "{STEAM_LIB}/steamapps/common/Stellaris"
mods_subdir      = "mod"

[load_order]
sources     = ["launcher_v2_sqlite", "dlc_load_json", "directory_scan"]
later_wins  = true                             # rank component of the tuple
same_filename_replaces_whole_file = true       # Stage A default for every [[dir]]
filename_compare  = "ordinal"                  # byte order, case-sensitive
default_order     = "lios"
filename_key      = "unknown"                  # basename_noext | basename | dir_relative
vanilla_in_filename_axis = "unknown"           # true | false
replace_path_scope = "unknown"                 # vanilla_only | vanilla_and_earlier
in_file_duplicate  = "unknown"                 # first | last

[syntax]
script       = "clausewitz"
localisation = "pdx_locyml"
descriptor   = "clausewitz_kv"

[version]
source  = "launcher-settings.json"
pattern = "wildcard_dotted"    # 3.13.*

[[dir]]
glob = "common/traits/**/*.txt"   ; domain = "trait"    ; merge = "merge_by_key" ; order = "fios"
[[dir]]
glob = "common/buildings/**/*.txt"; domain = "building" ; merge = "merge_by_key"
[[dir]]
glob = "common/defines/**/*.txt"  ; domain = "define"   ; merge = "merge_by_key" ; level = 1
[[dir]]
glob = "events/**/*.txt"          ; domain = "event"    ; merge = "merge_by_key" ; order = "fios" ; extractor = "pdx_event"
[[dir]]
glob = "localisation/**/*.yml"    ; domain = "loc" ; parser = "pdx_locyml" ; scope = "loc" ; merge = "unknown" ; same_path = "unknown"
[[dir]]
glob = "localisation/replace/**/*.yml" ; domain = "loc" ; parser = "pdx_locyml" ; scope = "loc" ; tier = 1 ; merge = "unknown" ; same_path = "unknown"
[[dir]]
glob = "gfx/**"                   ; domain = "asset"    ; parser = "none" ; merge = "path_only"

# on_action-shaped: the key resolves to one winner, but the named children accumulate.
# Selectable only once the extractor emits AdditiveSplit (§ 03, Invariant A1), and only
# once the child set is probed. Until both, the shipped profile says "unknown".
[[dir]]
glob   = "common/on_actions/**/*.txt"
domain = "on_action"
merge  = "unknown"
# merge = "merge_by_key_additive_body" ; additive_children = ["events"]

[benign]      # never worth reporting
paths = ["thumbnail.png", "descriptor.mod", "readme*", ".git/**", "*.md", "/*", ".*", "**/.*/**"]

[asset_archives]   # containers declared to hold no script
paths = ["dlc/*/*.zip"]
```

(TOML written with `;` separators above purely for compactness in this doc; the real
files use one key per line.)

Roughly 60 `[[dir]]` entries cover Stellaris's `common/` tree. Writing them is
mechanical, and a `cargo xtask dir-audit <game-dir>` command (not yet built — gated H39,
M1; the name `profile-audit` belongs to the release-profile lint gate) lists directories
present in the vanilla install that the profile does not mention — so a new game patch
that adds a directory is caught by a tool run, not by a user's bug report.

### Keys, by resolution stage

Each key feeds exactly one stage of the resolver (§ `07`). A key the loader does not
recognise is a load **error**, not a warning: silently dropping `order = "fios"` inverts
the verdict for fourteen directories.

| Key | Where | Stage | Values | Meaning |
| --- | --- | --- | --- | --- |
| `same_filename_replaces_whole_file` | `[load_order]` | A | bool | Default for every `[[dir]]`'s `same_path` |
| `same_path` | `[[dir]]` | A | `replace` · `unknown` | Whether an identical root-relative path replaces the loser's whole file |
| `vanilla_is_lowest` | `[load_order]` | A | bool | Vanilla loses every same-path contest. Says nothing about differently-named files |
| `replace_path_scope` | `[load_order]` | A | `vanilla_only` · `vanilla_and_earlier` · `unknown` | What a `replace_path` drops |
| `merge` | `[[dir]]` | C | see below | Cross-file behaviour only |
| `additive_children` | `[[dir]]` | C | list | Children that accumulate; requires `merge_by_key_additive_body` and an `AdditiveSplit` extractor |
| `scope` | `[[dir]]` | C | name | Entries sharing a scope compete for the same keys; default: one scope per entry |
| `tier` | `[[dir]]` | D | 0–255, default 0 | Beats every lower tier in the same scope, before filename or load order |
| `order` | `[[dir]]` | D | `lios` · `fios` · `unknown` | Filename direction; absent = `default_order` |
| `default_order` | `[load_order]` | D | `lios` · `fios` · `unknown` | `order` for every `[[dir]]` that does not set it |
| `filename_compare` | `[load_order]` | D | `ordinal` | Only ordinal is defined |
| `filename_key` | `[load_order]` | D | `basename_noext` · `basename` · `dir_relative` · `unknown` | What the filename axis compares |
| `vanilla_in_filename_axis` | `[load_order]` | D | bool · `unknown` | Whether vanilla's differently-named files take part, or vanilla always ranks lowest |
| `in_file_duplicate` | `[load_order]` | D | `first` · `last` · `unknown` | Same key twice in one file |
| `later_wins` | `[load_order]` | D | bool | Direction of the `rank` component |
| `paths` | `[benign]` | A | glob list | Files never worth reporting. They are counted and inventoried but never join the overlay, so they cannot be a same-path finding. A pattern without `/` matches the file name at any depth, and a leading `/` anchors a pattern to the source root. Files directly at a source root (`/*`: `bigpicture.jpg`, thumbnails, readmes) are benign for the same reason: Stellaris loads game content only from subdirectories, so a root file is inventoried and counted but never overlaid. Dot-files and dot-directories (`.*`, `**/.*/**`) are listed because a mod author's repository and editor state (`.gitattributes`, `.idea/**`) is not game content |
| `paths` | `[asset_archives]` | — | glob list | Archives the profile declares hold no script, so an oversize one does not make the scan incomplete. Honoured for the vanilla source only; a mod's oversize file of the same shape stays `Unparsed(Oversize)`. The archive is still reported as not read (`scan.asset_archive_unread`), never silently dropped (§ `05`, § `06`) |
| `verified` | `[[dir]]` | — | `"<VERIFY id or probe id> @ <game version>"` | The entry's values were observed in that build. Absent = `SemanticCertainty::Assumed` for findings in that directory; rejected on a `merge = "unknown"` entry. Earned per entry, never defaulted |
| `*_confidence` | `[[dir]]` / `[load_order]` | — | `corroborated` · `single_source` | Prior-art strength, for the reader; never read by the resolver |

Every `unknown` is resolved by enumeration, not by default (§ `07`): the verdict is stated
when all readings agree, and otherwise carries the `ResolutionGap` naming the key.


## Merge mode is profile data, and most of it is unverified

`MergeMode` (§ `03`) is the lever the whole severity model turns on — for the
**cross-file** axis. Same-path whole-file replacement is not one of its values: it happens
in every directory whose `same_path` is `replace`, before `merge` is consulted (§ `07`,
Stage A). An earlier version of this table offered `file_replace` *instead of*
`merge_by_key`; Stellaris does both in one directory, selected by filename identity, so
that choice could not be made correctly for `common/traits` or any other script
directory (REVIEW-2026-10, H1). Six values:

| Value | Meaning | Reported as |
| --- | --- | --- |
| `path_only` | No key layer; differently-named files never interact (binary assets) | Only same-path conflicts, from Stage A |
| `merge_by_key` | Definitions merge across files; the highest-**precedence** definition of a key wins — filename axis first, then load order (§ `03` Precedence) | Conflict only where keys actually collide |
| `merge_by_key_additive_body` | The key resolves to one winner, but the children named in `additive_children` **accumulate** from every source. Requires `AdditiveSplit` (§ `03`, Invariant A1) | Not a conflict at all, unless the residual (non-additive children) differs |
| `additive` | Duplicates coexist; nothing overrides anything | Never a conflict — reporting one would be a false positive |
| `single` | The directory is read as exactly one logical file | Conflict at directory granularity |
| `unknown` | No answer yet, or no glob matched | Detected, reported as `SemanticCertainty::Unmodelled` (§ `03`), never asserted |

**`merge_by_key_additive_body` is not a refinement, it is a correctness fix.**
`on_game_start` is hooked by nearly every content mod that exists. Treating that
directory as replacement produces a single finding with ~200 participants, at High, which
is 100% false and would be the loudest thing in the report — the tool's own headline
would be its worst output. `additive_children` names the keys whose list bodies
concatenate rather than replace, so the finding fires only when two mods disagree about a
child that genuinely does not accumulate.

That last clause is only computable with child-level facts. A whole-definition
`body_hash` differs whenever two mods add different events — the harmless case — and
equally when they disagree on a non-accumulating child, so the mode cannot be
implemented from it (REVIEW-2026-10, finding 9). It is therefore **gated**: the loader
rejects `merge = "merge_by_key_additive_body"` unless the `[[dir]]`'s extractor declares
`AdditiveSplit` support, and the shipped profile keeps `common/on_actions` at `unknown`
until both that extractor (H4) and the probe of the child set (`VERIFY` V18) land.

**`unknown` is a first-class answer.** The coverage target below is 95%, so ~5% of paths
match no glob by design; those get `unknown` rather than a default, because a default is
an assertion and this layer is where an unexamined assertion becomes a confident wrong
verdict. Findings on an `unknown` path are still emitted — Stage A does not depend on
merge semantics, so a same-path conflict there is still stated — but key-level findings
carry `ResolutionGap::MergeModeUnknown` and `SemanticCertainty::Unmodelled` (§ `03`), are
capped by the certainty caps (§ `09`), and the explanation says the outcome depends on how the game merges that directory rather
than naming a winner.

### What the shipped values rest on

The original question this section posed — "is `common/` same-filename replacement or
key-by-key merging?" — dissolved under prior art: it is both, selected by filename
identity, and the model above is built on that answer (`GROUND-TRUTH-PRIOR-ART.md`
§ 3.4, § 5.1). What did *not* dissolve is the evidence class of each value, and the
profile carries it honestly:

| Class | Examples | In the profile |
| --- | --- | --- |
| Prior art, corroborated | same-path whole-file replacement in `common/` and assets (`VERIFY` V15); defines per key (V16); traits per object (V17); a filename axis exists, ordinal (V21) | Stated values, `*_confidence = "corroborated"`, no `verified` |
| Prior art, single source (Irony) | the per-directory `merge_by_key` map (V14); the fourteen FIOS directories (V02); `localisation/replace/` per key (V20); filename key = basename without extension (V37); vanilla in the filename axis outside localisation (V09) | Stated values with `single_source` where the resolver can tolerate being wrong at a cost of one hedge; `unknown` where being wrong inverts verdicts (V09, V37) |
| No source, or sources disagree | on_actions child set (V18); `replace_path` scope (V10); in-file duplicates (V38); same-path for localisation (V39); the `single` hypotheses | `unknown` |

**Nothing in the profile is empirically verified**, so no `[[dir]]` carries `verified`
and every directory's findings are at best `SemanticCertainty::Assumed`. M0.5's probe pass
(§ `15`) is what earns `verified`, one entry at a time, stamped with the build it was
observed in.

The fourteen FIOS entries ship as `order = "fios"` rather than `unknown` despite resting
on one source. The reasoning is the asymmetry: with `order = "unknown"` the resolver
would enumerate both directions and every differently-named-file conflict in those
directories would become `Unknown` — the honest answer, but the same as having no
model. With `fios` stated and `single_source` recorded, the cost of being wrong is
inverted winners in fourteen directories, capped at `Assumed` and called out in the
explanation. That trade is revisited the day probe P3 runs (`VERIFY` V02).

The cost of being wrong is asymmetric and worth stating: a directory wrongly marked
`path_only` hides every key collision in it; wrongly marked `merge_by_key` it invents
collisions that the game resolves silently; a wrong `order` inverts every winner.
`unknown` costs a hedge in the prose and nothing else.

### CWTools config: licence cleared, fitness not

The `.cwt` config repository is MIT, like the engine (`GROUND-TRUTH-PRIOR-ART.md` § 1), so
an importer is not blocked by licence. It is blocked by fitness: the config is a type
system and encodes no override semantics, so it can enumerate directories worth a
`[[dir]]` entry but cannot supply `merge`, `order`, `same_path`, or `tier`. If an
importer is built it lives behind `xtask`, generates the directory inventory only, and
its output is a checked-in artefact rather than a runtime dependency on their repo.

## The trait, for tiers 2 and 3

```rust
pub trait GameProfile: Sync {
    fn id(&self) -> &'static str;
    fn locations(&self, os: Os) -> LocationSpec;
    fn order_sources(&self) -> &[SourceSpec];                // names, not impls
    fn classify(&self, path: &NormPath) -> Classification;
    fn descriptor_spec(&self) -> DescriptorSpec;
    fn version_spec(&self) -> VersionSpec;
    fn order_rules(&self) -> OrderRules;                     // the [load_order] Stage A/D keys
}

pub struct Classification {
    pub domain:            Option<DomainId>,
    pub rules:             DirRules,           // merge, same_path, order, tier, scope,
                                               // verified (§ `07`); merge Unknown when no
                                               // glob matched
    pub extractor:         ExtractorId,
    pub level:             u8,
    pub additive_children: &'static [SymbolName],  // empty unless MergeByKeyAdditiveBody
}

/// Profile-wide parameters. Each `Option`/`Unknown` is enumerated by the resolver.
pub struct OrderRules {
    pub later_wins:               bool,
    pub default_order:            FileOrder,
    pub filename_key:             Option<FilenameKey>,     // BasenameNoExt | Basename | DirRelative
    pub vanilla_in_filename_axis: Option<bool>,
    pub replace_path_scope:       Option<ReplacePathScope>, // VanillaOnly | VanillaAndEarlier
    pub in_file_duplicate:        Option<InFileDuplicate>,  // First | Last
}
```

**Overlapping globs.** `localisation/**` matches every file `localisation/replace/**`
does. Classification takes the most specific matching entry (longest literal prefix,
then declaration order), and the loader rejects two entries that tie. Specificity picks
the entry — and so the `tier` — while a shared `scope` keeps both entries' keys in one
competition (§ `07`).

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
   printed, and they are the ones whose key-level findings carry `merge_mode_unknown`.
3. **Golden classification** — a fixed list of ~200 paths maps to expected
   `(domain, merge, same_path, order, tier, scope, level, additive_children)` tuples; snapshot-tested so a profile edit
   shows its blast radius in the diff. `merge` is in the tuple because changing it changes
   severity across a whole directory, which is precisely the edit that should never pass
   unnoticed.
4. **`unknown` is accounted for** — every `[[dir]]` carrying `merge = "unknown"` has a
   matching open item in `docs/VERIFY.md`. A profile may ship unknowns; it may not ship
   unknowns nobody is tracking. The same holds for each `unknown` in `[load_order]`.
5. **Precedence goldens** — the worked cases in § `07` (same-path, FIOS vs LIOS, one mod's
   two files, `localisation/replace/`, vanilla in a FIOS directory) run as resolver
   fixtures against the shipped profile, so a profile edit that flips a winner fails a
   test rather than a user.
6. **Rejections are tested** — `merge_by_key_additive_body` without an `AdditiveSplit`
   extractor, `verified` on a `merge = "unknown"` entry, an unrecognised key, and two
   tying globs each fail to load.
