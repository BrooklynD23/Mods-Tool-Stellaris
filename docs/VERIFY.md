# Assumptions to verify against a live install

This design was written without a Stellaris installation to hand. The items below are
the load-bearing factual claims. Each is encoded as **data** (in `profiles/stellaris.toml`)
rather than code specifically so that correcting one is a config edit, but a wrong
assumption here produces confidently wrong output, which is the worst failure mode this
tool has.

**This file is the exit criterion for M0.5 (§ `architecture/15`), which blocks M1.** Not
"before M1 ships" — before M1 starts. Answer each item **inline in this file**, in its
*Observed* record, and land the answer in `profiles/stellaris.toml` where it has a home
there. An answer recorded only in an issue or a commit message is an answer the next
person re-derives.

---

## How an item is recorded (H40)

Every item has a stable id (`V01`…), cited by the architecture documents and the profile,
and carries **two separate records**. They are never merged into one sentence, because
the August pass showed how easily "two projects believe X" reads as "X".

| Record | States | Stamp it must carry |
| --- | --- | --- |
| ***Prior art*** — what other implementations and documents believe | `CORROBORATED` (2+ independent sources agree) · `SINGLE SOURCE` · `NO SOURCE` (silent, or sources disagree) | The baseline id (below): which revision of which source, and which game version that source targets |
| ***Observed*** — what the running game, launcher or real data did | `NOT RUN` · `VERIFIED` (the prior-art reading held) · `CONTRADICTED` (it did not; the observed answer is stated) · `INCONCLUSIVE` (the run happened and did not discriminate; say why) | Game version **and checksum**, launcher version, OS, date, probe id or method, and the run-log line |

**A box is ticked only by an Observed record of `VERIFIED` or `CONTRADICTED` that
carries the full stamp.** Prior art never ticks a box, however strong. Items that need
real data rather than the game (a workshop corpus, the vanilla files) are stamped the same
way, with the corpus snapshot date in place of a probe id. An observation made on one
game version is an answer for that version. A later game version re-opens an item only
if its patch notes touch the item's mechanism. The refresh below is the procedure for
deciding that.

Prior art that agrees with an observation is corroboration of the observation. Prior art
alone is not an answer.

### Prior-art baselines

| Id | Sources and revisions read | Game version those sources target | Read on |
| --- | --- | --- | --- |
| `PA-2026-08` | Irony Mod Manager `master` (last commit before the pass), CWTools `master`, `cwtools-stellaris-config` `master`, tiger docs only, Stellaris wiki **via search summaries only**, Steam/Paradox forum via search summaries. Detail in `docs/GROUND-TRUTH-PRIOR-ART.md`. | ≤ 4.4.x. Irony's newest stable notes advertised *"Stellaris 4.2 CS support"*; 4.5 was in open beta. | 2026-08-06 |
| `PA-2026-10` | Irony tag `v1.28.97-alpha` (pre-release, 2026-09-22) for `StellarisDefinitionInfoProvider.cs` and `ModBaseService.cs`; Irony `master` `Paradox/v4` launcher models and `Constants.cs`; `Pseudorizer/paradox-launcher-playset-exporter` README. Read through fetch summaries with quoted lines, not a local clone. | Unstated. The release says only *"Latest Stellaris CS support"*. | 2026-10-17 |

---

## Stellaris 4.5 refresh (2026-10-17) — H41, H42

The August pass predates 4.5. Checked against the official Steam announcements feed for
app 281990 (`api.steampowered.com/ISteamNews/GetNewsForApp/v2/?appid=281990`), accessed
2026-10-17:

| Claim (from the October review) | Finding | Source |
| --- | --- | --- |
| Stellaris 4.5 "Cygnus" released Sept 2026 | **Confirmed.** *"Stellaris 4.5 "Cygnus" now available!"*, 2026-09-22. Saves from 4.4 and earlier do not load in 4.5 (Pop Group rework). | https://steamstore-a.akamaihd.net/news/externalpost/steam_community_announcements/1844115010503624 |
| …with hotfixes | **Confirmed.** 4.5.1 (checksum `358e`), 2026-09-24; 4.5.2 (checksum `9776`), 2026-10-06. **4.5.2 is the live version as of this refresh.** | …/1844751498221872, …/1845383656396316 |
| Irony advertises 4.2 support, not 4.5 | **Confirmed, with one qualification.** The stable 1.27.x series notes list *"Stellaris 4.2 CS support"*. No release names 4.3, 4.4 or 4.5. The newest build, `v1.28.97-alpha`, is a **pre-release** published the same day as 4.5. Its only game-support note is the unversioned *"Latest Stellaris CS support"*. Irony `master`'s last commit is 2026-07-29, before 4.5. | https://github.com/bcssov/IronyModManager/releases, …/releases/tag/v1.28.97-alpha, …/commits/master |
| Irony still models FIOS/LIOS and conflict inspection | **Confirmed.** The FIOS path list at `v1.28.97-alpha` is the same fourteen directories recorded in August (V02). | `StellarisDefinitionInfoProvider.cs` @ `v1.28.97-alpha` |
| `launcher-v2.sqlite` plausible; schema and snapshot behaviour unvalidated | **Confirmed as stated.** The file name is corroborated by two sources: Irony, and the Pseudorizer exporter README (a HOI4 tool, which names no tables). The table and column model is Irony's alone (V03, V41). Nothing anywhere addresses WAL or locking (V04, V05, V40). No 4.5.x patch note mentions the launcher. | https://github.com/Pseudorizer/paradox-launcher-playset-exporter |

**Modding-relevant 4.5.x changes, and the items they touch:**

- 4.5.1 and 4.5.2, Modding: *"Duplicate keys in a database now always log an error."*
  This bears on V38 (in-file duplicates) and V36. On ≥ 4.5.1 the probe's `error.log` is
  itself a readout.
