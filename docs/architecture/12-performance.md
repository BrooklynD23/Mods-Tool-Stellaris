# 12 — Performance budget

Targets are stated up front so regressions are measurable rather than debatable. These
are **design targets on the reference machine**, not measurements — nothing is built
yet, and the workload below is an estimate. **No row here becomes a CI gate until the
reference workload has been measured** (§ "Reference workload"); gating on invented
numbers produces a green pipeline that certifies nothing.

## Reference workload

The workload must be **measured across three real 200–400 mod playsets** before M1, and
this section replaced with those figures plus the by-extension histogram they produce.
Until then the numbers below are a provisional shape, chosen to be internally consistent
rather than to be right.

Provisional playset: **300 mods, ~154k files, ~1.29 GB on disk**, of which **~409 MB in
88k files is parsed** and the remainder is hashed but never opened.

| Extension | Files | Mean | Bytes | Handling |
| --- | ---: | ---: | ---: | --- |
| `.txt` — script (`common/`, `events/`, `map/`) | 70,000 | 4.5 KB | 315 MB | parsed |
| `.yml` — localisation | 14,000 | 5.0 KB | 70 MB | parsed |
| `.gui`, `.gfx`, `.asset` | 4,000 | 6.0 KB | 24 MB | parsed |
| **Parsed subtotal** | **88,000** | **4.6 KB** | **409 MB** | |
| `.dds` | 58,000 | 9 KB | 522 MB | hashed |
| `.png` | 6,000 | 20 KB | 120 MB | hashed |
| `.mesh`, `.anim` | 1,500 | 90 KB | 135 MB | hashed |
| `.ogg`, `.wav` | 400 | 250 KB | 100 MB | hashed |
| **Hashed-only subtotal** | **65,900** | **13.3 KB** | **877 MB** | |
| **Total** | **153,900** | **8.4 KB** | **1.29 GB** | |

The mean parsed file is **4.6 KB**. That single number decides the read strategy, the
cache design, and the shape of the enumerate stage, which is why it is published rather
than implied: a file count and a byte total that only agree in aggregate can imply a
per-file size that is wrong by several times, and six decisions in this document inherit
it.

Derived counts used by the memory budget: ~500k definitions, ~800k localisation keys,
therefore **~1.3M interned symbols** — loc keys are symbols too.

**Machine:** 8-core desktop, NVMe SSD, 16 GB RAM, Windows 11.

## Time targets

| Stage | Cold (empty cache) | Warm (unchanged playset) |
| --- | ---: | ---: |
| Discover + load order | 0.3 s | 0.3 s |
| Enumerate + stat | 1.0 s | 1.0 s |
| Read + content hash | 9.0 s | — |
| Parse (cold) / cache read + decode (warm) | 4.0 s | 0.6 s |
| Index + resolve | 1.2 s | 1.2 s |
| Analyse (all rules) | 1.5 s | 1.5 s |
| Headroom | 3.0 s | 0.4 s |
| **Full scan, end to end** | **≤ 20 s** | **≤ 5 s** |

| Other operations | Target |
| --- | ---: |
| Reorder → new `live` set | ≤ 100 ms |
| Reorder → re-derived findings | ≤ 1.6 s |
| Findings page query (200 rows) | ≤ 15 ms |
| GUI cold start to interactive | ≤ 1 s |
| Installer size (Windows) | ≤ 15 MB |

Three things about that table are worth stating explicitly, because the previous version
of it was wrong in all three.

**The columns sum.** Every row is a budget, the rows add to the total, and the headroom
row is where variance lives rather than being hidden inside the stage estimates.

**Index and analyse cost the same warm as cold.** They consume facts; they do not care
where the facts came from. Any warm target below the sum of those two stages is
unreachable, which is what made the previous 2 s figure impossible before a single line
of code existed. **5 s is the honest number** for a warm full scan, and it is reached
only because of the stat cache described next.

**The warm column has no hashing row at all.** With a content-addressed cache and nothing
in front of it, a warm scan must re-read and re-hash all 1.29 GB purely to compute the
keys of a cache it is about to hit — the design would pay the entire cold IO cost to
avoid the parse cost, which is the smaller of the two. The stat-keyed layer
(`(volume_id, file_id, size, mtime_ns) → content_hash`, § `06-parsing.md`) is what makes
the row disappear: a warm scan stats and stops. On Windows the parallel directory walk
returns size and mtime as part of enumeration, so the layer costs no extra syscall.

## Where the time goes, and the decisions that shape it

1. **The stat cache in front of the content cache.** The largest single win, and the one
   that makes warm scanning viable at all: it removes 1.29 GB of reads and hashing from
   the normal path. (§ `06`)
2. **Content-addressed parse cache.** Warm parse drops from 4 s to 0.6 s, survives mod
   updates and path changes, and deduplicates files vendored across mods. It caches
   `RawFact` — string-based, file-local — never interned ids. (§ `06`)
