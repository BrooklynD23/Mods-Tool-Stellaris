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

### Iterative, with an explicit depth cap

The parser tracks nesting with its own counter and a `Vec` of open-block state. It never
recurses on input structure. This is not a style preference: `catch_unwind` — the
mechanism the rest of the design relies on to contain bad input (§ `08-rule-engine.md`)
— cannot catch a stack overflow, and § `13-testing.md` fuzzes 10k-deep nesting against a
"never panics on any byte sequence" guarantee. A recursive-descent parser does not fail
that test, it aborts the process running it.

Nesting beyond **256** is not meaningful Clausewitz. At the cap the parser emits
`Diagnostic::DepthLimit { span, depth }`, skips to the close of the outermost block that
exceeded it, and continues. Every fact outside that block still reaches the sink.

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

Target throughput for the lexer: **≥200 MB/s per core** over a `&[u8]` buffer, achieved
with a byte-class lookup table and by never allocating for tokens that turn out to be
uninteresting.

That target is a floor on a microbenchmark, not a lever on the budget. ~400 MB of text
at 200 MB/s across 8 cores is ~0.26 s — about 1% of the cold scan and a small fraction
of the parse stage that contains it (§ `12-performance.md`). What the parse stage
actually spends its time on is fact extraction, body hashing, and cache writes. Keep the
lexer fast enough to stay off the profile; do not expect to buy the budget back there.

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

That discount is only as good as the body it hashed. A definition whose block was closed
by brace recovery or cut by the depth cap hashes a truncated body, and two truncations can
agree where the real bodies do not. Such a file is `ParseCompleteness::Partial`, and policy
refuses every reassuring modifier on a finding whose completeness is not `Complete`
(§ `09`, H18).

## Completeness — what the parser did not see

Every file the overlay knows about leaves stage 5 with a `ParseCompleteness` (§ `03`),
recorded per contribution in the index:

| Value | Set when | What it rules out |
| --- | --- | --- |
| `Complete` | Parsed end to end with no recovery | nothing |
| `Partial` | Brace recovery at EOF, `DepthLimit`, a block skipped as unknown syntax, or a replacement character inside a key token after the Windows-1252 fallback | any claim that a symbol is *absent* from this file, and any agreement discount over its bodies |
| `Unparsed(reason)` | Above the 32 MB cap, unreadable, changed during the scan, refused at the ingest trust boundary (link, cloud placeholder, special file, root escape, unsafe archive entry — § `05`), or no parser accepts the path | every claim about this file's contents; only path-level and blob-level facts remain |
| `NotApplicable` | `parser = "none"` | nothing — no facts were ever expected |

The failure this exists to prevent is specific. A mod replaces
`common/technology/00_eng_tech.txt` with a 40 MB file. The file is skipped for parsing;
the overlay correctly marks vanilla's copy dead; vanilla's definitions now have no live
site; and `core.symbol_deleted` reports every engineering tech as gone, at the highest
base weight in the catalogue — about the one mod that may well define all of them. "We
did not read it" became "it is not there". An unparsed or partial file is therefore a
stated gap on every negative claim whose scope includes it (`ResolutionGap::ParseIncomplete`,
§ `07`): the scope of a key in a `merge_by_key` directory is every surviving file in that
directory, because any filename can define it. Absence of findings is not a clean bill
either; the report states `ScanCompleteness` — counts of `Partial` and `Unparsed` files
and the domains they touch — beside the findings, even when there are none (§ `08`).

Some gaps have no file to hang on. A source whose tree exceeded an enumeration bound, a
directory link refused at the trust boundary (its subtree is never walked), or an archive
refused whole or not yet read (§ `05`), has files the scan never listed, so no
per-file record exists to mark. Such a source is **content-incomplete**: it could define
any key in any directory, so every negative claim in a scope it could contribute to — in
practice, every `Deleted` while it is not `Inactive` — carries `ParseIncomplete`, and
`ScanCompleteness` counts it separately as *sources truncated or content-unread*. Harsh,
and correct: the alternative is a deletion verdict about a mod whose files were not read.

Encoding fallback alone is `Complete`: Windows-1252 decodes every byte except five, and a
replacement character only matters where it lands inside an identifier.

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
fails on two parsers claiming the same `(kind, glob)` at the same priority (not yet
built; gated H37, M2 — § `13` lint ledger).

`ParseInput` hands the parser a `&[u8]` borrowed from a pooled per-thread read buffer,
not a mapping. § `12-performance.md` records why that is the default and the two
conditions under which mmap is permitted instead.

## Parse caching

The most impactful performance decision in the system — and it is two caches, not one.

### What is cached: `RawFact`, never `Fact`

