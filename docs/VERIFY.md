# Assumptions to verify against a live install

This design was written without a Stellaris installation to hand. The items below are
the load-bearing factual claims. Each is encoded as **data** (in `profiles/stellaris.toml`)
rather than code specifically so that correcting one is a config edit, but a wrong
assumption here produces confidently wrong output, which is the worst failure mode this
tool has.

**This file is the exit criterion for M0.5 (§ `architecture/15`), which blocks M1.** Not
"before M1 ships" — before M1 starts. Confirm each item, **record the finding inline in
this file** next to the box, and land the answer in `profiles/stellaris.toml` where it has
a home there. An answer recorded only in an issue or a commit message is an answer the
next person re-derives.

---

## Status of this pass (2026-08-06) — prior art, not empiricism

The empirical route (probe mods, both load orders, observe in game) was blocked, so this
pass took the **second route named in the Method section below**: deriving the answers
from prior art that already encodes them. See `docs/GROUND-TRUTH-PRIOR-ART.md` for the
projects consulted, their licences, and where they disagreed.

**Prior-art corroboration is not empirical verification.** Every finding below is a
statement about what two or more independent implementations or documents *believe*
Stellaris does. Irony Mod Manager in particular has been shipping against real installs
for six years and its beliefs are load-bearing for thousands of users, which makes it
strong evidence — but it is still evidence, not observation. Two of the items below are
marked with Irony's *own* hedging language, because its author wrote the code and left a
comment saying he was guessing.

**The corroborated items are sufficient to build against.** They are also sufficient to
have already invalidated part of the design (see the three contradictions flagged
`⚠ DESIGN CONTRADICTION` below). The empirical pass still has to happen; its scope is now
much smaller and much better targeted, and `docs/GROUND-TRUTH-PRIOR-ART.md` closes with a
ranked list of exactly what the probe mods must resolve.

Each item carries exactly one of three states:

| State | Meaning | Box |
| --- | --- | --- |
| `CORROBORATED` | Two or more independent sources agree | ticked |
| `SINGLE SOURCE` | Only one source speaks to it | unticked |
| `NEEDS THE GAME` | Sources disagree, or none address it | unticked |

Tally: **7 corroborated, 22 single-source, 7 need the game** — 36 items. That is one more
box than this file started with: the FIOS/LIOS filename axis (second item below) is a new
question that prior art surfaced and the original list did not know to ask.

The shape of that tally is itself a finding. Corroboration is scarce because **there is
really only one deep source of Stellaris override semantics in the world — Irony Mod
Manager.** CWTools is a validator and models a type system rather than merge behaviour;
the wiki is thin on precedence; tiger does not support Stellaris at all. Twenty-two
single-source items are twenty-two places where we are trusting one project's reading of
an undocumented engine. That is better than trusting our own guess, and much worse than
observation.

---

## Load order semantics — **highest priority**

- [x] **Does a mod later in the playset override an earlier one, or the reverse?**
      **CORROBORATED — later wins.** Irony orders every candidate definition by
      `modOrder.IndexOf(modName)` ascending and then takes the **last** element as the
      winner when the competing files share a name, labelling the outcome `ModOrder`;
      its SQLite exporter appends newly-added mods (including its own generated
      conflict-resolution patch mod, which must beat everything) at the **highest**
      `position` values. CWTools sorts vanilla to the end of its precedence list so that
      mods always come out on top. The community consensus, including the most-cited
      Steam load-order guide, is "mods at the top of the list are loaded first, mods
      lower down will overwrite". Three independent sources agree.
      **⚠ But see the next item — "later wins" is not the whole rule.**
      Sources: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/ModBaseService.cs,
      https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/Exporter/SQLiteExporter.cs,
      https://github.com/cwtools/cwtools/blob/master/CWTools/Game/ResourceManager.fs,
      https://steamcommunity.com/sharedfiles/filedetails/?id=2290164696

