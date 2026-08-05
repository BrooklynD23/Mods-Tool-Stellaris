# Assumptions to verify against a live install

This design was written without a Stellaris installation to hand. The items below are
the load-bearing factual claims. Each is encoded as **data** (in `profiles/stellaris.toml`)
rather than code specifically so that correcting one is a config edit, but they must be
confirmed before M1 ships — a wrong assumption here produces confidently wrong output,
which is the worst failure mode this tool has.

Confirm each, record the finding inline, and open an issue for any that differ.

## Load order semantics — **highest priority**

- [ ] **Does a mod later in the playset override an earlier one, or the reverse?**
      Everything in the tool keys off this. Verify empirically: two mods defining the
      same trait, observe which wins in game.
- [ ] Where does the current launcher store the active playset and its ordering?
      Expected: `launcher-v2.sqlite`, tables for playsets and playset↔mod with a
      position column. Confirm the schema and the column that carries order.
- [ ] Is the launcher's displayed order the load order, or is it re-sorted (by name, by
      dependency) before being handed to the game?
- [ ] Does `dlc_load.json` still reflect the active playset in current versions, or is it
      legacy?
- [ ] Where does vanilla sit in the precedence chain relative to mods? (Design assumes
      `SourceId(0)`, i.e. always lowest.)

## `replace_path`

- [ ] Confirm exact semantics: does it drop the vanilla directory only, or also that
      directory's contents from *all* mods loading earlier?
- [ ] Is it relative to the game root, and does it match recursively?
- [ ] Can a mod declare several? (Design assumes yes and accumulates.)
- [ ] What happens when two mods declare `replace_path` for the same directory?

## Directory merge behaviour

- [ ] For each `[[dir]]` entry in `profiles/stellaris.toml`, confirm the `merge` mode.
      The critical distinction: directories where a same-named file **replaces** the
      whole file, versus directories where entries **merge by key** across files.
- [ ] Confirm the widely-repeated claim that same-filename replacement is the rule for
      most of `common/`, and identify the exceptions.
- [ ] `common/defines/` specifically: does a mod's defines file replace the whole vanilla
      set, or merge key-by-key?
- [ ] Does `localisation/replace/` take precedence over `localisation/` as assumed?

## Paths

- [ ] Windows user directory when OneDrive has redirected `Documents`. This affects a
      large share of Windows users and is a common source of "no mods found".
- [ ] Linux native path: `~/.local/share/Paradox Interactive/Stellaris` vs
      `~/Documents/Paradox Interactive/Stellaris`.
- [ ] macOS path and whether the workshop directory differs in shape.
- [ ] Proton prefix path for a Windows install running on Linux.
- [ ] Multi-library Steam setups: is the workshop always under the same library as the
      game, or can it differ?

## Descriptors

- [ ] Do `.mod` (launcher copy) and `descriptor.mod` (in-mod copy) ever disagree in
      practice, and which one does the game actually honour?
- [ ] Are `dependencies` matched by display name or by `remote_file_id`? Design assumes
      name, which is fuzzy; if ids are available the matching becomes exact and
      `dep.missing` gets much stronger.
- [ ] Are zipped mods (`archive=`) still supported by the current launcher?
- [ ] What identifies a mod stably across reinstalls when `remote_file_id` is absent
      (local mods)?

## Version

- [ ] Where is the installed game version readable without launching? Candidates:
      `launcher-settings.json`, executable metadata, a `version.txt`.
- [ ] Confirm `supported_version` wildcard forms in the wild (`3.13.*`, `4.*`, `v3.13.*`,
      bare `3.13`) so the pattern parser covers all of them.

## Syntax

- [ ] Confirm the full operator set in current Stellaris script (`=`, `==`, `>`, `<`,
      `>=`, `<=`, `!=`, `?=`).
- [ ] Confirm `@variable`, `@[expression]`, `$PARAM$`, and `[[PARAM] … ]` forms.
- [ ] Sample real workshop mods for non-UTF-8 encodings and how common they are.
- [ ] Are there directories where duplicate top-level keys are legal and *both* apply
      (i.e. `MergeMode::Additive`)? If so, list them — reporting those as conflicts
      would be a false positive.

## Method

Fastest reliable route for most of these: build two minimal test mods that define the
same object with distinguishable values, load them in both orders, and observe in game.
Second: read Irony Mod Manager's and CWTools' source, both of which encode hard-won
answers to several of these questions.
