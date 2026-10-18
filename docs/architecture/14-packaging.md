# 14 — Packaging, distribution, and compatibility

The GUI artefacts below are **provisional**: the shell decision is re-taken at M7 against
a browser-first alternative (§ `15`, ADR-0001). The CLI artefacts are not provisional, and
this document is written so that the CLI row alone is a shippable product.

## Artefacts per release

| Artefact | Platform | Size (realistic) | Notes |
| --- | --- | --- | --- |
| `smd-cli-<target>` | all, incl. `x86_64-unknown-linux-musl` | ≤ 8 MB | **Static, zero runtime deps.** The compatibility floor |
| `smd-setup-x64.exe` (NSIS) | Windows 10 1809+ | ≤ 15 MB | WebView2 `downloadBootstrapper` — **needs internet at install time** |
| `smd-x64.msi` | Windows | ≤ 15 MB | Managed/enterprise installs; same WebView2 requirement |
| `smd_amd64.deb` / `.rpm` | Debian/Ubuntu, Fedora | ≤ 12 MB | Declares WebKitGTK 4.1 dep |
| `smd-x86_64.AppImage` | Linux, glibc ≥ build host | **80–110 MB** | Bundles WebKitGTK and its closure |
| `smd-aarch64.dmg` | macOS 14+ | ≤ 20 MB | Signed + notarised, or not shipped |
| `smd-x86_64.dmg` | macOS 14+ | ≤ 20 MB | Separate artefact, not Universal2 |
| `<name>-*.whl` | all | small | PyO3 binding, optional |

Three of those numbers were wrong in an earlier draft and are worth stating plainly,
because each was load-bearing for a decision:

- **AppImage is not 40 MB.** A Tauri AppImage carries WebKitGTK and its dependency
  closure; real ones land at 76–100 MB and up. Worse, an AppImage built against an older
  Ubuntu has known rendering failures on newer distributions — the very portability it is
  chosen for is the thing it does least reliably. So the AppImage is a convenience, and
  **the static musl CLI is the compatibility floor**, not the AppImage.
- **There is no Universal2 DMG at 20 MB.** A universal binary is both architectures in one
  file. Ship two DMGs; the download page picks by architecture.
- **The Windows installer trilemma.** "≤ 15 MB" *and* "bundles the WebView2 bootstrapper"
  *and* "no network" are three constraints and you may have two:

| WebView2 option | Adds to installer | Works with no internet |
| --- | --- | --- |
| `downloadBootstrapper` | ~0 MB | **No** — fetches the runtime during install |
| `embedBootstrapper` | ~1.8 MB | **No** — the bootstrapper still downloads the runtime |
| `offlineInstaller` | **~127 MB** | Yes |

  The choice is `downloadBootstrapper`, and the consequence is stated rather than buried:
  **installing the GUI on a Windows machine without WebView2 requires an internet
  connection at install time.** Nothing needs the network afterwards. A user who has
  neither WebView2 nor a connection gets the CLI, which is why it is the first row of the
  artefact table.

## Compatibility matrix

| Platform | Support | Known caveats |
| --- | --- | --- |
| Windows 11 / 10 21H2+ | Primary | None |
| Windows 10 1809–20H2 | Supported | WebView2 install may be needed; see below |
| Windows without WebView2, offline | **CLI only** | The bootstrapper cannot fetch the runtime. Documented on the download page, not discovered at the installer's error dialog |
| Windows 8.1 / 7 | Unsupported | No WebView2; CLI may work, untested |
| Ubuntu 22.04+ / Fedora 38+ | Supported | WebKitGTK 4.1 required |
| Steam Deck (SteamOS) | Supported via AppImage | Read-only rootfs; AppImage is the right shape |
| Other Linux | Best effort | CLI first, AppImage second — and the AppImage may fail to render on distributions newer than the build host |
| macOS 14+ | **CLI only until the Apple Developer account exists** | See § Code signing; the GUI cannot be shipped un-notarised |
| Proton/Wine Windows install on Linux | Supported | Prefix path probing in `smd-fs` (§ `05`) |

### WebView2, accurately