- [ ] **⚠ DESIGN CONTRADICTION — load order is not the only precedence axis, and for
      some directories it is not even the deciding one.**
      **SINGLE SOURCE (Irony).** Irony resolves a conflict in two stages. First, among
      candidates sharing the *same* relative filename, the later mod in load order wins
      (that is the item above). Second, among candidates in *differently named* files,
      the winner is chosen by **ordinal filename sort**, and the direction depends on the
      directory: most directories are **LIOS** (last name alphabetically wins), but a
      named set of Stellaris directories are **FIOS** (first name alphabetically wins).
      Irony names the concepts explicitly (`DefinitionUsesFIOSRules`, `PriorityType.FIOS`,
      `PriorityType.LIOS`) and generates winning filenames by prefixing `!!!_` for FIOS
      and `zzz_` for LIOS. The Stellaris FIOS list it ships is:
      `common/component_sets`, `common/component_templates`, `common/event_chains`,
      `common/global_ship_designs`, `common/scripted_variables`, `common/section_templates`,
      `common/ship_behaviors`, `common/special_projects`, `common/strategic_resources`,
      `events`, `common/solar_system_initializers`, `common/traits`,
      `common/start_screen_messages`, `common/governments/authorities`.
      **This means a mod placed *later* in the playset can still lose a `common/traits`
      conflict if its filename sorts later.** `[load_order] later_wins = true` in the
      profile is therefore true but incomplete, and the design has no representation for
      the filename axis at all. Only Irony encodes the FIOS path list; the community
      corroborates that filename order matters (below) but not which directories invert.
      Sources: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/InfoProviders/StellarisDefinitionInfoProvider.cs,
      https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/InfoProviders/BaseDefinitionInfoProvider.cs

- [ ] Where does the current launcher store the active playset and its ordering?
      **SINGLE SOURCE (Irony).** `launcher-v2.sqlite` in the game's user directory, with
      `launcher-v2_openbeta.sqlite` for the open beta branch. Ordering lives in a
      `playsets_mods` join table with columns `playsetId`, `modId`, `position`, `enabled`;
      mods are in `mods` and playsets in `playsets`. **`position` changed type across
      launcher versions** — older schemas stored it as a zero-padded 10-character
      hexadecimal *string*, newer ones as an integer, so a reader that assumes integer
      will mis-sort an old DB and one that assumes string will mis-sort a new one.
      Ascending `position` is ascending load order.
      Sources: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/Models/Paradox/v4/PlaysetsMods.cs,
      https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/Exporter/SQLiteExporter.cs

- [ ] Confirm the launcher DB runs in **WAL** mode, and that a playset reordered in the
      open launcher is visible only after `-wal` is replayed. Test the copy protocol
      (§ `architecture/05`) against a launcher that is open and has just been used: copy
      main-file-only and all-three, compare. If main-file-only ever returns the *new*
      order, say why — do not assume the protocol was unnecessary.
      **NEEDS THE GAME.** No consulted source addresses journal mode. Irony connects to
      the DB through an ORM without setting or inspecting `journal_mode`, and neither
      CWTools nor tiger reads the launcher DB at all. The copy protocol is unvalidated
      in either direction: nothing corroborates it and nothing contradicts it.

- [ ] Does the launcher hold a lock that prevents even a file copy on Windows?
      **NEEDS THE GAME.** No source addresses it. Irony writes to the launcher DB
      directly rather than copying it, and its documented failure mode is "close the
      launcher first", which is consistent with a lock but does not establish one.

- [ ] Record the schema version marker (`user_version`, a migrations table, whatever
      exists) that the reader can check to detect a launcher update.
      **SINGLE SOURCE (Irony).** It is **not** `user_version`. The launcher is a
      Knex-based JavaScript application and carries a **`knex_migrations` table of named
      migrations**; Irony detects schema generation by testing for the presence of
      specific migration names, three of which it hardcodes:
      `addCreatedAndSubscribedDate`, `modifyPositionToInteger` (the hex-string→integer
      change noted above), and `addModShortDescription`. So the marker is a *set of
      names*, not a monotonic integer, and a reader must test for the migrations it
      cares about rather than compare a version number.
      Sources: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/Constants.cs,
      https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/Models/Paradox/v2/KnoxMigrations.cs

- [ ] Is the launcher's displayed order the load order, or is it re-sorted (by name, by
      dependency) before being handed to the game?
      **SINGLE SOURCE (Irony).** Irony treats ascending `position` as the load order
      handed to the game, and rewrites `position` contiguously from zero when it exports
      a collection. It applies no name or dependency re-sort of its own. This is
      consistent with the displayed order being the load order, but Irony *writes* the
      order rather than observing what the game consumes, so it cannot rule out a
      re-sort inside the game.