- 4.5.2, Modding: fleet, starbase and defense-platform policies moved to
  `06_policies_fleets_and_starbases.txt`, and `defense_platform_policy_effects.txt` was
  renamed `fleet_and_starbase_policy_effects.txt`. **Vanilla renamed files in a hotfix.**
  A mod that whole-file-overrode the old name now replaces nothing, and its keys collide
  under the filename axis instead (V15, V02, V09). Same-path identity against vanilla is
  therefore a property of a game version, and the vanilla file list must come from the
  installed game, never from profile data. This one does not need a probe; it is a
  design constraint for the resolver.
- No 4.5.x note touches load order, playsets, `replace_path`, `localisation/replace`,
  `on_actions` or descriptor handling. **No August finding is re-opened by the patch
  notes.** All of them still stand on prior art that targets ≤ 4.4, so the empirical pass
  is run on **≥ 4.5.2** and stamps the checksum.

**Prior-art correction found by this refresh:** V09, "vanilla is always lowest", was
filed as CORROBORATED in August. Re-reading Irony at `v1.28.97-alpha` shows that Irony
does **not** believe this for differently named files outside `localisation/`. See V09.

---

## Status

| Record | Count |
| --- | --- |
| Observed `VERIFIED` / `CONTRADICTED` (box ticked) | **0 of 42** |
| Prior art `CORROBORATED` | 6 (V01, V15, V16, V17, V21, V33). V09 was demoted by the refresh. |
| Prior art `SINGLE SOURCE` | 26 |
| Prior art `NO SOURCE` | 10 |

42 items. V01–V36 are the August list. V37–V42 were added in October. Four come from
the resolver redesign's `unknown` parameters: V37 filename key, V38 in-file duplicates,
V39 localisation same-path, V42 case-insensitive identity. Two come from the launcher
snapshot redesign: V40 locking mode, V41 active-playset marker.

The shape of the prior-art column is itself a finding. Corroboration is scarce because
**there is really only one deep source of Stellaris override semantics in the world —
Irony Mod Manager.** CWTools is a validator and models a type system rather than merge
behaviour. The wiki is thin on precedence. tiger does not support Stellaris at all. Every
single-source item is a place where we are trusting one project's reading of an
undocumented engine. That is better than trusting our own guess, and much worse than
observation.

The probes are in `tools/probe-mods/` (P1–P7). Each run uses an **isolated playset
containing only that probe's mods** and records its canary readouts (H45).

---

## Load order semantics — **highest priority**

- [ ] **V01 — Does a mod later in the playset override an earlier one, or the reverse?**
      *Prior art — CORROBORATED (PA-2026-08): later wins, for the same filename.* Irony
      orders every candidate definition by `modOrder.IndexOf(modName)` ascending and then
      takes the **last** element as the winner when the competing files share a name,
      labelling the outcome `ModOrder`. Its SQLite exporter appends newly-added mods
      (including its own generated conflict-resolution patch mod, which must beat
      everything) at the **highest** `position` values. CWTools sorts vanilla to the end
      of its precedence list so that mods always come out on top. The community
      consensus, including the most-cited Steam load-order guide, is "mods at the top of
      the list are loaded first, mods lower down will overwrite". Three independent
      sources agree. **⚠ But see V02: "later wins" is not the whole rule.**
      Sources: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/ModBaseService.cs,
      https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/Exporter/SQLiteExporter.cs,
      https://github.com/cwtools/cwtools/blob/master/CWTools/Game/ResourceManager.fs,
      https://steamcommunity.com/sharedfiles/filedetails/?id=2290164696
      *Observed — NOT RUN.* Probe P1.