The Evergreen WebView2 Runtime is a **separate product from the Edge browser**: installing
Edge does not install it, and "everyone has Edge" is not an argument that anyone has
WebView2. It ships with Windows 11, and reached Windows 10 consumer devices through an
automatic rollout in **mid-2022** — not 2021, and not via Edge.

Windows 10 stays supported. Steam's June 2026 hardware survey still puts roughly 23.6% of
users there, and that is a fifth of the audience, not a rounding error. The practical
population needing the bootstrapper is small: Windows 10 machines that missed the rollout,
LTSC/N editions, and managed fleets.

## Update mechanism

Tauri's updater, checking a signed manifest on GitHub Releases — and if the shell decision
at M7 goes the other way, a version check that opens the download page, which is the same
constraints with less machinery. Constraints:

- **Off by default; opt in on first run.** A tool that phones home unasked is a tool
  people stop trusting.
- Signed with a key held only in CI secrets.
- Manual download always works — the updater is a convenience, not the only path.
- The CLI never self-updates. Package managers or a manual download.

## Code signing

Unsigned Windows binaries trigger SmartScreen, which reads to a non-technical user as
"this is malware". Options, in order of preference once the project has users:

1. Azure Trusted Signing / an EV certificate (~$200–400/yr) — removes the warning.
2. Reproducible builds plus published `sha256` and a signed provenance attestation
   (`actions/attest-build-provenance`) — free, verifiable, does not remove the warning.
3. For v0.x: option 2 plus a `README` section explaining the warning honestly.

### macOS: the workaround no longer exists

The familiar advice — "Control-click the app and choose Open" — **was removed in macOS 15
Sequoia.** A user who downloads an un-notarised app now gets a blocked dialog with no
bypass in it; recovering requires opening System Settings → Privacy & Security, finding
the blocked item, and clicking "Open Anyway", then re-launching and confirming again.

That is not a workaround a non-technical player will complete, and it reads exactly like
the malware warning it resembles. Two consequences, and neither is optional:

- The $99/yr Apple Developer account is a **hard prerequisite** for any macOS GUI release,
  not a nice-to-have. Signing and notarisation happen in CI or the DMG is not published.
- Until that account exists, **macOS ships CLI-only**, which is the stated default. A
  terminal binary needs no Gatekeeper bypass a user can fail at.

The paperwork has lead time (enrolment, and first-time notarisation issues surface late),
so it starts at M5 rather than at the release (§ `15`).

## What the app is allowed to touch

| Location | Access |
| --- | --- |
| Game install dir | **read only** |
| Mod dirs (local + workshop) | **read only** |
| Launcher DB | **read only** — snapshotted through SQLite (online backup over a read-only connection, or a verified double copy of `db` + `-wal`, never `-shm`), and read from the in-memory snapshot, § `05`. Never `immutable=1` or `nolock=1` on the live file; the main file and `-wal` are never written (gate H29) |
| OS cache dir (`%LOCALAPPDATA%`, `~/.cache`) | read/write — parse cache, stat cache |
| OS config dir | read/write — settings, suppressions |
| Private temp dir (`0700`) | read/write — launcher DB copy on the fallback path only, deleted on every exit path |
| User-chosen export path | write — reports only |
| Network | **none at runtime in v1** — including indirectly: cloud-placeholder files (OneDrive) are not hydrated, and no output loads remote content (§ `05`, § `11`). Two exceptions, both outside the running app: the Windows installer's WebView2 bootstrapper (install time), and the opt-in updater check |

This is enforced structurally: `smd-fs` exposes `read_only_root()` for game and mod
paths and there is no write API reachable from those handles. The same handle is the
*read* boundary (§ `05`, Trust boundaries): it is the only way to obtain bytes, it does not
follow links or reparse points inside the root, and it refuses a handle whose final path
leaves the root. There is no API that takes a bare path and returns content. A future patch-generation
feature must go through a new, explicitly-named module and an explicit user confirmation,
and must write only into a new mod directory it creates.

Nothing runs the game, launches Steam, or modifies the playset in v1.

## Privacy