- [ ] Does `dlc_load.json` still reflect the active playset in current versions, or is it
      legacy?
      **SINGLE SOURCE (Irony).** Still live, and still written. Irony maintains both a
      JSON exporter and a SQLite exporter and writes `dlc_load.json` alongside
      `launcher-v2.sqlite`, together with `game_data.json`, `content_load.json`,
      `mods_registry.json` and `playsets.json`. That it is still maintained in a
      2026-active codebase is strong evidence it is not vestigial, but no source states
      which of the two the *game* actually reads when they disagree — and that
      disagreement is exactly the case this tool must diagnose.

- [x] Where does vanilla sit in the precedence chain relative to mods? (Design assumes
      `SourceId(0)`, i.e. always lowest.)
      **CORROBORATED — vanilla is always lowest.** CWTools tags vanilla content with the
      `embedded` scope and sorts it to the end of the precedence list, so a mod entity
      always takes `Overwrote` and the vanilla one always takes `Overwritten`. Irony
      carries an `IsFromGame` flag on every definition and, wherever a tie or fallback
      arises, deliberately prefers non-game definitions and pushes game definitions to
      the back; it also strips vanilla definitions that collide with a mod's on filename.
      Two independent projects agree. The design's `SourceId(0)` assumption holds.
      Sources: https://github.com/cwtools/cwtools/blob/master/CWTools/Game/ResourceManager.fs,
      https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/ModBaseService.cs

## `replace_path`

- [ ] Confirm exact semantics: does it drop the vanilla directory only, or also that
      directory's contents from *all* mods loading earlier?
      **SINGLE SOURCE (Irony), and Irony itself flags it as a guess.** Irony's merge
      service excludes a mod's files under a replaced path when that mod sits *earlier*
      in the collection than the last mod declaring the same `replace_path` — i.e. it
      implements "drops vanilla **and** every earlier mod's contents at that path". The
      code comment states the uncertainty in the author's own words: *"Believe that
      handling of replace_path in the game is so any mod before does not copy its output
      *only* the ones following with replace path will be copied."* No second source
      addresses it. **This is the single highest-value item for the empirical pass**: it
      is the difference between `replace_path` being a narrow vanilla-only exclusion and
      it being a load-order-wide guillotine, and the tool's blast-radius reporting
      changes completely between the two.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/ModMergeService.cs

- [ ] Is it relative to the game root, and does it match recursively?
      **SINGLE SOURCE (Irony).** Relative to the game root, and matched as a
      case-insensitive **path prefix** against each file's root-relative path — which
      makes it recursive by construction, and also means `replace_path = "common/tra"`
      would match `common/traits/`. Whether the game is equally naive about partial
      segment matches is untested. The EU4 wiki's `replace_path = "missions"` example is
      consistent with root-relative whole-directory semantics but does not speak to
      recursion.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/ModMergeService.cs

- [ ] Can a mod declare several? (Design assumes yes and accumulates.)
      **SINGLE SOURCE (Irony) — design assumption holds.** Irony parses `replace_path`
      with its repeated-key accessor (`GetKeyedValues`) rather than its single-value
      accessor, and models it as a list on the mod object, so multiple declarations in
      one descriptor accumulate. `user_dir` is handled the same way. No second source
      addresses multiplicity.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Parser/Mod/ModParser.cs

- [ ] What happens when two mods declare `replace_path` for the same directory?
      **NEEDS THE GAME.** Only Irony speaks to it, and only as a consequence of the
      guess above: it takes the **last** mod declaring that path as the cutoff, so every
      mod before it is excluded and every mod after it contributes normally. Nothing
      corroborates this, and it follows from a belief the author explicitly hedged.
      Treat as unknown.

## Directory merge behaviour

**This section is the single source of truth for merge modes.** Where another document
appears to assert one, this file wins and that document is out of date. Every entry marked
`merge = "unknown"` in `profiles/stellaris.toml` corresponds to a box below.

