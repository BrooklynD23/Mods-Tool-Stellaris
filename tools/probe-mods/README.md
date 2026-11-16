# Probe mods — answering `docs/VERIFY.md` empirically

Twelve tiny mods for seven probes. Each probe isolates **one** variable. None of them
needs an event, a console command, a save game, or a started campaign. Every readout is
visible on the empire-creation screen (species traits) or the planet build menu within a
couple of minutes of launching.

Prior art has already guessed most of these answers (`docs/GROUND-TRUTH-PRIOR-ART.md`;
the guesses are in each `VERIFY.md` item's *Prior art* record). These probes exist to
observe the running game, because prior art tells you what other implementations
*believe* Stellaris does, and one of them says in writing that it was guessing.

| Probe | Mods | Answers | Runs |
| --- | --- | --- | --- |
| P0 | *none* | the baseline readouts every other run is checked against | 1 |
| P1 | `order_a`, `order_b` | V01 later wins (same path); V07 incidentally | 2 |
| P2 | `fname_aaa`, `fname_zzz` | V02/V21 filename axis in localisation | 2 |
| P3 | `fios_aaa`, `fios_zzz` | V02 `common/traits` FIOS; V14, V17 | 2 |
| P4 | `vanilla` | V09 vanilla's place on the filename axis | 1 |
| P5 | `fkey_a`, `fkey_b` | V37 basename vs relative path | 2 |
| P6 | `dup` | V38 duplicate key within one file | 1 |
| P7 | `locpath_a`, `locpath_b` | V39 same-path localisation: whole file or per key | 2 |

## The isolation rule — read this first (H45)

**Every run uses a playset that contains only the mods listed for that probe, and
nothing else.** That means no other probe mods and no workshop mods, not even "harmless"
ones. Create one launcher playset per run, named after the run (`P3A`, `P3B`, …).

This is not tidiness. Probes 2, 3 and 5 measure a filename axis. In a FIOS directory, any
other mod shipping a lower-sorting file that touches the same key changes the winner. A
probe answer from a mixed playset therefore cannot be told apart from a contaminated one.
Earlier revisions of these mods had probes 1 and 2 overriding the same key
(`trait_intelligent`), so running both in one playset made each answer the other's
noise. That is fixed: **no two probes share a key, an object id, or a file path** (table
below). The rule still applies, because a workshop mod can touch anything.

**Canaries.** Because keys are disjoint, each run can prove its own isolation: every
readout that belongs to a *different* probe must show the P0 baseline value. Record the
canaries in every run. A run whose canaries are not at baseline is void, whatever its own
readout says.

### Key allocation

A new probe takes a fresh row. It never reuses a key, object id or file path from another
row.

| Probe | Keys / objects touched | Files |
| --- | --- | --- |
| P1 | loc `trait_intelligent` | `localisation/english/smd_probe_l_english.yml` (same path in both mods, on purpose) |
| P2 | loc `trait_strong` | `localisation/english/aaa_smd_probe_l_english.yml`, `…/zzz_smd_probe_l_english.yml` |
| P3 | trait `trait_smd_probe3`; loc `trait_smd_probe3`, `trait_smd_probe3_desc` | `common/traits/aaa_smd_probe.txt`, `…/zzz_smd_probe.txt`; `localisation/english/smd_probe3_{aaa,zzz}_l_english.yml` |
| P4 | trait `trait_thrifty`, `trait_smd_probe4_marker`; building `building_research_lab_1` (or substitute, logged); loc `trait_rapid_breeders`, `trait_slow_breeders`, `trait_smd_probe4_marker` | `common/traits/zzz_smd_probe4_traits.txt`, `common/buildings/!!!_smd_probe4_buildings.txt`, `localisation/english/!!!_smd_probe4_l_english.yml`, `…/zzz_smd_probe4_l_english.yml` |
| P5 | trait `trait_smd_probe5`; loc `trait_smd_probe5` | `common/traits/a_smd/zzz_smd_probe5.txt`, `common/traits/b_smd/aaa_smd_probe5.txt`; `localisation/english/smd_probe5_{a,b}_l_english.yml` |
| P6 | trait `trait_smd_probe6`; loc `trait_smd_probe6` | `common/traits/smd_probe6.txt`, `localisation/english/smd_probe6_l_english.yml` |
| P7 | loc `trait_agrarian` (A), `trait_industrious` (B) | `localisation/english/smd_probe7_l_english.yml` (same path in both mods, on purpose) |

Inside P3 and P5, both mods carry the loc name of the probe trait in differently named
files with **identical** values. Their collision is invisible by construction.

## Install

Copy the twelve mod folders into your mod directory, and the twelve `.mod` files from
`launcher-mod-files/` alongside them:

```
Documents/Paradox Interactive/Stellaris/
├── mod/
│   ├── smd_probe_order_a.mod          ← from launcher-mod-files/
│   ├── smd_probe_order_a/             ← the folder
│   └── … (twelve of each)
```

On Linux the base is `~/.local/share/Paradox Interactive/Stellaris/`. If Documents is
redirected to OneDrive, use the redirected location, and note which one worked (V22).

**Prepare P4 before any run.** Its two `common/` files ship as instructions only, because
the vanilla trait and building bodies change between game versions and must be copied
from *your* install, verbatim. Follow the comments at the top of
`smd_probe_vanilla/common/traits/zzz_smd_probe4_traits.txt` and
`smd_probe_vanilla/common/buildings/!!!_smd_probe4_buildings.txt`. Until that is done, P4
is void.

Restart the launcher. All twelve should appear in the mod list. **If they do not appear at
all, stop and say so.** That is a descriptor problem, and it is worth knowing about before
drawing conclusions from a blank result.

## P0 — baseline (no mods)

Run once with an **empty** playset. Record the vanilla value of every readout used below:
the displayed names of Intelligent, Strong, Rapid Breeders, Slow Breeders, Agrarian and
Industrious; the cost of Thrifty; the minerals cost of the P4 building. Also record that
no "SMD PROBE…" trait exists. Every later run compares against these values.

## P1 — does later in the playset win?

**Isolates:** load order. Both mods ship localisation at the **same path**, so any
filename tiebreak is a tie and load order alone decides.

| Run | Playset (top → bottom), nothing else | Look at | Record |
| --- | --- | --- | --- |
| 1A | `Order A`, then `Order B` | Species → Traits → Intelligent | `PROBE1_A` or `PROBE1_B`? |
| 1B | `Order B`, then `Order A` | same | same |

**Reading it:** `PROBE1_B` in 1A and `PROBE1_A` in 1B → **later wins** (V01 VERIFIED).
The same string in both runs → load order did not decide it; record which string won.

## P2 — is there a filename axis in localisation?

**Isolates:** filename. Same key (`trait_strong`), files named `aaa_…` and `zzz_…`.

| Run | Playset (top → bottom), nothing else | Record (Strong's name) |
| --- | --- | --- |
| 2A | `Filename aaa`, then `Filename zzz` | `PROBE2_AAA` or `PROBE2_ZZZ`? |
| 2B | `Filename zzz`, then `Filename aaa` | same |

**Reading it:**

- **Same answer in both runs** → a filename axis exists and it outranks load order.
  `PROBE2_ZZZ` means LIOS for localisation; `PROBE2_AAA` means FIOS.
- **Answer flips with the order** → no filename axis here; load order decides. That
  contradicts prior art, so note it loudly.

## P3 — is `common/traits` really FIOS?

**Isolates:** the filename axis in `common/traits`, which prior art lists as one of
fourteen **FIOS** directories (V02, single source). Both mods define `trait_smd_probe3`
in differently named files, differing only in `cost`: 1 in the `aaa_` file, 5 in the
`zzz_` file.

| Run | Playset (top → bottom), nothing else | Look at | Record |
| --- | --- | --- | --- |
| 3A | `FIOS aaa`, then `FIOS zzz` | "SMD PROBE3 TRAIT" | Cost 1 or 5? |
| 3B | `FIOS zzz`, then `FIOS aaa` | same | same |

**Reading it:** cost 1 in both runs → FIOS. Cost 5 in both → LIOS. Cost flips → load
order decides in `common/traits`. If P2 says LIOS and P3 says FIOS, that directly
confirms that **direction is per directory**.

### Caveat for P3, P5 and P6

These define a new trait, and the trait schema changes between game versions. If an "SMD
PROBE" trait does not appear at all, open `error.log` in the Stellaris user directory and
compare the probe file against a real entry in your install's `common/traits/`.
`allowed_archetypes` and the `modifier` block are the likely culprits. Fix every probe
trait file identically, keeping only `cost` different.

## P4 — is vanilla always lowest, for differently named files?

**Isolates:** whether vanilla takes part in the filename axis (V09). The design assumes
vanilla is always lowest. Prior art supports that only for **same-path** collisions.
Irony's own resolver keeps vanilla in the filename sort when the names differ (outside
localisation). One mod, run alone, with every file named so that a participating vanilla
file would beat it:

| Readout | Mod file | Why that name | If vanilla is always lowest | If vanilla takes part in the axis |
| --- | --- | --- | --- | --- |
| Thrifty cost | `common/traits/zzz_…` | FIOS dir; `zzz_` sorts after vanilla's file | 9 | vanilla cost |
| P4 building minerals cost | `common/buildings/!!!_…` | LIOS dir; `!!!_` sorts before vanilla's file | 777 | vanilla cost |
| Rapid Breeders name | `localisation/english/!!!_…` | loses to vanilla if localisation is LIOS | `PROBE4_FIRST` | vanilla name |
| Slow Breeders name | `localisation/english/zzz_…` | loses to vanilla if localisation is FIOS | `PROBE4_LAST` | vanilla name |

| Run | Playset, nothing else | Record |
| --- | --- | --- |
| 4 | `Vanilla vs differently named files` | all four readouts, the vanilla file names you copied from, and whether "SMD PROBE4 MARKER" exists |

**Reading it:** "SMD PROBE4 MARKER" must be visible, which proves the traits file was
parsed. If it is missing, the run is void. Then:

- **All four show the mod value** → vanilla is always lowest (V09 VERIFIED for these
  directories).
- **A `common/` readout shows the vanilla value** → vanilla takes part in the filename
  axis in that directory (V09 CONTRADICTED). Expect this to hold for both directories
  together, and if it does not, say so.
- Under any reading, at most one localisation readout should show vanilla, and which one
  should agree with P2's direction. Irony's reading is that both show the mod value.

## P5 — does the filename axis compare the basename or the path?

**Isolates:** the comparison key (V37). Both mods define `trait_smd_probe5` in a
subdirectory of `common/traits`: mod A in `a_smd/zzz_smd_probe5.txt` (cost 2), mod B in
`b_smd/aaa_smd_probe5.txt` (cost 8). By basename, `aaa_` sorts first. By relative path,
`a_smd/` sorts first.

| Run | Playset (top → bottom), nothing else | Record |
| --- | --- | --- |
| 5A | `Filename key (a)`, then `(b)` | cost 2 or 8? |
| 5B | `(b)`, then `(a)` | same |

**Reading it** (assuming P3 found FIOS; invert if P3 found LIOS): 8 in both → basename.
2 in both → directory-relative path. Flips → load order decides. **Trait absent** → the
game does not read subdirectories of `common/traits`. Record that as its own finding,
because the profile globs are `common/traits/**`.

## P6 — the same key twice in one file

**Isolates:** in-file duplicates (V38). One mod, one file, `trait_smd_probe6` defined
twice: cost 3, then cost 7.

| Run | Playset, nothing else | Record |
| --- | --- | --- |
| 6 | `Duplicate key in one file` | cost 3 (first kept) or 7 (last kept); the `error.log` line verbatim |

On ≥ 4.5.1 the patch notes say duplicate keys "now always log an error". Its absence is
itself worth recording.

## P7 — same-path localisation: whole file or per key?

**Isolates:** whether two localisation files at the **same path** replace each other
wholesale (V39). Both mods ship `localisation/english/smd_probe7_l_english.yml`. A renames
Agrarian to `PROBE7_A`, and B renames Industrious to `PROBE7_B`. The keys are disjoint.

| Run | Playset (top → bottom), nothing else | Record (Agrarian / Industrious names) |
| --- | --- | --- |
| 7A | `Localisation same path (a)`, then `(b)` | |
| 7B | `(b)`, then `(a)` | |

**Reading it:** both renamed in both runs → per-key merge. Only the later mod's key
renamed, with the other at baseline → whole-file replacement, the V15 rule extended to
localisation.

## Run log — record here, then in `VERIFY.md`

Fill one header per session, and one line per run. A run without canaries is not
recorded.

```
Game version (launcher-settings.json rawVersion / version):  ____________
Checksum (main menu, bottom corner):                          ____
Launcher version:                                             ____________
OS / install (Steam/GOG/Proton):                              ____________
Mod dir that worked:                                          ____________
Date:                                                         ____________

P0  baseline:   Intelligent=____ Strong=____ RapidBr=____ SlowBr=____ Agrarian=____
                Industrious=____ Thrifty cost=__ P4 building (id ____) minerals=___
Run  Playset (top→bottom)       Readout                Canaries at baseline?  Void?
1A   order_a, order_b           Intelligent=________   yes / no (which)       y/n
1B   order_b, order_a           Intelligent=________
2A   fname_aaa, fname_zzz       Strong=________
2B   fname_zzz, fname_aaa       Strong=________
3A   fios_aaa, fios_zzz         P3 cost=__
3B   fios_zzz, fios_aaa         P3 cost=__
4    vanilla                    Thrifty=__ bldg=___ RapidBr=____ SlowBr=____ marker y/n
                                vanilla files: traits ________ buildings ________
5A   fkey_a, fkey_b             P5 cost=__ (absent?)
5B   fkey_b, fkey_a             P5 cost=__
6    dup                        P6 cost=__  error.log: ______________________
7A   locpath_a, locpath_b       Agrarian=____ Industrious=____
7B   locpath_b, locpath_a       Agrarian=____ Industrious=____
```

Then transfer each answer into its `VERIFY.md` item's *Observed* record, as `VERIFIED`,
`CONTRADICTED` or `INCONCLUSIVE`, with the session header as the stamp. Do not edit the
*Prior art* record to match. The two records are kept apart on purpose (H40). Check the
filled log in next to the `VERIFY.md` diff.

## What these deliberately do not test yet

`replace_path` scope and the two-declarer case (V10–V13), `additive_children` in
`common/on_actions` (V18), whole-file *loss* on same-path collisions in `common/` (V15),
the launcher schema, WAL and locking (V03–V06, V40, V41), and `.mod` vs `descriptor.mod`
(V27). These need either a different probe shape or filesystem inspection rather than an
on-screen readout. The `GROUND-TRUTH-PRIOR-ART.md` § 6 designs apply, with one
amendment: each one takes fresh rows in the key-allocation table above and runs on its
own isolated playset (H47).