- [ ] **V02 — ⚠ DESIGN CONTRADICTION — load order is not the only precedence axis, and
      for some directories it is not even the deciding one. Which directories are FIOS?**
      *Prior art — SINGLE SOURCE (Irony; PA-2026-08, list re-read unchanged at PA-2026-10).*
      Irony resolves a conflict in two stages. First, among candidates sharing the *same*
      filename, the later mod in load order wins (V01). Second, among candidates in
      *differently named* files, the winner is chosen by **ordinal filename sort**, and the
      direction depends on the directory: most directories are **LIOS** (last name
      alphabetically wins), but a named set of Stellaris directories are **FIOS** (first
      name alphabetically wins). Irony names the concepts explicitly
      (`DefinitionUsesFIOSRules`, `PriorityType.FIOS`, `PriorityType.LIOS`) and generates
      winning filenames by prefixing `!!!_` for FIOS and `zzz_` for LIOS. The Stellaris FIOS
      list it ships is:
      `common/component_sets`, `common/component_templates`, `common/event_chains`,
      `common/global_ship_designs`, `common/scripted_variables`, `common/section_templates`,
      `common/ship_behaviors`, `common/special_projects`, `common/strategic_resources`,
      `events`, `common/solar_system_initializers`, `common/traits`,
      `common/start_screen_messages`, `common/governments/authorities`.
      **This means a mod placed *later* in the playset can still lose a `common/traits`
      conflict if its filename sorts later.** Only Irony encodes the FIOS path list. The
      community corroborates that filename order matters (V21) but not which directories
      invert. The design now carries the axis in the precedence tuple (`REVIEW-2026-10`
      H2). The list did not change between the August read and `v1.28.97-alpha`. That
      means Irony has not *changed its belief* for 4.5. It does not mean the belief was
      re-checked against 4.5.
      Sources: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/InfoProviders/StellarisDefinitionInfoProvider.cs,
      https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/InfoProviders/BaseDefinitionInfoProvider.cs,
      https://github.com/bcssov/IronyModManager/blob/v1.28.97-alpha/src/IronyModManager.IO/Mods/InfoProviders/StellarisDefinitionInfoProvider.cs
      *Observed — NOT RUN.* Probe P3 (`common/traits`). P2 covers localisation. The other
      thirteen directories need spot-checks after P3 (`GROUND-TRUTH-PRIOR-ART.md` § 6 #2).

- [ ] **V03 — Where does the current launcher store the active playset and its ordering,
      and under which table and column names?**
      *Prior art — file name CORROBORATED; schema SINGLE SOURCE (Irony; PA-2026-08,
      models re-read at PA-2026-10).* The file is `launcher-v2.sqlite` in the game's user
      directory, with `launcher-v2_openbeta.sqlite` for the open beta branch. Irony names
      it, and so does the Pseudorizer exporter's README for HOI4, which names no tables.
      Irony's newest (`v4`) launcher model:
      - `playsets(id PK, name, isActive, isRemoved, loadOrder, createdOn)`. The older `v2`
        model has no `isRemoved` or `createdOn`.
      - `playsets_mods(playsetId, modId, position, enabled)`.
      - `mods(id PK, name, displayName, dirPath, archivePath, steamId, pdxId,
        gameRegistryId, status, source, version, requiredVersion, tags, timeUpdated, …)`.
        `tags` is a JSON string.

      Irony reads every boolean through a tolerant object-to-bool handler, which suggests
      the stored type is not reliably integer `0/1`. **`position` changed type across
      launcher versions**: older schemas stored it as a zero-padded 10-character
      hexadecimal *string*, newer ones as an integer. A reader that assumes integer will
      mis-sort an old DB, and one that assumes string will mis-sort a new one. Ascending
      `position` is ascending load order. **No source describes the launcher that ships
      with 4.5.x.** Irony `master` predates 4.5, and no 4.5.x patch note mentions the
      launcher.
      Sources: https://github.com/bcssov/IronyModManager/tree/master/src/IronyModManager.IO/Mods/Models/Paradox/v4,
      https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/Constants.cs,
      https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/Exporter/SQLiteExporter.cs,
      https://github.com/Pseudorizer/paradox-launcher-playset-exporter
      *Observed — NOT RUN.* Method: on a closed launcher, run `sqlite3 -readonly
      launcher-v2.sqlite .schema` and `SELECT name FROM knex_migrations`, and check both
      output files in under `bench/launcher-schema/<launcher-version>/`. The reader's
      schema probe (§ `architecture/05`) checks exactly the columns this item confirms.

- [ ] **V04 — Does the launcher DB run in WAL mode, and is a reorder made in the open
      launcher visible only after `-wal` is replayed?**
      *Prior art — NO SOURCE.* No consulted source addresses journal mode. Irony connects
      to the DB through an ORM without setting or inspecting `journal_mode`, and neither
      CWTools nor tiger reads the launcher DB at all.
      *Observed — NOT RUN.* Method: record header bytes 18 and 19 of the live file (both
      `2` means WAL) and `PRAGMA journal_mode` on a copy. Then reorder a playset in the open
      launcher and test **the snapshot protocol actually specified in § `architecture/05`**
      (online backup with read-back verification, and the stable-file-copy fallback). If a
      naive main-file-only copy ever returns the *new* order, say why. Do not conclude the
      protocol is unnecessary. Also record whether `-wal` and `-shm` **exist** beside the
      DB while the launcher is open, and again after it is closed. The online path is taken
      only when both already exist, so the reader never creates files beside the user's
      DB.

- [ ] **V05 — Does the launcher hold a lock or a share mode that prevents even a file copy
      on Windows?**
      *Prior art — NO SOURCE.* Irony writes to the launcher DB directly rather than copying
      it, and its documented failure mode is "close the launcher first". That is consistent
      with a lock but does not establish one.
      *Observed — NOT RUN.* Method: copy `launcher-v2.sqlite` and `-wal` with the launcher
      open on Windows, and record the error code if the copy is refused. Repeat from WSL
      through `/mnt/c` (drvfs) against the same Windows DB. § `architecture/05` forbids the
      online path across drvfs/9p/SMB/Proton-from-host and uses the double-copy fallback
      there, so this run checks that the fallback actually works on drvfs.

- [ ] **V06 — What schema version marker can the reader check to detect a launcher
      update?**
      *Prior art — SINGLE SOURCE (Irony; PA-2026-08, re-read PA-2026-10).* It is **not**
      `user_version`. The launcher is a Knex-based JavaScript application and carries a
      **`knex_migrations` table of named migrations**. Irony detects schema generation by
      testing for specific migration names, three of which it hardcodes:
      `addCreatedAndSubscribedDate`, `modifyPositionToInteger` (the hex-string→integer
      change in V03), and `addModShortDescription`. So the marker is a *set of names*, not
      a monotonic integer, and a reader must test for the migrations it cares about rather
      than compare a version number. Irony's constants name the migrations but not Knex;
      the Knex attribution comes from its `KnoxMigrations` model.
      Sources: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/Constants.cs,
      https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/Models/Paradox/v2/KnoxMigrations.cs
      *Observed — NOT RUN.* Same dump as V03. Record the **full** migration-name list, not
      just the three Irony knows.

- [ ] **V07 — Is the launcher's displayed order the load order, or is it re-sorted (by
      name, by dependency) before being handed to the game?**
      *Prior art — SINGLE SOURCE (Irony; PA-2026-08).* Irony treats ascending `position` as
      the load order handed to the game, and rewrites `position` contiguously from zero
      when it exports a collection. It applies no name or dependency re-sort of its own.
      That is consistent with the displayed order being the load order. But Irony *writes*
      the order rather than observing what the game consumes, so it cannot rule out a
      re-sort inside the game.
      *Observed — NOT RUN.* P1 answers it incidentally: run 1B is the displayed order
      reversed. Also compare the playset's `position` order with `dlc_load.json` after
      pressing Play (V08).