- [ ] For each `[[dir]]` entry in `profiles/stellaris.toml`, confirm the `merge` mode.
      The critical distinction: directories where a same-named file **replaces** the
      whole file, versus directories where entries **merge by key** across files.
      **SINGLE SOURCE (Irony) for the per-directory map; the general rule is
      corroborated separately below.** The framing of this question turns out to be a
      false dichotomy — see the next item. Irony assigns a parser per directory and that
      assignment is a de-facto merge-mode table: most of `common/` gets a key parser
      (one definition per top-level key ⇒ key merge), `common/defines` and
      `localisation/` get per-key variable parsers, `common/on_actions` gets a
      whole-text parser (file is the unit), and two Stellaris-specific parsers mark
      object-level and single-file-per-directory behaviour. Those assignments are
      transcribed into `profiles/stellaris.toml`. Directories Irony does not
      special-case are `merge = "unknown"` in the profile.
      Source: https://github.com/bcssov/IronyModManager/tree/master/src/IronyModManager.Parser

- [x] Confirm the widely-repeated claim that same-filename replacement is the rule for
      most of `common/`, and identify the exceptions.
      **CORROBORATED — but the rule is conditional on filename identity, and that is
      not what the design models.** All three sources agree on a two-part rule:
      (a) across files with **different** names, entries merge and the last-resolved
      definition of a key wins; (b) across files with the **same** root-relative name,
      the winning file replaces the earlier ones **wholesale**, so keys present only in
      an earlier copy are lost. Irony implements (b) as an explicit gate: for each
      definition it finds the highest-load-order mod shipping that same filename and
      discards the definition if that mod's copy of the file does not contain it
      (`existsInLastFile`). CWTools groups entities by `logicalpath` — the root-relative
      path — and marks all but the winner `Overwritten`, i.e. file-identity is its unit
      of override. The community statement of the same rule: *"If the keys are the same,
      the keys are usually just simply overridden by the mod that is loaded last.
      However, when the file names are exactly the same, the paradox launcher will
      attempt a full file override, rather than just overriding individual keys."*
      **⚠ DESIGN CONTRADICTION: `MergeMode` cannot express this.** `file_replace` and
      `merge_by_key` are modelled in § `architecture/03` and § `architecture/10` as
      mutually exclusive properties *of a directory*. They are not — they are two
      behaviours of the *same* directory, selected per pair of files by whether the
      names match. A profile that marks `common/traits` as `merge_by_key` will miss the
      real and severe case of two mods both shipping `00_traits.txt`; one that marks it
      `file_replace` will inflate every ordinary trait overlap. This needs a design
      decision, not a profile edit. `profiles/stellaris.toml` records the cross-file
      behaviour in `merge` and adds a profile-level
      `[load_order] same_filename_replaces_whole_file = true`, flagged there as a
      proposed schema addition.
      Sources: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/ModPatchCollectionService.cs,
      https://github.com/cwtools/cwtools/blob/master/CWTools/Game/ResourceManager.fs,
      https://steamcommunity.com/app/281990/discussions/0/4427688685166331427/

- [x] `common/defines/` specifically: does a mod's defines file replace the whole vanilla
      set, or merge key-by-key? Two mods each shipping a one-key defines file is the test.
      `pdx.defines_overlap` carries a base weight of 75 and this answer sets whether that
      is right or is 75 points of noise.
      **CORROBORATED — merges key-by-key.** Irony parses `common/defines` and
      `unchecked_defines` with a dedicated parser that emits **one definition per
      individual define key**, typed `SpecialVariable`, with the namespace
      (`NGameplay`, `NShip`, …) as the definition type and the leaf key as the id —
      including flattening dot-notation and the Lua-style nested form. Conflicts are
      therefore reported at individual-define granularity, not file granularity. The
      community rule quoted above says the same for keys generally. CWTools' Stellaris
      config independently models each defines namespace as its own type rather than
      treating the file as one object.
      **⚠ Consequence for scoring: `pdx.defines_overlap` at base weight 75 is wrong as
      specified if it fires on files overlapping.** It should fire on *keys* colliding.
      Two mods each shipping a one-key defines file with different keys is a non-event
      and must score zero. The 75 is defensible only for a genuine same-key collision,
      or for two mods both shipping a file named `00_defines.txt` — which is the
      filename-identity case above and is genuinely severe.
      Sources: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Parser/Generic/DefinesParser.cs,
      https://github.com/cwtools/cwtools-stellaris-config/blob/master/config/common/defines.cwt,
      https://steamcommunity.com/app/281990/discussions/0/4427688685166331427/