3. **Hash-only for assets.** ~68% of bytes and ~43% of files are art and audio.
   Classifying by profile glob and never opening them removes most of the parse stage's
   IO.
4. **`read()` into pooled buffers, not mmap.** See below — this reverses an earlier
   decision and the reasons matter.
5. **Interning everything.** Holding 154k paths and 1.3M symbols as `String`s makes every
   comparison a memcmp and every map key a pointer chase; as `u32`s, comparisons are
   register ops and the maps shrink enough to keep their working set in cache. The
   savings are in the *references* to the strings, of which there are millions, not in
   the string bytes themselves.
6. **Map-reduce indexing.** No shared-map contention during the burstiest stage, at the
   cost of a merge transient the memory budget below accounts for. (§ `04`)
7. **`SmallVec` inline storage.** Most paths and symbols have one or two contributors;
   inline storage avoids ~150k heap allocations per scan.
8. **Capability-gated index building.** Structures nobody asked for are never built —
   gating reference extraction off saves 120 MB and a slice of the parse stage. (§ `08`)

### Why `read()` rather than mmap

Four reasons, any one of them sufficient:

- **Size.** mmap wins only when the cost of establishing and tearing down a mapping
  amortises over a large file. The parsed-file mean here is 4.6 KB. At that size the page
  faults and TLB work dominate and a `read()` into an already-warm, reused buffer wins
  outright.
- **Windows contention.** `MapViewOfFile`/`UnmapViewOfFile` serialise on a process-wide
  VAD lock. 88,000 map/unmap pairs across 8 threads convert a parallel stage into a
  partly serial one — the opposite of what the mapping was for.
- **Windows locks mapped files.** A Steam workshop update landing mid-scan then fails
  against the user's own game install. The tool's read-only promise is not just about not
  writing; mmap breaks it at the OS level, against the one asset the user cares about
  most.
- **A page fault on a changed file is uncatchable.** If the file backing a mapping is
  truncated or rewritten underneath it, the fault raises `SIGBUS` on Unix and
  `EXCEPTION_IN_PAGE_ERROR` on Windows. `catch_unwind` catches neither. The single most
  likely disruption during a scan — a file changing while the scan runs — becomes a hard
  crash instead of a diagnostic.

mmap remains permitted under **both** of two conditions together: the file exceeds a
threshold established by measurement rather than by guess, and it lives under the game
install directory, which is read-only to this tool and not modified by Steam except
during an update the tool can detect. Anything else is `read()`.

Windows opens use `FILE_FLAG_SEQUENTIAL_SCAN`. The read buffer is per-thread and reused,
so the parse stage performs no allocation for file bodies regardless of file size.

### Antivirus is part of the budget

Windows Defender's filter driver inspects the **first** read of every file. On a
154k-file cold scan that is plausibly the largest single term in the 9 s read-and-hash
budget — larger than the disk, larger than blake3, and invisible in any benchmark run on
Linux.

Two consequences. The README recommends an exclusion for the workshop and mod
directories, with an explanation of what is being excluded and why, because that is a
real and legitimate several-fold speedup available to every user. And the benchmark of
record runs **with Defender enabled**, because that is the configuration users actually
have; the excluded number is reported alongside it as an upper bound, never instead of
it.

## Memory budget

Two numbers, budgeted separately, because they are measured by different mechanisms and
one of them is not visible to the other.

**Engine heap**, at steady state after a scan:

| Component | With references | Without (D2 gates them off) |
| --- | ---: | ---: |
| Path interner (~154k paths) | 20 MB | 20 MB |
| Symbol interner (~1.3M entries) | 115 MB | 115 MB |
| OverlayTable | 40 MB | 40 MB |
| DefinitionTable | 70 MB | 70 MB |
| LocTable | 90 MB | 90 MB |
| ReferenceTable | 120 MB | — |
| Finding vector (~200k findings) | 50 MB | 40 MB |
| **Steady engine heap** | **505 MB** | **375 MB** |

Peak adds what exists only during a scan:

| Component | With references | Without |
| --- | ---: | ---: |
| Steady (above) | 505 MB | 375 MB |
| Merge transient — shard maps live until the merge completes | 200 MB | 150 MB |
| Parse arenas and read buffers (8 × 8 MB) | 64 MB | 64 MB |
| **Peak engine heap** | **769 MB** | **589 MB** |

**Process-tree RSS**, which is what the operating system, the user, and the OOM killer
see:

| Component | With references | Without |
| --- | ---: | ---: |
| Peak engine heap | 769 MB | 589 MB |
| Allocator overhead and fragmentation (~10%) | 77 MB | 59 MB |
| Binary, static data, thread stacks | 25 MB | 25 MB |
| **CLI peak** | **871 MB** | **673 MB** |
| WebView process tree (browser + GPU + renderer + JS heap) | 350 MB | 350 MB |
| **GUI peak** | **1221 MB** | **1023 MB** |

