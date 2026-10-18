# M0.5 — Ground truth from prior art

`docs/VERIFY.md` lists ~35 load-bearing factual claims about Stellaris and the Paradox
launcher. They were written from reasoning, without the game. The empirical route (probe
mods, both load orders, observe) was blocked, so this pass took the second route the
Method section names: deriving the answers from prior art that already encodes them.

This document records **which projects were consulted, what each is licensed under and
whether its source could be read, where the sources disagreed, and what still needs the
game.** The findings themselves live inline in `docs/VERIFY.md`; the profile values live
in `profiles/stellaris.toml`.

Date of pass: 2026-08-06 (baseline `PA-2026-08` in `VERIFY.md`). Re-read in part on
2026-10-17 (baseline `PA-2026-10`), after Stellaris 4.5. See § 7, which corrects one
August conclusion.

**Nothing in this document is an observation of the game.** It is corroboration, and
`VERIFY.md` keeps it in a separate *Prior art* record under each item. A box there is
ticked only by an *Observed* record stamped with game and launcher version (H40).

---

## 1. Licence findings

Our outbound licence is `MIT OR Apache-2.0`. Every project's licence was checked **before**
any of its source was read.

| Project | Licence | Source read? | Notes |
| --- | --- | --- | --- |
| [cwtools/cwtools-stellaris-config](https://github.com/cwtools/cwtools-stellaris-config) | **MIT** (© 2018 tboby) | Yes | The question § `architecture/10` flagged — *"the config repo's licence may differ from the engine's"* — is answered: **it does not**. Both are MIT. |
| [cwtools/cwtools](https://github.com/cwtools/cwtools) | **MIT** (© Thomas Boby) | Yes | Engine + Clausewitz parser. |
| [bcssov/IronyModManager](https://github.com/bcssov/IronyModManager) | **MIT** (© 2020 Mario) | Yes | By far the richest source for this task. |
| [amtep/tiger](https://github.com/amtep/tiger) | **GPL-3.0** | **NO — docs only** | Copyleft. Per policy, only its README/docs were read; no source file was opened. |

### Consequence of tiger's licence

tiger is GPL-3.0, so under our policy its source is off limits and only its documentation
was consulted. In practice this cost us **nothing**, for a reason worth recording: **tiger
does not support Stellaris.** Its README states support for Crusader Kings 3, Victoria 3
and Imperator: Rome, with Hearts of Iron IV in development. Stellaris is not on the list.

> ⚠ The task brief for this milestone described tiger as covering "CK3/Vic3/Imperator, and
> Stellaris support". That is not correct as of this pass. tiger is still a good
> architectural reference — it is the closest project to our own design — but it is not a
> source of Stellaris facts, and no Stellaris claim in `VERIFY.md` rests on it.

### Consequence of the CWTools config licence being MIT

The "candidate shortcut" in § `architecture/10` — importing CWTools' `.cwt` files instead
of hand-writing `[[dir]]` entries — is **not blocked by licence**. MIT permits it with
attribution.

It is, however, **blocked by fitness for purpose**, which is the more interesting finding:

> **The CWTools Stellaris config does not encode merge modes.** It is a *type system* —
> it maps `type[trait]` to `path = "game/common/traits"`, declares each type's fields,
> cardinalities and scopes, and drives validation and autocomplete. It says nothing about
> what happens when two mods both define `trait_intelligent`. Across 66 config files the
> only structural options in use are `type_key_filter` (391 uses), `severity` (123),
> `path_strict` (9), `path_file` (7), `graph_related_types` (5) and `type_per_file` (4) —
> none of which is an override-semantics declaration.

So the expectation that the config repo is "close to a machine-readable answer to the
merge-mode questions" does not hold. It **is** valuable for something else: it is an
excellent machine-readable inventory of the vanilla directory tree (184 distinct
`path = "game/…"` declarations), and it was used that way — to enumerate directories worth
a `[[dir]]` entry, not to decide their merge modes.

**Nothing was copied.** No code, config file, comment or structure from any of these
projects has been reproduced in this repository. Every finding is restated in our own
words, and `profiles/stellaris.toml` was written from documented behaviour rather than
transcribed from `.cwt` files. Facts about how Stellaris behaves are not copyrightable;
these projects' expression of them is.

---

## 2. Access limitations encountered

Worth recording because it caps the confidence of several items and will affect anyone
repeating this work from the same environment:

- **`stellaris.paradoxwikis.com` is blocked (HTTP 403) by the session's egress policy.**
  So is `stellaris.fandom.com`, and so is `web.archive.org`. The wiki could therefore only
  be reached **indirectly, through web search result summaries**, which surface quoted
  passages but not the full page. Every wiki-derived claim in `VERIFY.md` should be
  re-checked against the page itself by someone who can load it — this is cheap and should
  be done before the probe pass, since it could upgrade two or three items from SINGLE
  SOURCE to CORROBORATED without needing the game at all.
- GitHub (repos, raw files, wikis) was fully accessible. Steam Community and Paradox forum
  content was reachable only through search summaries, same caveat.

---

## 3. Where the sources disagreed

Three real disagreements, plus one apparent one that dissolved on inspection.

### 3.1 `common/on_actions` — additive, or whole-file replacement?

- **The wiki** says on_action entries **accumulate**: a mod ships a uniquely named
  on_actions file and its events are called *in addition to* vanilla's, with events from
  multiple files firing in ASCII-betical filename order.
- **Irony** parses `common/on_actions` with its **whole-text** parser, treating the entire
  file as one indivisible definition.

These look contradictory but are not. Irony keys the whole-text definition on the
**filename**, so two mods hooking `on_game_start` in *differently named* files produce
different definition ids and Irony reports **no conflict** — the same practical outcome
the wiki describes. The disagreement is one of representation, not of behaviour, and both
sources agree on the outcome that matters: **this directory is not a conflict source in
the common case.**

What remains genuinely unresolved is narrower and is now the open question: *which child
keys accumulate*. Only the wiki speaks to it, and it names only `events`.

### 3.2 Load-order direction versus the filename axis

- **Community documentation and the design** say, flatly, "later in the playset wins".
- **Irony** implements a two-stage rule in which filename ordering can *override* load
  order, and inverts direction (FIOS) for fourteen Stellaris directories.

This is the most consequential disagreement in the pass. It is not that either side is
wrong — the community statement is true for the same-filename case, which is what users
usually mean — but the design took the simple statement as complete. It is not. See § 5.

### 3.3 `replace_path` — Irony disagrees with itself

Irony implements "a `replace_path` excludes vanilla **and every earlier mod's** content at
that path", and its author left a comment saying he *believes* that is how the game
behaves. That hedge is the only statement of intent available anywhere; no other source
addresses the question. So this is a case where the single source explicitly declines to
be authoritative, and it is treated as unresolved rather than as SINGLE SOURCE evidence.

### 3.4 Apparent disagreement that dissolved: `file_replace` vs `merge_by_key`

§ `architecture/10` notes that the community answer for `common/` is same-filename
replacement while the community answer for `common/defines/` is key-by-key merging, and
observes "the two cannot both be the general rule".

**They can, and both are true.** The rule is conditional on filename identity:
differently-named files merge by key; identically-named files replace wholesale. Irony,
CWTools and the community all encode this, and the design's error was modelling the two as
mutually exclusive properties of a directory. This is § 5's first contradiction.

---

## 4. Result

| State | Count |
| --- | ---: |
| `CORROBORATED` (2+ independent sources) | 7 |
| `SINGLE SOURCE` (one source) | 22 |
| `NEEDS THE GAME` (sources disagree or silent) | 7 |
| **Total** | **36** |

*As of August. Under the October evidence model these are prior-art states only, and none
ticks a box. V09 has since moved from CORROBORATED to SINGLE SOURCE (§ 7), and five
items were added (V37–V41). The current tally is in `VERIFY.md` § Status.*

36 rather than 35: prior art surfaced a question the original list did not know to ask —
the FIOS/LIOS filename axis — and it is now a box of its own.

**The distribution matters more than the totals.** Corroboration is scarce because there
is effectively **one** deep source of Stellaris override semantics in existence: Irony Mod
Manager. CWTools models a type system, not merge behaviour. The wiki is thin on
precedence. tiger does not cover Stellaris. So 22 of 36 items rest on a single project's
reading of an undocumented engine — better than our own guess, considerably worse than
observation, and a strong argument for not deferring the empirical pass indefinitely just
because the corroborated core is enough to start building.

The headline question — **does a mod later in the playset override an earlier one?** — is
**CORROBORATED: later wins**, from three independent sources. In August this section also
said "with vanilla always lowest" and called the `SourceId(0)` assumption **confirmed**.
**That was an overreach** (§ 7.2). Both sources speak to same-path collisions. For
differently named files, Irony keeps vanilla *in* the filename sort. And corroboration
was never confirmation in the first place.

---

## 5. Contradictions with the current design

These are reported, not fixed. Per the milestone's own instruction, no design document
other than `profiles/stellaris.toml` (authored fresh) has been edited to match. Each needs
a decision.

### 5.1 `MergeMode` cannot express the actual rule — **structural, highest impact**

> *October: addressed in design by `REVIEW-2026-10` H1 (file overlay before key resolution, § `architecture/07`). Still unobserved: V15.*

`file_replace` and `merge_by_key` are specified in § `architecture/03` and
§ `architecture/10` as mutually exclusive properties **of a directory**. Observed
behaviour is that both apply to the *same* directory, selected per pair of files by
whether their root-relative paths match:

- different filenames → entries merge, last-resolved key wins;
- identical filenames → the winner's file replaces the loser's **wholesale**, and keys
  present only in the loser's copy are lost.

Irony implements the second as an explicit gate (for each definition, find the
highest-load-order mod shipping that filename and discard the definition if that mod's
copy does not contain it). CWTools groups by root-relative path for the same reason.

Marking `common/traits` `merge_by_key` misses two mods both shipping `00_traits.txt` — a
real and severe conflict. Marking it `file_replace` inflates every ordinary trait overlap.
Neither value is correct because the question the enum asks is the wrong question.

**Suggested resolution:** keep `MergeMode` for the cross-file axis and add a
profile-level `same_filename_replaces_whole_file` flag plus a file-identity check in the
resolver. `profiles/stellaris.toml` is written on that assumption and flags the key as a
proposed schema addition.

### 5.2 Precedence has a second axis the model does not have — **structural**

> *October: addressed in design by `REVIEW-2026-10` H2 (the precedence tuple). Still unobserved: V02, V09, V37.*

Precedence is `(load-order index, filename)`, not load-order index alone, and the filename
comparison direction is **per directory**:

- most directories are **LIOS** — the last filename in ordinal order wins;
- fourteen Stellaris directories are **FIOS** — the *first* filename wins:
  `common/component_sets`, `common/component_templates`, `common/event_chains`,
  `common/global_ship_designs`, `common/scripted_variables`, `common/section_templates`,
  `common/ship_behaviors`, `common/special_projects`, `common/strategic_resources`,
  `events`, `common/solar_system_initializers`, `common/traits`,
  `common/start_screen_messages`, `common/governments/authorities`.

Load order breaks ties only *within* a filename. **A mod placed later in the playset can
lose a `common/traits` conflict to an earlier mod whose filename sorts lower.** The
comparison is ordinal — case-sensitive byte order, so `Z` sorts before `a`.

`SourceId` ordering alone therefore cannot resolve a conflict between differently-named
files, and any verdict the tool produces for those fourteen directories will be **inverted**
if this is not implemented. `profiles/stellaris.toml` carries a per-`[[dir]]` `order` key
and `[load_order].filename_axis` as proposed additions.

### 5.3 `common/traits` is a bad canonical example — **documentation**

> *October: § `architecture/03`'s example switched to `common/buildings`.*

The design uses a trait collision as its canonical "later mod wins" illustration.
`common/traits` is FIOS, so in that directory the later mod frequently does **not** win.
It is close to the worst available example. Recommend switching the canonical example to
an ordinary LIOS directory such as `common/buildings` or `common/edicts`.

### 5.4 `pdx.defines_overlap`'s base weight of 75 is mis-targeted — **scoring**

`common/defines/` is corroborated to merge **key by key**. If the rule fires on two mods
touching defines *files*, 75 points is noise: two mods each shipping a one-key defines
file with different keys is a complete non-event. The weight is defensible only when
scoped to an actual same-key collision (or to the filename-identity case, e.g. two mods
both shipping `00_defines.txt`, which is genuinely severe). The rule needs re-scoping,
not re-weighting.

### 5.5 `additive_children` is two-thirds unsupported — **profile data**

> *October: `REVIEW-2026-10` H4 gates `merge_by_key_additive_body` on child-level hashing as well as on V18.*

The design proposes `additive_children = ["events", "effect", "random_events"]` for
`common/on_actions`. Only `events` has any source behind it. `effect` in particular is
doubtful — an effect block is a statement list, not a set of keyed entries. Shipping all
three would assert two unchecked facts in the exact place the design says an unexamined
assertion becomes a confident wrong verdict. The profile ships `merge = "unknown"` with
the candidate list commented out.

### 5.6 Smaller corrections

- **`{DOCUMENTS}` must be resolved via the OS known-folder API**, not by joining
  `%USERPROFILE%` + `Documents`. OneDrive redirection repoints the known folder; string
  joining silently misses every redirected user. This belongs in the ingest layer, not the
  profile.
- **The launcher schema marker is not `user_version`.** The launcher is a Knex
  application with a `knex_migrations` table of *named* migrations. Detection means
  testing for named migrations, not comparing an integer.
- **`position` changed type across launcher schema versions** — zero-padded 10-character
  hex *string* in older schemas, integer in newer ones (migration
  `modifyPositionToInteger`). A reader that assumes either one will mis-sort the other.
- **`[[PARAM] … ]` is probably a CK3/Imperator form, not Stellaris.** Stellaris
  parameterises through `common/inline_scripts`. Verify before implementing it in the
  syntax layer.
- **`dependencies` are matched by display name, and there is no id-based upgrade path.**
  `remote_file_id` exists and is parsed, but is not what dependencies reference. The
  design's fuzzy-name assumption holds; `dep.missing` does not get to become exact.

---

## 6. Handover: what the probe mods must still resolve

Ranked by value — highest first. Value = (how much of the tool's output depends on it) ×
(how uncertain it currently is). Items 1–4 are worth building probes for; 5–9 are cheap
add-ons to those probes; 10–11 do not need the game at all and should be done first
because they are the fastest wins.

| # | Question | Why it ranks here | Probe design |
| --- | --- | --- | --- |
| **1** | **`replace_path` scope** — does it drop vanilla only, or vanilla *and every earlier mod* at that path? | Sole source is Irony, whose author explicitly wrote that he was guessing. Changes blast-radius reporting completely. | Three mods: A ships `common/edicts/a.txt`; B ships `common/edicts/b.txt`; C declares `replace_path = "common/edicts"` and ships `c.txt`. Load A, B, C in that order. Observe whether A's and B's edicts survive. Then reorder to C, A, B and repeat. |
| **2** | **FIOS/LIOS — is the fourteen-directory list real and complete?** | Fourteen directories produce **inverted** verdicts if wrong. Single source. | Two mods, each shipping one trait file with the *same* trait id but different filenames (`aaa_x.txt` vs `zzz_x.txt`) and distinguishable values. Load in both orders. If the `aaa_` value wins in both, FIOS is confirmed for `common/traits`. Repeat for one known-LIOS directory (`common/buildings`) as a control, then spot-check three more of the fourteen. |
| **3** | **`on_actions` `additive_children`** — which child keys accumulate? | Determines whether the tool's loudest potential finding is real. `events` supported; `effect`, `random_events` unsupported. | Two mods, differently named on_actions files, both declaring `on_game_start` with a distinguishable `events` entry *and* a distinguishable `effect` block. Observe whether both events fire and whether both effects run. |
| **4** | **Same-filename whole-file replacement** — confirm the gate directly. | Corroborated by three sources, but it is the load-bearing rule for § 5.1 and deserves a direct observation. | Two mods both shipping `common/edicts/00_edicts.txt`; the later one contains only one edict. Observe whether the earlier mod's other edicts (and vanilla's) survive. |
| **5** | Launcher DB **WAL** mode, and whether main-file-only copy returns stale order | Ingest correctness; § `architecture/05`'s copy protocol is entirely unvalidated. | Reorder a playset in an open launcher, copy main file only vs all three, compare. Check `PRAGMA journal_mode`. |
| **6** | Does the launcher hold a **Windows lock** preventing a file copy? | Determines whether ingest needs a fallback path at all. | Copy `launcher-v2.sqlite` with the launcher open on Windows. |
| **7** | `.mod` vs `descriptor.mod` — **which does the game honour** when they disagree? | Plausible cause of "launcher shows X, game does Y" bugs; nothing addresses it. | Ship a mod whose two descriptors declare different `name` and different `replace_path`. Observe which takes effect. |
| **8** | Two mods declaring the **same `replace_path`** | Falls out of probe 1 at near-zero extra cost. | Extend probe 1: give B a `replace_path` for the same directory. |
| **9** | **Multi-library Steam** — can the workshop live in a different library from the game? | Affects discovery on a minority of installs; cheap to settle. | Move the game to a second library, leave workshop content in the first, run discovery. |
| **10** | **`supported_version` wildcard shapes in the wild** | **Does not need the game.** Needs a scrape of a few thousand workshop descriptors. Do this now. | Scrape workshop `descriptor.mod` files; tabulate distinct `supported_version` forms. |
| **11** | **Non-UTF-8 encoding frequency** | **Does not need the game.** Same corpus as #10, same pass. | On the same scrape, detect encodings; confirm the `common/name_lists` BOM requirement. |

### Also worth doing before the probe pass

Re-read the Stellaris wiki's Modding, Game start modding and On actions pages **from an
environment that can load `paradoxwikis.com`**. Several items are SINGLE SOURCE only
because the wiki was reachable through search summaries rather than directly; a direct
read could upgrade the `on_actions`, `replace_path` and localisation items without any
probe at all.

### Not worth probing

- **Load-order direction** (later wins) is corroborated by three independent sources.
  Confirm it incidentally (probe P1), but do not spend more than that on it.
  *Withdrawn in October:* **vanilla precedence** was listed here too. It is not settled
  for differently named files (§ 7.2) and now has a dedicated probe (P4).
- **Operator set** — corroborated against a working Clausewitz parser's own enumeration.
  Settled.

---

## 7. October 2026 re-read (`PA-2026-10`, 2026-10-17)

A partial re-read prompted by the October review (`docs/REVIEW-2026-10.md`). This one
read through fetch summaries with quoted lines, not a local clone. The licence position
is unchanged, and again nothing was copied.

### 7.1 What the sources now target

- **Stellaris 4.5 "Cygnus"** shipped 2026-09-22. 4.5.1 (`358e`) followed on 2026-09-24
  and 4.5.2 (`9776`) on 2026-10-06. Source: the Steam announcements feed for app 281990;
  links in `VERIFY.md` § Stellaris 4.5 refresh.
- **Irony.** The stable notes still advertise *"Stellaris 4.2 CS support"*. The newest
  build is `v1.28.97-alpha`, a pre-release published on 4.5's release day, which claims
  only *"Latest Stellaris CS support"*. `master`'s last commit is 2026-07-29. So every
  Irony-derived finding in this document describes Irony's model of **≤ 4.4**, not of 4.5.
  This is the main reason the empirical pass is not optional.
- The FIOS path list at `v1.28.97-alpha` is **unchanged**: the same fourteen directories
  as § 5.2.
- **Launcher schema.** Irony's newest (`v4`) models add `playsets.isRemoved` and
  `playsets.createdOn` to the August picture. The only second source the review named,
  `Pseudorizer/paradox-launcher-playset-exporter`, is a HOI4 exporter whose README
  confirms the file name `launcher-v2.sqlite` and nothing else. It names no tables and
  has no releases. No source addresses WAL, locking, or the launcher shipped with 4.5.x.

### 7.2 Correction: vanilla is *not* always lowest in Irony's own model

August recorded "vanilla is always lowest" as CORROBORATED by CWTools and Irony. A closer
reading splits it:

- CWTools' `Overwrote`/`Overwritten` is decided per **logical path**, so it speaks only to
  same-path collisions.
- Irony's non-localisation priority evaluation removes a vanilla definition only when a
  mod definition **shares its filename**. That grouping is case-insensitive, while the
  sort is ordinal. A vanilla definition in a uniquely named file **stays in the ordinal
  filename sort** and can win: `00_…` beats a mod's `zzz_…` in a FIOS directory, and a
  mod's `!!!_…` in a LIOS one. Vanilla is pushed last only in the no-winner fallback.
  Localisation is the exception: there vanilla is excluded from the ordering.

So the corroborated claim is "vanilla loses same-path collisions". For differently named
files, the single source says vanilla participates. The review's point that this was
unestablished is **upheld**, and the design's `SourceId(0)`-is-lowest assumption is
restricted to same-path collisions until probe P4 runs. The same read shows Irony
compares the **basename without extension** on the filename axis, not the relative path.
That is a new single-source item, V37.

### 7.3 What changes in § 6

- Vanilla's place on the filename axis is now a probe (P4), not "confirm incidentally".
- The filename comparison key (V37), in-file duplicate keys (V38) and same-path
  localisation (V39) have probes too (P5–P7). On ≥ 4.5.1 the engine logs duplicate keys
  as errors, so `error.log` becomes a readout.
- Every probe in § 6 is amended in one way: it takes keys no other probe touches, and it
  runs on an **isolated playset containing only its own mods** (`tools/probe-mods/README.md`,
  H45). The August probe set had two probes overriding the same localisation key.