- [x] `common/traits/` specifically: same question. It is the design's canonical example
      and has been written both ways.
      **CORROBORATED — merges per trait object, not per file.** Irony routes
      `common/traits` to its Stellaris `OverwrittenParser`, which emits one definition
      per top-level trait object and tags it `OverwrittenObject` — meaning a later
      definition of `trait_intelligent` replaces that trait *object* whole (its child
      keys are not merged), while other traits in the same file are untouched. CWTools'
      Stellaris config independently models `type[trait]` with `path = "game/common/traits"`
      and a per-object key, i.e. the trait id is the unit of identity, not the file.
      Two independent projects agree.
      **⚠ Two caveats that the design's canonical example gets wrong.** First, the
      *body* of a trait does not merge — a mod redefining `trait_intelligent` to change
      one modifier silently drops every other key in the vanilla trait. Second,
      `common/traits` is **FIOS** (see the second item in Load order semantics), so the
      winner among differently-named trait files is the one whose filename sorts
      **first**, not last. Using traits as the canonical "later mod wins" example in the
      docs is therefore actively misleading.
      Sources: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Parser/Games/Stellaris/OverwrittenParser.cs,
      https://github.com/cwtools/cwtools-stellaris-config/blob/master/config/common/traits.cwt

- [ ] **Additive-body directories.** In `common/on_actions/`, when two mods both declare
      `on_game_start`, do the `events`/`effect` lists **accumulate**, or does the later
      declaration replace the earlier one wholesale? Which child keys accumulate and which
      do not? The answer becomes `additive_children` in the profile. Nearly every content
      mod hooks `on_game_start`, so getting this wrong produces one ~200-participant High
      finding that is entirely false.
      **SINGLE SOURCE (wiki) for the mechanism; the "not a conflict" conclusion is
      corroborated.** The wiki states that mods *"can provide their own, uniquely named,
      on_action file whose events are called in addition [to] the vanilla events"*, and
      that where an action is registered in several files the events fire in
      *"ASCII-betical order"* by filename — i.e. **`events` lists accumulate across
      distinct files**. Irony reaches the same practical outcome by a different
      mechanism: it parses `common/on_actions` with its **whole-text** parser and keys
      the definition on the **filename**, so two mods hooking `on_game_start` in
      differently named files produce different definition ids and **no conflict is
      reported at all**. So the design's core fear is confirmed unfounded, from two
      directions.
      **But the box stays open, because the part that becomes profile data is
      unanswered.** Only one source (the wiki) speaks to *which* child keys accumulate,
      and it names only `events`. Nothing corroborates that `effect` or `random_events`
      accumulate — and there is good reason to doubt `effect` does, since an effect block
      is a statement list rather than a set of ids. The design's proposed
      `additive_children = ["events", "effect", "random_events"]` is therefore two-thirds
      unsupported and must not ship as an assertion.
      **⚠ Also note what "additive" does *not* cover:** two mods both shipping
      `common/on_actions/00_on_actions.txt` — the vanilla filename — hit the
      filename-identity rule, and the later one drops the earlier one's entire file,
      including vanilla's. That is a real, severe conflict, and § `architecture/10`'s
      claim that this directory is *"not a conflict at all"* is wrong in that case.
      Sources: https://stellaris.paradoxwikis.com/Game_start_modding,
      https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Parser/Generic/WholeTextParser.cs

- [ ] **Are any directories genuinely `Additive`** — duplicates coexist and *both* apply,
      with nothing overriding anything? If so, list them. Reporting those as conflicts is
      a false positive by construction, no matter how the rest of the model behaves.
      **SINGLE SOURCE (wiki), one directory only.** `common/on_actions` behaves
      additively across distinct filenames, per the item above. No source identifies any
      other genuinely additive directory in Stellaris, and neither Irony nor CWTools has
      a concept corresponding to "duplicates coexist" for any other path — Irony's
      nearest equivalent is a narrow per-directory allow-list of duplicate ids covering
      exactly one case (`ethic_categories` under `common/ethics`), which is a
      duplicate-key tolerance rather than additive semantics. Treat the additive set as
      "on_actions, and possibly nothing else" until probed.