- [ ] **V08 — Does the game read `dlc_load.json` (written by the launcher at Play), and
      what happens when it disagrees with the launcher DB?**
      *Prior art — SINGLE SOURCE (Irony; PA-2026-08).* Still live, and still written. Irony
      maintains both a JSON exporter and a SQLite exporter. It writes `dlc_load.json`
      alongside `launcher-v2.sqlite`, together with `game_data.json`, `content_load.json`,
      `mods_registry.json` and `playsets.json`. That it is still maintained is strong
      evidence the file is not vestigial. But no source states which of the two the *game*
      reads when they disagree, and that disagreement is exactly the case this tool must
      diagnose. The ingest model (§ `architecture/05`) treats the DB as the **next** launch
      and `dlc_load.json` as the **last** launch, and reports a disagreement as
      `ingest.activation_disagreement`. This item is the evidence that model needs.
      *Observed — NOT RUN.* Method: press Play with playset X, quit, switch the launcher to
      playset Y without pressing Play, then launch the game executable directly. Record
      which playset loaded.

- [ ] **V09 — Where does vanilla sit in the filename-precedence chain relative to mods?
      (Design assumes `SourceId(0)`, i.e. always lowest.)**
      *Prior art — same path: CORROBORATED (PA-2026-08). Differently named files: SINGLE
      SOURCE (PA-2026-10), and that source says vanilla is **not** always lowest.*
      The August pass ticked this as corroborated. The October re-read shows that the
      corroboration covers only the same-path case:
      - **CWTools** tags vanilla `embedded` and sorts it to the end, so a mod entity takes
        `Overwrote` and vanilla `Overwritten`. But its unit of override is `logicalpath`,
        the root-relative path, so it speaks only to the same-path case.
      - **Irony**, in `EvalDefinitionPriorityInternal`'s non-localisation branch, removes a
        vanilla definition only when it **shares a filename** with a mod's definition. That
        grouping is case-*insensitive* (`FileNameCI`), while the sort itself is ordinal.
        Vanilla definitions in uniquely named files **stay in the candidate set and take
        part in the ordinal filename sort**. Vanilla is pushed last only in the no-winner
        fallback. In the localisation branch vanilla *is* excluded from the ordering.

      So Irony's reading is: vanilla loses same-path and localisation collisions, but in a
      FIOS directory `00_species_traits.txt` beats a mod's `zzz_traits.txt`, and in a LIOS
      directory `00_buildings.txt` beats a mod's `!!!_buildings.txt`. That is consistent
      with Irony generating `!!!_` names for FIOS patch files. **The design's `SourceId(0)`
      assumption holds for same-path collisions only.** The profile's
      `vanilla_is_lowest = true` is overclaimed and is carried as
      `vanilla_in_filename_axis = "unknown"` (§ `architecture/07`, `10`). This is the
      review's "vanilla is always lowest is unestablished for differently named files",
      confirmed (H44).
      Sources: https://github.com/cwtools/cwtools/blob/master/CWTools/Game/ResourceManager.fs,
      https://github.com/bcssov/IronyModManager/blob/v1.28.97-alpha/src/IronyModManager.Services/ModBaseService.cs
      (read through a fetch summary with quoted lines; re-read the file before relying on
      the case-insensitive grouping detail).
      *Observed — NOT RUN.* Probe P4. It runs both directions: a `zzz_` file in FIOS
      `common/traits`, a `!!!_` file in LIOS `common/buildings`, and `!!!_`/`zzz_` files in
      localisation.

- [ ] **V37 — Is the filename-axis comparison over the basename or the
      directory-relative path?** This matters for files in subdirectories, e.g.
      `common/traits/sub/x.txt`.
      *Prior art — SINGLE SOURCE (Irony; PA-2026-10).* Basename **without extension**.
      Irony compares `Path.GetFileNameWithoutExtension(...)` ordinally, which discards the
      directory part. Its localisation branch passes the full `File` through the same
      function, so localisation is also effectively basename. Whether the game reads
      subdirectories of every `common/` folder at all is part of the question.
      Source: https://github.com/bcssov/IronyModManager/blob/v1.28.97-alpha/src/IronyModManager.Services/ModBaseService.cs
      *Observed — NOT RUN.* Probe P5.

- [ ] **V38 — Within one file, when the same top-level key is defined twice, does the
      first or the last definition win?**
      *Prior art — NO SOURCE* for which one wins. New on ≥ 4.5.1: the patch notes say
      *"Duplicate keys in a database now always log an error"*, so the engine detects the
      case and `error.log` should name it. That says nothing about which definition is
      kept.
      Source: https://steamstore-a.akamaihd.net/news/externalpost/steam_community_announcements/1844751498221872
      *Observed — NOT RUN.* Probe P6. Also record the `error.log` line verbatim.

- [ ] **V42 — Is same-path identity case-insensitive in the game?** The design normalises
      paths to lowercase for identity (§ `architecture/03`). On Linux and macOS,
      `00_Traits.txt` and `00_traits.txt` are distinct files on disk.
      *Prior art — SINGLE SOURCE (Irony; PA-2026-10).* Irony groups vanilla-versus-mod
      filename collisions case-insensitively (`FileNameCI`), while its filename *sort* is
      ordinal and case-sensitive. No source says what the game does on a case-sensitive
      filesystem.
      *Observed — NOT RUN.* No probe shipped yet (H47). Shape: on Linux, two mods ship
      `common/edicts/SMD_probe_case.txt` and `common/edicts/smd_probe_case.txt`, and the later one
      omits an edict the earlier one defines. Whole-file loss means identity is
      case-insensitive. Run it after the V15 probe, which it depends on.

