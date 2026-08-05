# 14 — Packaging, distribution, and compatibility

## Artefacts per release

| Artefact | Platform | Size (target) | Notes |
| --- | --- | --- | --- |
| `smd-setup-x64.exe` (NSIS) | Windows 10 1803+ | ≤ 15 MB | Bundles WebView2 bootstrapper |
| `smd-x64.msi` | Windows | ≤ 15 MB | For managed/enterprise installs |
| `smd_amd64.deb` / `.rpm` | Debian/Ubuntu, Fedora | ≤ 12 MB | Declares WebKitGTK 4.1 dep |
| `smd-x86_64.AppImage` | Any Linux | ≤ 40 MB | For distros where the dep is awkward |
| `smd-universal.dmg` | macOS 11+ | ≤ 20 MB | Universal2, notarised |
| `smd-cli-<target>` | all, incl. `x86_64-unknown-linux-musl` | ≤ 8 MB | **Static, zero runtime deps** |
| `stellaris_mod_doctor-*.whl` | all | small | PyO3 binding, optional |

The static musl CLI is the compatibility floor: whatever happens with WebView runtimes
on an exotic distro, there is always a binary that runs.

## Compatibility matrix

| Platform | Support | Known caveats |
| --- | --- | --- |
| Windows 11 / 10 21H2+ | Primary | None |
| Windows 10 1803–20H2 | Supported | WebView2 may need the bundled bootstrapper |
| Windows 8.1 / 7 | Unsupported | No WebView2; CLI may work, untested |
| Ubuntu 22.04+ / Fedora 38+ | Supported | WebKitGTK 4.1 required |
| Steam Deck (SteamOS) | Supported via AppImage | Read-only rootfs; AppImage is the right shape |
| Other Linux | Best effort | AppImage or CLI |
| macOS 11+ | Supported | Stellaris runs here; workshop paths differ, handled in the profile |
| Proton/Wine Windows install on Linux | Supported | Prefix path probing in `smd-fs` (§ `05`) |

## Update mechanism

Tauri's updater, checking a signed manifest on GitHub Releases. Constraints:

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

macOS notarisation is mandatory in practice and requires a $99/yr Apple Developer
account; without it the `.dmg` cannot be opened without a right-click workaround. Budget
for it before the first macOS release, or ship macOS as CLI-only until then.

## What the app is allowed to touch

| Location | Access |
| --- | --- |
| Game install dir | **read only** |
| Mod dirs (local + workshop) | **read only** |
| Launcher DB | **read only** (copied to temp if locked) |
| OS cache dir (`%LOCALAPPDATA%`, `~/.cache`) | read/write — parse cache |
| OS config dir | read/write — settings, suppressions |
| User-chosen export path | write — reports only |
| Network | none in v1 |

This is enforced structurally: `smd-fs` exposes `read_only_root()` for game and mod
paths and there is no write API reachable from those handles. A future patch-generation
feature must go through a new, explicitly-named module and an explicit user confirmation,
and must write only into a new mod directory it creates.

Nothing runs the game, launches Steam, or modifies the playset in v1.

## Privacy

- No telemetry, no analytics, no crash reporting to a server.
- Crash dumps written locally, and the "report a bug" flow opens a pre-filled GitHub
  issue that the **user** reviews and submits.
- Exported reports contain mod names and file paths, which may include a Windows
  username. `--redact` replaces home directory prefixes with `~` and is **on by default
  for clipboard exports**, since those get pasted into public Discord channels.

## Release process

Tag → CI builds all targets → attestations generated → GitHub Release drafted with
checksums and a generated changelog → manual review → publish. Version scheme:
`MAJOR.MINOR.PATCH`, with `CONTRACT_VERSION` (§ `03`) bumped independently and noted in
the changelog when caches will be invalidated.