- [ ] Does `localisation/replace/` take precedence over `localisation/` as assumed? Does
      it work per-key, per-file, or per-directory? This is a path relation the overlay
      cannot currently express, so the answer decides how much machinery it needs.
      **SINGLE SOURCE (Irony) — the assumption holds, and it is per-key.** Irony special-cases
      localisation ahead of all its other precedence logic: for a given loc key, if *any*
      candidate definition lives under a `replace` directory, it discards every candidate
      that does not, then resolves among the survivors. So `localisation/replace/` wins
      over `localisation/` **regardless of load order**. The unit is the individual YAML
      key — Irony's localisation parser emits one definition per key — so a
      `replace` file overrides only the keys it actually contains, not the whole file or
      directory. Irony also carries a numeric `CustomPriorityOrder` applied before mod
      order within localisation, which suggests further structure this pass did not
      resolve.
      **Machinery implication: the overlay needs a per-key path-precedence rule for
      exactly one path relation, not a general mechanism.**
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/ModBaseService.cs

- [x] **Intra-source ordering.** Within one mod, does the `00_`/`zz_` filename convention
      actually affect load order, or is it convention only? If it is real, ordering inside
      a source is a second precedence axis the model does not have.
      **CORROBORATED — it is real, and it is a second precedence axis.** Irony sorts
      candidate filenames with an **ordinal** string comparison and picks the first or
      last depending on the directory's FIOS/LIOS classification; when it generates a
      file that must win, it prefixes `!!!_` (to sort first, for FIOS directories) or
      `zzz_` (to sort last, for LIOS directories). It applies this *within* a single mod
      too, collapsing each mod's candidates by filename before comparing across mods. The
      wiki states on_action files fire in *"ASCII-betical order"*. Community practice
      independently confirms `zzz_`/`zz_` prefixing as the standard technique for making
      a file win, and warns that unusual leading characters (e.g. `_`) can cause a file
      to sort earlier than intended and lose.
      **⚠ DESIGN CONTRADICTION — confirmed exactly as the question feared.** Precedence
      is `(load-order index, filename)` with a per-directory sort direction, not a single
      load-order index. `SourceId` ordering alone cannot resolve a conflict between two
      differently-named files. Note also that the comparison is **ordinal**, not
      culture-aware or case-insensitive, so `Z` sorts before `a`.
      Sources: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/InfoProviders/BaseDefinitionInfoProvider.cs,
      https://stellaris.paradoxwikis.com/Game_start_modding,
      https://forum.paradoxplaza.com/forum/threads/the-mod-load-order-may-not-be-working-as-intended-stellaris.1428245/

## Paths

- [ ] Windows user directory when OneDrive has redirected `Documents`. This affects a
      large share of Windows users and is a common source of "no mods found".
      **SINGLE SOURCE (Irony) — but the technique is the answer.** Irony does not
      construct the path from `%USERPROFILE%\Documents`; it asks the OS for the
      `MyDocuments` known folder and appends `Paradox Interactive`. That resolves
      OneDrive redirection automatically, because redirection is implemented by
      repointing the known folder. **The design's `{DOCUMENTS}` placeholder is therefore
      correct only if it is resolved via `SHGetKnownFolderPath`/`FOLDERID_Documents` (or
      the equivalent) rather than by string-joining the profile directory** — that
      distinction is the whole bug and should be stated in the ingest layer, not left to
      the profile.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/Registrations/UserDirectory.cs

- [ ] Linux native path: `~/.local/share/Paradox Interactive/Stellaris` vs
      `~/Documents/Paradox Interactive/Stellaris`.
      **SINGLE SOURCE (Irony) — the design's assumption holds.** Irony uses the
      `LocalApplicationData` folder (`$XDG_DATA_HOME`, defaulting to `~/.local/share`)
      plus `Paradox Interactive` on Linux. Not `~/Documents`.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/Registrations/UserDirectory.cs