A `Fact` holds interned ids: `SymbolId`, `PathId`, `SourceId`. Interning is
**scan-local**. The same symbol receives a different id on the next run, and an id minted
while parsing mod A means something else entirely when that entry is served for mod B
three scans later. A cache of interned facts attributes facts to the wrong mod —
silently, with no error, on every warm scan, which is the normal path.

So the cached form is `RawFact`: file-local and string-based, no ids, no cross-file
references, spans as raw-file byte offsets. Interning happens *after* the cache, on the
way into the index. The governing clause lives in § `03-data-contracts.md` and is
absolute: **interned ids are scan-local and must never be serialised, cached, or cross
the API boundary.**

Value encoding is rkyv or postcard — one `Vec<RawFact>` per file, decoded with zero or
near-zero copying.

### Layer 2 — content hash and classification → facts

```
blake3(file bytes) ‖ parser_id ‖ parser_version ‖ CONTRACT_VERSION ‖ profile_hash
                   ‖ ExtractionClass::key_bytes()
```

Because the key is content-addressed, the cache survives mod updates, path changes,
reinstalls, and even the same file vendored by five different mods (one entry, five
hits). Bumping `parser_version` invalidates exactly the affected entries and nothing
else. The `profile_hash` term is what makes editing `stellaris.toml` safe — changing a
directory's domain correctly invalidates the facts derived under the old mapping.

**Content is not the only input to a parse, and the key must name every input.** The facts
a file yields depend on its bytes *and* on where it sits: the cached `Definition` carries a
`domain`, the `level` decides which keys are definitions, the extractor decides whether
`id = x.1` is an event, and `additive_children` decides whether an `AdditiveSplit` is
emitted (§ `03`). `profile_hash` covers none of that, because it is one value for every
path. Without the `ExtractionClass` term, a file vendored into `common/traits/` and the
same bytes dropped into `common/civics/` share an entry — whichever was parsed first
supplies the other's facts, under the other's domain, on every warm scan, and the tool
reports trait collisions among civics. The term is the effective classification of the
path (`kind`, parser, extractor, `level`, domain name, the `additive_children` set),
encoded canonically; two paths with equal classifications still share an entry, which is
the vendoring case the cache exists for. A fixture pins it: identical bytes under two
directories with different domains must produce two entries and two different fact sets
(H10).

The cached value is the parse, not just its facts:

```rust
struct CachedParse { facts: Vec<RawFact>, diags: Vec<RawDiagnostic>, completeness: ParseCompleteness }
```

A cache of facts alone makes the warm scan forget every recovery the cold scan performed:
the malformed file's diagnostics vanish and its `Partial` becomes an implicit `Complete`,
so the incompleteness guard above holds on the first run and fails on every run after
it. `Unparsed` results are never cached — the reason (`ChangedDuringScan`, `Unreadable`)
belongs to the scan, not to the bytes.

### Layer 1 — stat → content hash

```
(volume_id, file_id, size, mtime_ns) → content_hash
```

Layer 2 alone does not produce a fast warm scan; it produces a warm scan that re-reads
and re-hashes 1.3 GB in order to compute the keys of a cache it is about to hit. A
content-addressed cache pays off only when something cheaper supplies the hash, and that
is all layer 1 is.

With it, a warm scan **stats and stops**. On Windows the parallel directory walk already
returns size and mtime for every entry as part of enumeration, so the whole layer costs
no additional syscall. `volume_id`/`file_id` come from `FILE_ID_INFO` on Windows and
`(st_dev, st_ino)` on Unix; the path is deliberately not part of the key, so moving or
renaming a mod directory keeps every entry.

Any of the four fields differing is a miss: re-read, re-hash, re-key. A missing or
malformed row is a miss, never a hit. One case escapes it — a file rewritten within the
same mtime tick and to the same byte length is invisible to layer 1 — which is why
`--no-stat-cache` exists and is the first thing a suspicious bug report is asked to run.

### Store and eviction

SQLite via `rusqlite` (`bundled`), one file under the OS cache dir, WAL mode, one table
per layer. It is the database the launcher reader already links (§ ADR-0001), so the
parse cache costs no new dependency and gets a query planner, a crash-safe journal, and
`VACUUM` for free.

Eviction is LRU at a configurable cap (default 512 MB), expressed directly:

```sql
DELETE FROM facts WHERE key IN (SELECT key FROM facts ORDER BY atime LIMIT ?);
```

**`atime` is never written on the read path.** A warm scan hits layer 2 roughly 88,000
times; stamping an access time per hit converts a read-only workload into 88,000 writes
serialised through a single writer, which costs more than the parsing it replaced. The
scan instead collects the keys it touched and stamps them in one batched transaction at
the end. That makes eviction order scan-granular, which is the only granularity an LRU
over a per-scan working set can use. Writes go through a dedicated thread so cache IO
never blocks the parse pipeline.
