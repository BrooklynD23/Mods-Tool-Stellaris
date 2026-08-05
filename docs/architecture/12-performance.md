# 12 — Performance budget

Targets are stated up front so regressions are measurable rather than debatable. These
are **design targets on the reference machine**, not measurements — nothing is built
yet. CI enforces them once the corpus generator exists.

## Reference workload and machine

- **Playset:** 300 mods, ~400k files, ~1.2 GB on disk, of which ~250 MB is text
  (the rest is `.dds`/`.png`/audio, hashed but never parsed).
- **Machine:** 8-core desktop, NVMe SSD, 16 GB RAM, Windows 11.

## Targets

| Operation | Cold (empty cache) | Warm (unchanged playset) |
| --- | --- | --- |
| Full scan, end to end | **≤ 20 s** | **≤ 2 s** |
| — discover + load order | ≤ 0.3 s | ≤ 0.3 s |
| — enumerate + hash | ≤ 6 s | ≤ 4 s |
| — parse | ≤ 9 s | ≤ 0.2 s (cache hits) |
| — index | ≤ 2 s | ≤ 2 s |
| — analyse (all rules) | ≤ 2 s | ≤ 2 s |
| Reorder + re-resolve (no rescan) | ≤ 100 ms | ≤ 100 ms |
| Findings page query (200 rows) | ≤ 15 ms | ≤ 15 ms |
| Peak RSS | ≤ 900 MB | ≤ 600 MB |
| Steady RSS after scan | ≤ 500 MB | ≤ 500 MB |
| GUI cold start to interactive | ≤ 1 s | ≤ 1 s |
| Installer size (Windows) | ≤ 15 MB | — |

Warm enumeration barely improves because it is syscall-bound — `stat`ing 400k files
costs what it costs. That is why the warm target is 2 s and not 0.2 s, and it is the
next thing to optimise (via directory mtime short-circuiting) if 2 s proves too slow.

## Where the time goes, and the seven decisions that shape it

1. **Content-addressed parse cache.** The single largest win: warm parse drops from
   ~9 s to ~0.2 s. Keyed by content hash, so it survives mod updates and deduplicates
   vendored copies across mods. (§ `06`)
2. **Hash-only for assets.** ~75% of bytes in a typical playset are art. Classifying by
   profile glob and never opening them removes most of the IO from the parse stage.
3. **mmap + `&[u8]` lexing.** No `String` allocation per token; no UTF-8 validation of
   bytes that will be skipped anyway.
4. **Interning everything.** 400k paths as `String`s would cost ~50 MB and make every
   comparison a memcmp; as `u32`s, comparisons are register ops and hash maps get small.
5. **Map-reduce indexing.** No shared-map contention during the burstiest stage. (§ `04`)
6. **`SmallVec` inline storage.** Most paths and symbols have one or two contributors;
   inline storage avoids ~400k heap allocations per scan.
7. **Capability-gated index building.** Structures nobody asked for are never built —
   disabling reference-based rules saves both memory and parse time. (§ `08`)

## Non-goals

- Streaming/incremental results into the UI *during* a scan. A partial conflict list is
  misleading, since a later mod can change every verdict. Progress is a bar, not partial
  data.
- Sub-second cold scans. Not achievable while hashing 1.2 GB, and not needed.
- Running on a spinning disk in 20 s. Document it as slower and move on.

## Regression protection

- `criterion` benches for: lexer throughput (MB/s), fact extraction, index build,
  full-pipeline on a synthetic corpus.
- `smd-testkit` generates a deterministic synthetic 300-mod corpus from a seed, so CI
  benchmarks a realistic workload without shipping gigabytes of fixtures or anyone's
  copyrighted mod content.
- CI fails on >10% regression against the recorded baseline for any tracked bench.
- Peak RSS asserted in an integration test using the OS allocator's stats.

## Known scaling limits

| Dimension | Comfortable | Degrades at | Mitigation if reached |
| --- | --- | --- | --- |
| Mods | 400 | ~1000 | Overlay becomes the bottleneck; shard by path prefix |
| Files | 500k | ~2M | Switch enumeration to a persistent directory-mtime index |
| Symbols | 800k | ~3M | Move `DefinitionTable` to an arena-backed open-addressed map |
| Findings | 200k | ~1M | Already paginated; the report renderer needs streaming |

None of these are reachable with today's Stellaris workshop. They are recorded so the
first person to hit one knows the intended fix instead of rediscovering it.