- [ ] macOS path and whether the workshop directory differs in shape.
      **SINGLE SOURCE (Irony), and only half-answered.** Irony uses
      `~/Documents/Paradox Interactive` on macOS, matching the design. It has no
      macOS-specific workshop handling, so the workshop shape question is unaddressed —
      absence of special-casing is weak evidence that it is the same shape, not proof.

- [ ] Proton prefix path for a Windows install running on Linux.
      **SINGLE SOURCE (Irony).** `<proton-prefix>/drive_c/users/steamuser/Documents/Paradox Interactive`.
      Irony additionally rewrites Windows-style paths found *inside* descriptors
      (`path=`, `archive=`) back to host paths when running against a Proton install,
      which implies descriptors in a Proton prefix contain `C:\`-style paths that a
      naive reader will fail to resolve. Worth carrying into ingest.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/Resolver/PathResolver.cs

- [ ] Multi-library Steam setups: is the workshop always under the same library as the
      game, or can it differ?
      **NEEDS THE GAME.** Irony discovers libraries by parsing `libraryfolders.vdf` from
      both `config/` and `steamapps/` and searching them, which is what you would do if
      the answer were "it can differ" — but that is inference from defensive code, not a
      statement, and Irony would write the same code to handle a *moved* game. No source
      states the invariant. Probe with a deliberately split install.

## Descriptors

- [ ] Do `.mod` (launcher copy) and `descriptor.mod` (in-mod copy) ever disagree in
      practice, and which one does the game actually honour?
      **NEEDS THE GAME.** Irony parses both and models them as separate artefacts,
      carrying `DescriptorFile` distinctly from the mod's own directory contents, and
      writes both when exporting — which establishes that it considers them capable of
      disagreeing, but not which one the game honours. Nothing in any source states the
      resolution rule. This is a genuine gap and a plausible source of user-visible
      "the launcher shows X but the game does Y" bugs, so it is worth probing directly.

- [ ] Are `dependencies` matched by display name or by `remote_file_id`? Design assumes
      name, which is fuzzy; if ids are available the matching becomes exact and
      `dep.missing` gets much stronger.
      **SINGLE SOURCE (Irony) — design assumption holds, unfortunately.** Irony parses
      `dependencies` as a list of **strings** and, in its dependency-aware conflict
      suppression, compares each entry against the other mod's **name**. Where it reads
      the launcher's JSON it takes `displayName` for the same purpose. `remote_file_id`
      *is* parsed and available (as a long), but is not what dependencies are matched
      against. So `dep.missing` stays fuzzy and name-based; there is no exact-id path to
      upgrade to.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Parser/Mod/ModParser.cs

- [ ] Are zipped mods (`archive=`) still supported by the current launcher?
      **SINGLE SOURCE (Irony).** Yes. Irony reads the mod's payload location from
      `path` **or** `archive`, treating them as alternates for the same field, and has a
      live archive-reading path. Actively maintained in a 2026 codebase, so not
      vestigial.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Parser/Mod/ModParser.cs

- [ ] What identifies a mod stably across reinstalls when `remote_file_id` is absent
      (local mods)?
      **NEEDS THE GAME.** Irony falls back to a composite of descriptor path, name,
      version, dependencies and `remote_file_id` when deciding whether two mod records
      are the same, which is a same-session equality check rather than a stable
      cross-reinstall identity, and it is explicitly *not* offered as one. No source
      proposes a stable local-mod identifier. Expect to have to invent one (content hash
      of the descriptor plus directory name is the obvious candidate) and document it as
      ours rather than the game's.

## Version

- [ ] Where is the installed game version readable without launching? Candidates:
      `launcher-settings.json`, executable metadata, a `version.txt`.
      **SINGLE SOURCE (Irony).** `launcher-settings.json`, located in the game's
      **install** directory beside the executable. Irony reads the `rawVersion` field
      first and falls back to `version`, validating each parses as a version before
      accepting it — because, in its author's words, *"some games use version field
      (inconsistency again)"*. So a reader must try both keys. Irony also has a
      last-resort path that scrapes a `\d+\.\d+\.\d+(\.\d+)?` pattern out of text.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/GameService.cs

- [ ] Confirm `supported_version` wildcard forms in the wild (`3.13.*`, `4.*`, `v3.13.*`,
      bare `3.13`) so the pattern parser covers all of them.
      **NEEDS THE GAME (or a workshop corpus).** Irony parses `supported_version` with a
      fallback to `version` and has its own version type, but this pass did not establish
      which wildcard shapes it accepts, and no source enumerates the forms that occur in
      the wild. This one does not strictly need the game — it needs a scrape of a few
      thousand workshop descriptors, which is cheaper and can be done independently of
      the empirical pass. Recommend doing that rather than waiting.

## Syntax

- [x] Confirm the full operator set in current Stellaris script (`=`, `==`, `>`, `<`,
      `>=`, `<=`, `!=`, `?=`).
      **CORROBORATED — all eight, and exactly those eight.** CWTools' Clausewitz parser
      defines an operator enumeration of precisely `=`, `>`, `<`, `>=`, `<=`, `!=`, `==`,
      `?=` and nothing else. Irony's parser independently handles `?=`, the least
      obvious member of the set. The design's list is complete and correct.
      Sources: https://github.com/cwtools/cwtools/blob/master/CWTools/Parser/Types.fs,
      https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Parser.Common/Parsers/BaseParser.cs

- [ ] Confirm `@variable`, `@[expression]`, `$PARAM$`, and `[[PARAM] … ]` forms.
      **SINGLE SOURCE per form, and one form is probably wrong for Stellaris.**
      `@variable` is corroborated in the sense that both projects model
      `common/scripted_variables` as a first-class directory (Irony has a dedicated
      `ScriptedVariablesParser` and treats that path as its "global variables" location;
      CWTools' config models it as a type) — but neither source enumerates the *syntax*
      forms, so `@[expression]` inline maths is unconfirmed.
      `$PARAM$` is confirmed indirectly: Stellaris's parameter mechanism is
      **`common/inline_scripts`** with `inline_script` invocation, which Irony
      special-cases by name in its parser and exposes as `SupportsInlineScripts` for
      Stellaris specifically.
      **`[[PARAM] … ]` appears to be a CK3/Imperator form, not a Stellaris one.** No
      consulted source shows it in a Stellaris context, and Stellaris's equivalent need
      is served by `inline_scripts`. Carrying it in the Stellaris syntax spec risks a
      parser accepting something the game rejects. Verify before implementing.

- [ ] Sample real workshop mods for non-UTF-8 encodings and how common they are.
      **SINGLE SOURCE (Irony) for existence; frequency unaddressed.** Encoding variance
      is real enough that Irony carries per-definition encoding selection and an
      `IsValidEncoding` check, and it enforces a Stellaris-specific rule that
      `common/name_lists` **must** be UTF-8 **with BOM** while other paths must not be —
      so "UTF-8" is not one answer even within one game. No source quantifies how common
      non-UTF-8 files are. Like the wildcard item above, this wants a workshop corpus
      scrape rather than the game.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/InfoProviders/StellarisDefinitionInfoProvider.cs

- [ ] Are there directories where duplicate top-level keys are legal and *both* apply
      (i.e. `MergeMode::Additive`)? If so, list them — reporting those as conflicts
      would be a false positive.
      **SINGLE SOURCE (wiki) — see the additive item under Directory merge behaviour.**
      `common/on_actions` is the one identified case, and only across distinct
      filenames. Irony's one duplicate-id tolerance (`ethic_categories` under
      `common/ethics`) is a parser accommodation rather than evidence of additive game
      semantics, and should not be treated as a second entry in this list without a
      probe.

## Method

Fastest reliable route for most of these: build two minimal test mods that define the
same object with distinguishable values, load them in both orders, and observe in game.
Second: read Irony Mod Manager's and CWTools' source, both of which encode hard-won
answers to several of these questions.

**The second route has now been taken** — see `docs/GROUND-TRUTH-PRIOR-ART.md` for what
it yielded, which projects were readable under our licence policy, and the ranked list of
what the probe mods must still resolve. The first route is still required, and the probe
design should now be driven by that ranked list rather than by this file's original
ordering: the highest-value probes are no longer "which mod wins" (answered) but
`replace_path` scope, the FIOS/LIOS filename axis, and the `additive_children` set.
