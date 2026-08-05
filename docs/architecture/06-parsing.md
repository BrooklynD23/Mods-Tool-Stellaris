# 06 — Parsing layer

## Design stance

The parser is **not** a general Clausewitz interpreter. It is a fact extractor. It needs
to know where named things are defined and referenced, and nothing about what they mean.
That restriction is what keeps it fast, robust against syntax it has never seen, and
free of game knowledge.

Consequences of that stance:

- No AST is retained. Facts stream out; the parse of a file is dropped as soon as its
  facts are emitted. Peak memory stays proportional to `cores × largest file`, not to
  the corpus.
- Unknown constructs are skipped, not rejected. A file using syntax from a future patch
  still yields the facts it does contain.
- Value semantics are ignored. `x = { … }` bodies are hashed, not interpreted.

## Two-layer parser

```
smd-syntax        primitives: byte cursor, comment/whitespace skipping, quoting,
                  brace depth tracking, span arithmetic
  ├─ smd-clausewitz   PDX script → events
  └─ smd-locyml       PDX localisation → events
```

`smd-clausewitz` is an **event-driven (push) parser**, not a tree builder:

```rust
pub trait ScriptSink {
    fn block_open(&mut self, key: &Token, depth: u16, span: Span);
    fn block_close(&mut self, depth: u16, span: Span);
    fn assign(&mut self, key: &Token, op: Op, value: &Value, depth: u16, span: Span);
    fn bare(&mut self, value: &Value, depth: u16, span: Span);
    fn error(&mut self, d: Diagnostic);
}
```

Callers implement a sink. The fact extractor is one sink (~120 lines); a pretty-printer,
a future validator, and the fuzz harness are others. Nobody pays for a tree they do not
want.

## Clausewitz syntax the lexer must survive

Written down because a naive regex approach breaks on most of these:

```
# comment to end of line
key = value
key = "quoted value with spaces and # not-a-comment"
key = { nested = { deep = yes } }
key = { a b c }                   # bare list
key > 5   key <= 3   key != foo   # comparison operators as assignment ops
@my_const = 10                    # script variable definition
size = @my_const                  # variable use
value = @[ my_const + 1 ]         # inline maths
$PARAM$ = yes                     # parameter substitution in scripted effects
[[PARAM] key = $PARAM$ ]          # conditional block in scripted effects
color = rgb { 128 64 32 }         # prefixed block
color = hsv { 0.5 0.5 1.0 }
key = yes / no                    # booleans
key = 1.5   key = -3              # numbers
"quoted key" = value              # keys may be quoted
key = {}                          # empty block
key = value key2 = value2         # no separators required, newline is not significant
```

Additional realities:

- **Encoding.** Files are usually UTF-8, frequently with a BOM, and occasionally
  Windows-1252 (older mods). Strategy: strip BOM, attempt UTF-8, fall back to a lossy
  Windows-1252 decode and emit a `Warn` diagnostic. Never fail the file.
- **Line endings.** CRLF and LF both, sometimes mixed within a file.
- **Duplicate keys are legal** and meaningful. The parser must not deduplicate.
- **Unbalanced braces exist in shipped mods.** Recovery: at EOF with depth > 0, emit a
  diagnostic and close the open blocks; keep everything parsed so far.

Target throughput for the lexer: **≥200 MB/s per core** on mmap'd bytes, achieved by
operating on `&[u8]` with a byte-class lookup table and never allocating for tokens that
turn out to be uninteresting.

## From events to facts

The fact extractor's whole job is deciding which `block_open` at depth 0 is a
**definition** — and that decision is *data*, from the game profile:

```toml
# profiles/stellaris.toml  (excerpt)
[[dir]]
glob   = "common/traits/**/*.txt"
domain = "trait"
merge  = "merge_by_key"
level  = 0            # definitions are the depth-0 keys

[[dir]]
glob   = "common/defines/**/*.txt"
domain = "define"
merge  = "merge_by_key"
level  = 1            # depth-0 is the group (NGameplay), depth-1 is the actual key

[[dir]]
glob   = "events/**/*.txt"
domain = "event"
extractor = "event"   # named extractor: namespace + id, not the block key

[[dir]]
glob   = "gfx/**"
domain = "asset"
parser = "none"       # hash only, never opened
```

Three extraction strategies cover essentially everything:

| Strategy | Behaviour | Used for |
| --- | --- | --- |
| `depth_key` (default) | every key at `level` is a definition | `common/**` |
| `event` | read `namespace = x` at depth 0, then `id = x.N` inside each event block | `events/**` |
| `none` | hash only | `gfx/**`, `music/**`, `sound/**` |

A new vanilla directory in a game patch is a TOML entry. A genuinely novel shape is a
new named extractor: one file implementing one trait, registered like any other plugin.

### Body hashing

Each definition carries a `body_hash` over its **normalised** body — comments stripped,
runs of whitespace collapsed, so cosmetic reformatting does not read as a change. This
single field powers the most valuable noise reduction in the tool: two mods defining the
same symbol with byte-identical bodies (extremely common, because mod A vendored mod B's
file) is scored down to `Info` instead of shouting `High`.

## Localisation parsing

`.yml` under `localisation/` is Paradox's own format, not YAML — a real YAML parser
fails on it, which is why `smd-locyml` exists:

```yml
l_english:
 trait_super_adaptive:0 "Super Adaptive"
 trait_x:1 "Value with \"escapes\" and $VAR$ and £energy£ and §Ycolour§!"
```

The parser extracts `(lang, key, version_ordinal, value_hash)` and keeps the raw value
only for the diff view. Notes: the leading space is conventional but not required; the
`:0` ordinal is optional; `l_english` may appear more than once in a file; `replace/`
subdirectories under `localisation/` take precedence over the base directory.

## Parser registry

```rust
pub trait FileParser: Sync {
    fn id(&self) -> &'static str;
    fn accepts(&self, kind: FileKind, path: &NormPath, profile: &GameProfile) -> bool;
    fn parse(&self, input: &ParseInput, sink: &mut dyn FactSink);
}

inventory::submit!(&ClausewitzScriptParser as &dyn FileParser);
```

Registration is via the `inventory` crate: **adding a parser adds a file and nothing
else** — no central list to edit, no merge conflicts between contributors adding parsers
in parallel. The registry resolves ties by explicit priority, and `cargo xtask lint`
fails on two parsers claiming the same `(kind, glob)` at the same priority.

## Parse caching

The most impactful performance decision in the system. Cache key:

```
blake3(file bytes) ‖ parser_id ‖ parser_version ‖ CONTRACT_VERSION ‖ profile_hash
```

Value: the file's serialised `Vec<Fact>` (rkyv or postcard — zero/near-zero-copy read).

Because the key is content-addressed, the cache survives mod updates, path changes,
reinstalls, and even the same file vendored by five different mods (one entry, five
hits). Bumping `parser_version` invalidates exactly the affected entries and nothing
else. The `profile_hash` term is what makes editing `stellaris.toml` safe — changing a
directory's domain correctly invalidates the facts derived under the old mapping.

Store: `redb`, single file under the OS cache dir, LRU-evicted at a configurable cap
(default 512 MB). Writes are batched on a dedicated thread so cache IO never blocks the
parse pipeline.
