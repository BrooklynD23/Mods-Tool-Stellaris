# Probe mods — answering `docs/VERIFY.md` empirically

Six tiny mods, in three pairs. Each pair isolates **one** variable. None of them needs an
event, a console command, a save game, or a started campaign — every readout is visible on
the empire-creation screen within about a minute of launching.

Prior art has already answered some of this (`docs/GROUND-TRUTH-PRIOR-ART.md`). These
probes exist to confirm it against the running game, because prior art tells you what four
other implementations *believe* Stellaris does, and one of them says in writing that it
was guessing.

## Install

Copy the six mod folders into your mod directory, and the six `.mod` files from
`launcher-mod-files/` alongside them:

```
Documents/Paradox Interactive/Stellaris/
├── mod/
│   ├── smd_probe_order_a.mod          ← from launcher-mod-files/
│   ├── smd_probe_order_a/             ← the folder
│   └── … (six of each)
```

On Linux the base is `~/.local/share/Paradox Interactive/Stellaris/`. If Documents is
redirected to OneDrive, use the redirected location — and note which one worked, because
that is itself an open question in `VERIFY.md`.

Restart the launcher. All six should appear in the mod list. **If they do not appear at
all, stop and say so** — that is a descriptor problem, and it is worth knowing about
before drawing conclusions from a blank result.

## Probe 1 — does later in the playset win?

**Isolates:** load order. Both mods ship localisation at the **same path and the same
filename**, so any filename-based tiebreak is a tie and load order alone decides.

| Run | Playset (top → bottom) | Look at | Record |
| --- | --- | --- | --- |
| 1A | `Order A`, then `Order B` | Empire creation → Species → Traits | Is the trait called `PROBE1_A` or `PROBE1_B`? |
| 1B | `Order B`, then `Order A` | same | same |

**Reading it:** if run 1A shows `PROBE1_B` and run 1B shows `PROBE1_A`, then **later
wins** — the design's core assumption holds. If it shows the same string in both runs,
load order is *not* what decided it and something else is in play; record which string won
and move to probe 2.

## Probe 2 — is there a filename axis at all?

**Isolates:** filename. Same localisation key, but the files are named `aaa_…` and
`zzz_…`. If only load order mattered, the winner would flip when you swap the order.

| Run | Playset (top → bottom) | Record |
| --- | --- | --- |
| 2A | `Filename aaa`, then `Filename zzz` | `PROBE2_AAA` or `PROBE2_ZZZ`? |
| 2B | `Filename zzz`, then `Filename aaa` | same |

**Reading it:**

- **Same answer in both runs** → a filename axis exists and it outranks load order. If
  that answer is `PROBE2_ZZZ`, localisation is LIOS (last filename wins); if `PROBE2_AAA`,
  FIOS.
- **Answer flips with the order** → no filename axis here; load order decides. This is the
  result that would contradict the prior-art finding, so if you see it, note it loudly.

## Probe 3 — is `common/traits` really FIOS?

**Isolates:** the same filename axis, but in `common/traits`, which prior art lists as one
of fourteen **FIOS** directories where the *lowest*-sorting filename wins. This is the
finding that would invert the tool's verdicts for those directories, and it rests on a
single source.

Both mods define the trait `trait_smd_probe` in differently-named files, differing only in
`cost` — 1 in the `aaa_` file, 5 in the `zzz_` file.

| Run | Playset (top → bottom) | Look at | Record |
| --- | --- | --- | --- |
| 3A | `FIOS aaa`, then `FIOS zzz` | Traits list → "SMD PROBE TRAIT" | Cost 1 or 5? |
| 3B | `FIOS zzz`, then `FIOS aaa` | same | same |

**Reading it:**

- **Cost 1 in both runs** → FIOS confirmed for `common/traits`. The lowest filename wins
  and load order does not override it.
- **Cost 5 in both runs** → LIOS. The filename axis exists but runs the other way, and the
  fourteen-directory list is wrong or `common/traits` is not on it.
- **Cost flips with the order** → no filename axis in `common/traits`; load order decides,
  and the prior-art finding does not hold here.

Compare against probe 2: if probe 2 says LIOS for localisation and probe 3 says FIOS for
traits, that is direct confirmation that **direction is per-directory**, which is the part
the current data model cannot express.

### One caveat on probe 3

Probes 1 and 2 use only localisation, so there is no schema to get wrong. Probe 3 defines
a trait, and the trait schema changes between game versions. If "SMD PROBE TRAIT" does not
appear in the traits list at all, open `error.log` in the Stellaris user directory and
compare the probe file against a real entry in
`common/traits/00_species_traits.txt` in your game install — `allowed_archetypes` and the
`modifier` block are the likely culprits. Fix both probe files identically, keeping only
`cost` different between them.

## Record the results here, then in `VERIFY.md`

```
Game version:            ____________
Launcher version:        ____________
Mod dir that worked:     ____________

Probe 1A (A then B):     PROBE1_____      Probe 1B (B then A):  PROBE1_____
  → later wins?          yes / no

Probe 2A (aaa then zzz): PROBE2_____      Probe 2B (zzz then aaa): PROBE2_____
  → filename axis?       none / LIOS / FIOS

Probe 3A (aaa then zzz): cost ____        Probe 3B (zzz then aaa): cost ____
  → common/traits is:    load-order / LIOS / FIOS
```

Then transfer each answer into `docs/VERIFY.md` inline beneath its box, marked
`**VERIFIED**` rather than `**CORROBORATED**` — the distinction is the whole point of that
file, and an empirical answer should not be filed as if it were an inference.

## What these deliberately do not test

`replace_path` scope, the launcher SQLite schema and its WAL behaviour, `additive_children`
in `common/on_actions`, and same-filename replacement semantics. Those need either a
different probe shape or filesystem inspection rather than an on-screen readout, and they
are ranked in `docs/GROUND-TRUTH-PRIOR-ART.md`. Probes 1–3 are the ones that gate the data
model, so they come first.