## Launcher locking (inputs to the snapshot protocol, § `architecture/05`) — H48

- [ ] **V40 — Does the launcher use stock SQLite locking: no `locking_mode=EXCLUSIVE`, no
      custom VFS?** An EXCLUSIVE launcher makes the online-backup path return
      `SQLITE_BUSY` on every attempt and forces the copy fallback.
      *Prior art — NO SOURCE.* Irony opens the DB through an ORM and documents only "close
      the launcher first".
      *Observed — NOT RUN.* Method: with the launcher open and idle, open the live DB
      `?mode=ro` and run one read transaction. Record success, or the error code.

- [ ] **V41 — In the current schema, what marks the active playset, and what does
      `playsets_mods.enabled` mean?**
      *Prior art — SINGLE SOURCE (Irony; PA-2026-10).* `playsets.isActive` (boolean) marks
      the active playset. `playsets.isRemoved` exists in the newest model and suggests that
      soft-deleted playsets remain in the table. `playsets_mods.enabled` is a per-row
      boolean. No source states that exactly one row has `isActive` set, or whether a
      disabled row (`enabled = 0`) is omitted from `dlc_load.json`.
      *Observed — NOT RUN.* Method: from the V03 dump, count the `isActive` rows. Disable one
      mod in a playset, press Play, and diff `dlc_load.json`. Delete a playset and check
      whether its row disappears or gains `isRemoved`.

## `replace_path`

- [ ] **V10 — Exact semantics: does it drop the vanilla directory only, or also that
      directory's contents from *all* mods loading earlier?**
      *Prior art — SINGLE SOURCE (Irony), and Irony itself flags it as a guess (PA-2026-08).*
      Irony's merge service excludes a mod's files under a replaced path when that mod sits
      *earlier* in the collection than the last mod declaring the same `replace_path`. That
      is, it implements "drops vanilla **and** every earlier mod's contents at that path".
      The code comment states the uncertainty in the author's own words: *"Believe that
      handling of replace_path in the game is so any mod before does not copy its output
      *only* the ones following with replace path will be copied."* No second source
      addresses it. **This is the single highest-value item for the empirical pass**: it
      is the difference between `replace_path` being a narrow vanilla-only exclusion and
      it being a load-order-wide guillotine, and the tool's blast-radius reporting
      changes completely between the two.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/ModMergeService.cs
      *Observed — NOT RUN.* No probe shipped yet (H47). Design in
      `GROUND-TRUTH-PRIOR-ART.md` § 6 #1.

- [ ] **V11 — Is it relative to the game root, and does it match recursively?**
      *Prior art — SINGLE SOURCE (Irony; PA-2026-08).* Relative to the game root, and
      matched as a case-insensitive **path prefix** against each file's root-relative path.
      That makes it recursive by construction, and also means `replace_path = "common/tra"`
      would match `common/traits/`. Whether the game is equally naive about partial segment
      matches is untested. The EU4 wiki's `replace_path = "missions"` example is consistent
      with root-relative whole-directory semantics but does not speak to recursion.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/ModMergeService.cs
      *Observed — NOT RUN.* Add-on to the V10 probe (H47).

- [ ] **V12 — Can a mod declare several? (Design assumes yes and accumulates.)**
      *Prior art — SINGLE SOURCE (Irony; PA-2026-08); the design assumption holds.* Irony
      parses `replace_path` with its repeated-key accessor (`GetKeyedValues`) rather than
      its single-value accessor, and models it as a list on the mod object. So multiple
      declarations in one descriptor accumulate. `user_dir` is handled the same way. No
      second source addresses multiplicity.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Parser/Mod/ModParser.cs
      *Observed — NOT RUN.* Add-on to the V10 probe (H47).

- [ ] **V13 — What happens when two mods declare `replace_path` for the same directory?**
      *Prior art — NO SOURCE (PA-2026-08).* Only Irony speaks to it, and only as a
      consequence of the guess in V10: it takes the **last** mod declaring that path as the
      cutoff, so every mod before it is excluded and every mod after it contributes
      normally. Nothing corroborates this, and it follows from a belief the author
      explicitly hedged. Treat as unknown.
      *Observed — NOT RUN.* Add-on to the V10 probe (H47).

## Directory merge behaviour

**This section is the single source of truth for merge modes.** Where another document
appears to assert one, this file wins and that document is out of date. Every entry marked
`merge = "unknown"` in `profiles/stellaris.toml` corresponds to an item below. **No merge
mode in the profile is observed.** Every value it ships rests on prior art, and the
profile comments say which kind.

- [ ] **V14 — For each `[[dir]]` entry in `profiles/stellaris.toml`, confirm the `merge`
      mode.** The critical distinction: directories where a same-named file **replaces**
      the whole file, versus directories where entries **merge by key** across files.
      *Prior art — SINGLE SOURCE (Irony; PA-2026-08) for the per-directory map; the general
      rule is corroborated separately (V15).* The framing of this question turns out to be
      a false dichotomy (V15). Irony assigns a parser per directory, and that assignment is
      a de-facto merge-mode table:
      - most of `common/` gets a key parser (one definition per top-level key ⇒ key merge);
      - `common/defines` and `localisation/` get per-key variable parsers;
      - `common/on_actions` gets a whole-text parser (the file is the unit);
      - two Stellaris-specific parsers mark object-level and single-file-per-directory
        behaviour.

      Those assignments are transcribed into `profiles/stellaris.toml`. Directories Irony
      does not special-case are `merge = "unknown"` in the profile.
      Source: https://github.com/bcssov/IronyModManager/tree/master/src/IronyModManager.Parser
      *Observed — NOT RUN.* P3 covers `common/traits` and P4 covers `common/buildings`.
      Every other directory needs its own observation or stays single-source.