Targets, rounded up from those sums:

| | With references | Without |
| --- | ---: | ---: |
| Peak engine heap | ≤ 800 MB | ≤ 620 MB |
| Peak process-tree RSS, CLI | ≤ 900 MB | ≤ 700 MB |
| Peak process-tree RSS, GUI | ≤ 1.3 GB | ≤ 1.1 GB |
| Steady process-tree RSS after scan, GUI | ≤ 1.0 GB | ≤ 0.8 GB |

The gap between the two columns is the real cost of decision D2. Gating reference
extraction off for v1 is worth ~200 MB of peak, and it is the difference between a
comfortable figure and one that needs defending on a 16 GB machine with a game running.

**Measurement.** The two numbers need two mechanisms, and conflating them is how the
previous budget under-counted. Engine heap comes from an allocator hook. Process-tree RSS
comes from the OS — `/proc/self/smaps_rollup` on Linux, `GetProcessMemoryInfo` over the
job object on Windows, `proc_pid_rusage` on macOS — walking children, because allocator
statistics cannot see mmap'd pages, SQLite's page cache, or any of the WebView's
processes. Two assertions in the integration test, not one.

**Allocator.** The Windows system allocator is the weakest of the three platforms for
this allocation shape: many small, short-lived, multi-threaded allocations with a burst
free at the end of indexing. `mimalloc` is a one-line change and is measured at M6;
adopt it if it wins on peak RSS or merge time, noting that it also changes which API
reports the heap number above.

## What "reorder in 100 ms" covers

Changing the load order does not re-score findings, it **re-derives** them. A new rank
vector changes which definition is `live`, which changes participants, which changes
whether a finding exists at all. That is the analyse stage's work, and it cannot be
100 ms and 1.5 s simultaneously.

So the 100 ms budget is scoped to exactly one thing: **re-resolving `live`**. Given a new
rank vector, walk the overlay and definition tables, recompute the winner per key, and
publish the new resolution set. No reparse, no re-index, no rule execution.

Everything downstream is slower and the UI must say so rather than imply otherwise. On a
reorder the findings table greys and holds its stale row count while re-derivation runs
at the analyse budget (hence the 1.6 s figure in the target table); facet counts grey
with it. A `ScanEpoch` bump invalidates in-flight queries so the virtualiser cannot
splice two orderings into one scroll position.

This path depends on `rank` being separable from source identity. Until that lands, the
reorder sandbox is not implementable as specified and should not be scheduled.

## Non-goals

- Streaming/incremental results into the UI *during* a scan. A partial conflict list is
  misleading, since a later mod can change every verdict. Progress is a bar, not partial
  data.
- Sub-second cold scans. Not achievable while hashing 1.29 GB, and not needed.
- Running on a spinning disk in 20 s. Document it as slower and move on.

## Regression protection

Benches are split in two, because a single suite cannot serve both purposes and the
combined one would be disabled within a month.

| Suite | Runs on | Contains | Gate |
| --- | --- | --- | --- |
| `bench-cpu` | Linux CI | Lexer throughput, fact extraction, index build, resolver, rule engine — all over in-memory fixtures, no filesystem access | **Fails at >10% regression** against the recorded baseline |
| `bench-io` | Windows runner | Full pipeline over the generated corpus: enumerate, read + hash, cold and warm cache, end to end | **Trend-reported.** Alerts on >2× or on sustained multi-run drift. No hard gate |

The split follows from two facts. Every platform-sensitive decision in this document —
mmap, `FILE_FLAG_SEQUENTIAL_SCAN`, the stat cache's zero-syscall claim, Defender — is
budgeted on Windows, so benchmarking them on Linux measures a different operating
system's filesystem and reports it as this one's. And a 10% gate on an IO-bound benchmark
running on a shared cloud runner is red for reasons that have nothing to do with the
commit; the first month of that teaches everyone to ignore bench failures, which costs
the CPU gate its credibility too.

Also:

- `smd-testkit` generates a deterministic synthetic corpus from a seed, matched to the
  measured by-extension histogram, so CI benchmarks a realistic workload without shipping
  gigabytes of fixtures or anyone's copyrighted mod content. The generator is regenerated
  when the measured workload lands.
- Peak engine heap and peak process-tree RSS are asserted in an integration test, by the
  two mechanisms described above.

## Known scaling limits

| Dimension | Comfortable | Degrades at | Mitigation if reached |
| --- | --- | --- | --- |
| Mods | 400 | ~1000 | Overlay becomes the bottleneck; shard by path prefix |
| Files | 200k | ~1M | Switch enumeration to a persistent directory-mtime index |
| Symbols | 1.5M | ~4M | Move `DefinitionTable` to an arena-backed open-addressed map |
| Findings | 200k | ~1M | Already paginated; the report renderer needs streaming |

None of these are reachable with today's Stellaris workshop. They are recorded so the
first person to hit one knows the intended fix instead of rediscovering it.