- No telemetry, no analytics, no crash reporting to a server.
- **Redaction is on by default for every export** — HTML, JSON, Markdown, clipboard —
  with `--no-redact` for local use. Redaction replaces the home directory prefix with `~`
  and drops the account name and Steam account ids from every path, in every field —
  diagnostics and OS error text included — and is proven by a corpus test, not by review
  (§ `11`, Output safety; gate H26). The default is this way round because the
  formats users attach to a bug report are the file exports, not the clipboard summary;
  defaulting the machine-readable format to "unredacted" and the human one to "redacted"
  gets it backwards from how they are actually shared.
- **Crash dumps must contain neither mod content nor user paths.** A dump carries a rule
  id, a stage, a `CONTRACT_VERSION`, and a redacted backtrace. Anything else is content
  the user did not choose to share, in a file the bug-report flow encourages them to
  attach. Enforced by a test over the dump writer, not by intention.
- The "report a bug" flow opens a pre-filled GitHub issue that the **user** reviews and
  submits.
- **If workshop metadata fetching is ever added, it goes through the Steamworks Web API
  with a key, and never through scraping the store pages.** Pre-committed here because
  the decision is easy to make correctly now and awkward to reverse once a scraper works.
  Scraping breaches Valve's terms, breaks on markup changes, and puts the project's
  network behaviour outside anything a user can reason about.

## Licence, naming, and inbound source policy

These are one-way doors. They are recorded in the packaging document because they
determine the installer's product name, the certificate CN, the updater manifest URL, and
what may legally be in the binary at all.

### Outbound licence: dual **MIT OR Apache-2.0**

The Rust ecosystem norm, and stated now rather than "TBD before release". A repository
with no licence is a repository whose contributions arrive under no grant at all, and that
compounds with every contributor — relicensing later means tracking down everyone who ever
opened a PR. `LICENSE-MIT` and `LICENSE-APACHE` exist from the first commit, with the
standard `Licensed under either of … at your option` block in every `README` and
`Cargo.toml`.

### Inbound: how prior art may be read

`VERIFY.md` sends the developer to Irony Mod Manager and CWTools for the load-order and
merge-mode answers. That instruction is safe for documentation and dangerous for source:
if either project is copyleft, reading its implementation and writing equivalent Rust is a
contamination risk that no later relicensing repairs.

| Source | Policy |
| --- | --- |
| Wikis, documentation, issue threads, forum posts, release notes | **Read freely.** Facts about a game are not anyone's copyright |
| A project's source code | **Only after confirming its licence is compatible with MIT/Apache-2.0**, and record in the commit or in `VERIFY.md` which files were read |
| CWTools `.cwt` config files | Blocked on the **config repository's** own terms, which may differ from the engine's (§ `10`) |
| Anything under an unconfirmed licence | Do not open it. Ask a person instead |

Observed behaviour of the running game is never anyone's intellectual property, which is
the deeper reason M0.5 exists: the probe-mod route answers the same questions with no
provenance question attached.

### Product name — **OPEN, owner decision required**

"Stellaris Mod Doctor" places a third-party trademark in the product name of a signed
installer. It also fixes the repository name, the crates.io reservations, the updater
manifest URL, the installer's upgrade path (a rename orphans every existing install), and
the certificate CN. All are expensive after they exist and nearly free before.

Recommendation: **a coined name, with nominative use in the tagline** — "*<Name>*, a
conflict diagnostic for Paradox Interactive's Stellaris". Read Paradox's fan-content and
trademark terms before settling it. Until it is settled, `smd`/`<name>` placeholders stand
in, and nothing is registered, reserved, or signed under the current working title.

## Release process

Tag → CI builds all targets → attestations generated → GitHub Release drafted with
checksums and a generated changelog → manual review → publish. Version scheme:
`MAJOR.MINOR.PATCH`, with `CONTRACT_VERSION` (§ `03`) bumped independently and noted in
the changelog when caches will be invalidated.

None of that is free to build. Release engineering is a budgeted milestone-sized item
(8–12 days: signing, notarisation, the updater manifest, per-target CI, the download page),
not a task that happens on the afternoon of the tag — § `15`.