- [ ] **V15 — Confirm the widely-repeated claim that same-filename replacement is the rule
      for most of `common/`, and identify the exceptions.**
      *Prior art — CORROBORATED (PA-2026-08), but the rule is conditional on filename
      identity, and that is not what the design modelled.* All three sources agree on a
      two-part rule:
      (a) across files with **different** names, entries merge and the last-resolved
      definition of a key wins;
      (b) across files with the **same** root-relative name, the winning file replaces the
      earlier ones **wholesale**, so keys present only in an earlier copy are lost.

      Irony implements (b) as an explicit gate: for each definition it finds the
      highest-load-order mod shipping that same filename and discards the definition if
      that mod's copy of the file does not contain it (`existsInLastFile`). CWTools groups
      entities by `logicalpath`, the root-relative path, and marks all but the winner
      `Overwritten`, i.e. file identity is its unit of override. The community statement of
      the same rule: *"If the keys are the same, the keys are usually just simply
      overridden by the mod that is loaded last. However, when the file names are exactly
      the same, the paradox launcher will attempt a full file override, rather than just
      overriding individual keys."* The sources speak to `common/` and binary assets.
      `localisation/` is a separate question (V39).
      **⚠ DESIGN CONTRADICTION (August):** `MergeMode` modelled `file_replace` and
      `merge_by_key` as mutually exclusive properties *of a directory*. They are two
      behaviours of the same directory, selected per pair of files by whether the names
      match. Addressed in design by `REVIEW-2026-10` H1 (§ `architecture/07`). **4.5.2 note:**
      vanilla renamed policy files in a hotfix (refresh above), so whether a mod file is
      same-path with vanilla depends on the game version.
      Sources: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/ModPatchCollectionService.cs,
      https://github.com/cwtools/cwtools/blob/master/CWTools/Game/ResourceManager.fs,
      https://steamcommunity.com/app/281990/discussions/0/4427688685166331427/
      *Observed — NOT RUN.* No dedicated probe shipped yet (H47; `GROUND-TRUTH-PRIOR-ART.md`
      § 6 #4). P1 shows same-path *precedence*, not whole-file *loss*.

- [ ] **V16 — `common/defines/` specifically: does a mod's defines file replace the whole
      vanilla set, or merge key by key?** Two mods each shipping a one-key defines file is
      the test. `pdx.defines_overlap` carries a base weight of 75, and this answer decides
      whether that is right or 75 points of noise.
      *Prior art — CORROBORATED (PA-2026-08): it merges key by key.* Irony parses
      `common/defines` and `unchecked_defines` with a dedicated parser that emits **one
      definition per individual define key**, typed `SpecialVariable`, with the namespace
      (`NGameplay`, `NShip`, …) as the definition type and the leaf key as the id. That
      includes flattening dot-notation and the Lua-style nested form. Conflicts are
      therefore reported at individual-define granularity, not file granularity. The
      community rule quoted under V15 says the same for keys generally. CWTools' Stellaris
      config independently models each defines namespace as its own type rather than
      treating the file as one object.
      **⚠ Consequence for scoring:** the rule must fire on colliding *keys*, or on the
      filename-identity case (two mods both shipping `00_defines.txt`), not on overlapping
      files.
      Sources: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Parser/Generic/DefinesParser.cs,
      https://github.com/cwtools/cwtools-stellaris-config/blob/master/config/common/defines.cwt,
      https://steamcommunity.com/app/281990/discussions/0/4427688685166331427/
      *Observed — NOT RUN.* No probe shipped yet.

- [ ] **V17 — `common/traits/` specifically: same question.** It is the design's canonical
      example and has been written both ways.
      *Prior art — CORROBORATED (PA-2026-08): it merges per trait object, not per file.*
      Irony routes `common/traits` to its Stellaris `OverwrittenParser`, which emits one
      definition per top-level trait object and tags it `OverwrittenObject`. That means a
      later definition of `trait_intelligent` replaces that trait *object* whole (its child
      keys are not merged), while other traits in the same file are untouched. CWTools'
      Stellaris config independently models `type[trait]` with
      `path = "game/common/traits"` and a per-object key, i.e. the trait id is the unit of
      identity, not the file. Two independent projects agree.
      **⚠ Two caveats.** First, the *body* of a trait does not merge: a mod redefining
      `trait_intelligent` to change one modifier silently drops every other key in the
      vanilla trait. Second, `common/traits` is **FIOS** (V02).
      Sources: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Parser/Games/Stellaris/OverwrittenParser.cs,
      https://github.com/cwtools/cwtools-stellaris-config/blob/master/config/common/traits.cwt
      *Observed — NOT RUN.* P3 shows per-object identity. Body non-merge needs P4's trait
      override with one key removed (add-on).

- [ ] **V18 — Additive-body directories. In `common/on_actions/`, when two mods both
      declare `on_game_start`, do the `events`/`effect` lists accumulate, or does the later
      declaration replace the earlier one wholesale? Which child keys accumulate and which
      do not?** The answer becomes `additive_children` in the profile. Nearly every content
      mod hooks `on_game_start`, so getting this wrong produces one ~200-participant High
      finding that is entirely false.
      *Prior art — SINGLE SOURCE (wiki, via search summary; PA-2026-08) for the mechanism;
      the "not a conflict" conclusion is corroborated.* The wiki states that mods *"can
      provide their own, uniquely named, on_action file whose events are called in addition
      [to] the vanilla events"*. Where an action is registered in several files, the events
      fire in *"ASCII-betical order"* by filename, i.e. **`events` lists accumulate across
      distinct files**. Irony reaches the same practical outcome by a different mechanism:
      it parses `common/on_actions` with its **whole-text** parser keyed on the
      **filename**, so differently named files produce no conflict at all. Only `events` is
      named as accumulating. `effect` and `random_events` have no source, and the proposed
      `additive_children = ["events", "effect", "random_events"]` must not ship as an
      assertion. **Two mods both shipping `00_on_actions.txt` hit V15** and lose whole
      files, including vanilla's.
      Sources: https://stellaris.paradoxwikis.com/Game_start_modding,
      https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Parser/Generic/WholeTextParser.cs
      *Observed — NOT RUN.* No probe shipped yet (H47; `GROUND-TRUTH-PRIOR-ART.md` § 6 #3).
      A positive observation still does not unlock `merge_by_key_additive_body` until
      child-level hashing lands (§ `architecture/07`).

- [ ] **V19 — Are any directories genuinely `Additive`**: duplicates coexist and *both*
      apply, with nothing overriding anything? If so, list them. Reporting those as
      conflicts is a false positive by construction.
      *Prior art — SINGLE SOURCE (wiki; PA-2026-08), one directory only.* `common/on_actions`
      behaves additively across distinct filenames (V18). No source identifies any other
      genuinely additive directory in Stellaris. Irony's nearest equivalent is a narrow
      duplicate-id allow-list (`ethic_categories` under `common/ethics`), which is a
      tolerance, not additive semantics. Treat the additive set as "on_actions, and possibly
      nothing else" until probed.
      *Observed — NOT RUN.*

- [ ] **V20 — Does `localisation/replace/` take precedence over `localisation/` as
      assumed? Does it work per key, per file, or per directory?**
      *Prior art — SINGLE SOURCE (Irony; PA-2026-08): the assumption holds, and it is per
      key.* Irony special-cases localisation ahead of all its other precedence logic: for a
      given loc key, if *any* candidate lives under a `replace` directory, it discards every
      candidate that does not, then resolves among the survivors. So `localisation/replace/`
      wins over `localisation/` **regardless of load order**. The unit is the individual
      YAML key. Irony also carries a numeric `CustomPriorityOrder` applied before mod order
      within localisation, which suggests further structure this pass did not resolve.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/ModBaseService.cs
      *Observed — NOT RUN.* No probe shipped yet.

- [ ] **V39 — In `localisation/`, do two files at the same path replace each other
      wholesale (V15), or merge per key?**
      *Prior art — NO SOURCE.* V15's whole-file rule is corroborated for `common/` and
      assets. Irony keys localisation per YAML key, and its localisation branch orders by
      file but never states that a same-path loser's other keys are dropped.
      *Observed — NOT RUN.* Probe P7.

- [ ] **V21 — Intra-source ordering. Within one mod, does the `00_`/`zz_` filename
      convention actually affect load order, or is it convention only?**
      *Prior art — CORROBORATED (PA-2026-08): it is real, and it is a second precedence
      axis.* Irony sorts candidate filenames ordinally and picks the first or last according
      to the directory's FIOS/LIOS class. It applies this *within* a single mod too,
      collapsing each mod's candidates by filename before comparing across mods. The wiki
      states that on_action files fire in *"ASCII-betical order"*. Community practice
      independently confirms `zzz_`/`zz_` prefixing as the standard technique for making a
      file win, and warns that unusual leading characters (e.g. `_`) can make a file sort
      earlier than intended and lose. The comparison is **ordinal**, so `Z` sorts before
      `a`.
      Sources: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/InfoProviders/BaseDefinitionInfoProvider.cs,
      https://stellaris.paradoxwikis.com/Game_start_modding,
      https://forum.paradoxplaza.com/forum/threads/the-mod-load-order-may-not-be-working-as-intended-stellaris.1428245/
      *Observed — NOT RUN.* P2 and P3 observe it across mods. A within-one-mod variant is
      an add-on.

## Paths

- [ ] **V22 — Windows user directory when OneDrive has redirected `Documents`.**
      *Prior art — SINGLE SOURCE (Irony; PA-2026-08), but the technique is the answer.*
      Irony asks the OS for the `MyDocuments` known folder rather than joining
      `%USERPROFILE%\Documents`. That resolves OneDrive redirection automatically. The
      `{DOCUMENTS}` placeholder is correct only if it is resolved via
      `SHGetKnownFolderPath`/`FOLDERID_Documents`.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/Registrations/UserDirectory.cs
      *Observed — NOT RUN.* The probe README records which mod directory worked.

- [ ] **V23 — Linux native path: `~/.local/share/Paradox Interactive/Stellaris` vs
      `~/Documents/Paradox Interactive/Stellaris`.**
      *Prior art — SINGLE SOURCE (Irony; PA-2026-08); the design's assumption holds.*
      `LocalApplicationData` (`$XDG_DATA_HOME`, defaulting to `~/.local/share`) plus
      `Paradox Interactive`.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/Registrations/UserDirectory.cs
      *Observed — NOT RUN.*

- [ ] **V24 — macOS path, and whether the workshop directory differs in shape.**
      *Prior art — SINGLE SOURCE (Irony; PA-2026-08), and only half-answered.*
      `~/Documents/Paradox Interactive`. The workshop shape is unaddressed. 4.5 ships native
      Apple Silicon builds (release post), which is a reason to check this on 4.5 rather
      than assume the 4.4 layout.
      *Observed — NOT RUN.*

- [ ] **V25 — Proton prefix path for a Windows install running on Linux.**
      *Prior art — SINGLE SOURCE (Irony; PA-2026-08).*
      `<proton-prefix>/drive_c/users/steamuser/Documents/Paradox Interactive`. Irony also
      rewrites Windows-style `path=`/`archive=` values in descriptors back to host paths.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/Resolver/PathResolver.cs
      *Observed — NOT RUN.*

- [ ] **V26 — Multi-library Steam setups: is the workshop always under the same library as
      the game?**
      *Prior art — NO SOURCE (PA-2026-08).* Irony parses `libraryfolders.vdf` from both
      `config/` and `steamapps/`. That is inference from defensive code, not a statement.
      *Observed — NOT RUN.* Probe with a deliberately split install.

## Descriptors

- [ ] **V27 — Do `.mod` (launcher copy) and `descriptor.mod` (in-mod copy) ever disagree in
      practice, and which one does the game honour?**
      *Prior art — NO SOURCE (PA-2026-08).* Irony parses and writes both, which shows it
      considers them capable of disagreeing, but not which one wins.
      *Observed — NOT RUN.*

- [ ] **V28 — Are `dependencies` matched by display name or by `remote_file_id`?**
      *Prior art — SINGLE SOURCE (Irony; PA-2026-08): by name.* `remote_file_id` is parsed
      but is not what dependencies reference.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Parser/Mod/ModParser.cs
      *Observed — NOT RUN.*

- [ ] **V29 — Are zipped mods (`archive=`) still supported by the current launcher?**
      *Prior art — SINGLE SOURCE (Irony; PA-2026-08): yes.* `path` or `archive`, as
      alternates. Irony's v4 `mods` model has both `dirPath` and `archivePath` (V03).
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Parser/Mod/ModParser.cs
      *Observed — NOT RUN.*

- [ ] **V30 — What identifies a mod stably across reinstalls when `remote_file_id` is
      absent (local mods)?**
      *Prior art — NO SOURCE (PA-2026-08).* Irony's composite equality check is
      same-session only. Expect to define our own identity and document it as ours.
      *Observed — NOT RUN.*

## Version

- [ ] **V31 — Where is the installed game version readable without launching?**
      *Prior art — SINGLE SOURCE (Irony; PA-2026-08).* `launcher-settings.json` in the
      install directory: `rawVersion` first, then `version`, because *"some games use
      version field (inconsistency again)"*.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Services/GameService.cs
      *Observed — NOT RUN.* Every probe run reads it anyway for its stamp. Record both
      fields, and whether the checksum appears anywhere on disk.

- [ ] **V32 — Confirm `supported_version` wildcard forms in the wild (`3.13.*`, `4.*`,
      `v3.13.*`, bare `3.13`).**
      *Prior art — NO SOURCE (PA-2026-08).* Needs a scrape of a few thousand workshop
      descriptors, not the game.
      *Observed — NOT RUN.* Stamp with the corpus snapshot date.

## Syntax

- [ ] **V33 — Confirm the full operator set (`=`, `==`, `>`, `<`, `>=`, `<=`, `!=`, `?=`).**
      *Prior art — CORROBORATED (PA-2026-08): all eight, and exactly those eight.* CWTools'
      operator enumeration, plus Irony's independent handling of `?=`.
      Sources: https://github.com/cwtools/cwtools/blob/master/CWTools/Parser/Types.fs,
      https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.Parser.Common/Parsers/BaseParser.cs
      *Observed — NOT RUN.* Method: lex the installed 4.5.2 vanilla tree and report every
      operator token. Stamp with the game version.

- [ ] **V34 — Confirm `@variable`, `@[expression]`, `$PARAM$`, and `[[PARAM] … ]` forms.**
      *Prior art — SINGLE SOURCE per form (PA-2026-08), and one form is probably wrong for
      Stellaris.* `@variable` is modelled by both projects. `@[expression]` is unconfirmed.
      `$PARAM$` is confirmed indirectly via `common/inline_scripts`. `[[PARAM] … ]` appears
      to be a CK3/Imperator form. Irony's July 2026 commits widened inline-script detection
      considerably. That is a sign that inline-script syntax in real mods is broader than
      the documented form.
      *Observed — NOT RUN.* Method: lex the vanilla tree, then the workshop corpus.

- [ ] **V35 — Sample real workshop mods for non-UTF-8 encodings and how common they are.**
      *Prior art — SINGLE SOURCE (Irony; PA-2026-08) for existence; frequency unaddressed.*
      `common/name_lists` must be UTF-8 **with BOM** and other paths must not be.
      Source: https://github.com/bcssov/IronyModManager/blob/master/src/IronyModManager.IO/Mods/InfoProviders/StellarisDefinitionInfoProvider.cs
      *Observed — NOT RUN.* Same corpus as V32.

- [ ] **V36 — Are there directories where duplicate top-level keys are legal and *both*
      apply (`MergeMode::Additive`)?**
      *Prior art — SINGLE SOURCE (wiki; PA-2026-08).* See V19. On ≥ 4.5.1, *"duplicate keys
      in a database now always log an error"*, which is weak evidence **against** a
      generally additive directory: the engine treats duplicates as errors.
      *Observed — NOT RUN.* P6's `error.log` line is relevant.

## Method

The empirical route: build minimal probe mods that define the same object with
distinguishable values, load them in both orders on an **isolated playset containing only
the probe's mods**, and observe in game. The probes, their key allocation and the run log
template are in `tools/probe-mods/README.md`. Every probe run stamps game version,
checksum, launcher version and OS, and records its canary readouts. Without the canaries,
an answer cannot be distinguished from a contaminated one.

Prior art (`docs/GROUND-TRUTH-PRIOR-ART.md`) drives the *ranking* of probes, not their
answers. The highest-value open items are V10 (`replace_path` scope), V02/V09 (filename
axis and vanilla's place on it), and V18 (`additive_children`). V09 moved into that group
in October: its August "corroborated" status did not survive a re-read.
