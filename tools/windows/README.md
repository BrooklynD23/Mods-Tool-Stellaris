# Running Mod Doctor on Windows

Double-click `ModDoctor.cmd`, or from PowerShell:

```powershell
.\tools\windows\ModDoctor.cmd                         # interactive menu
.\tools\windows\ModDoctor.cmd -Action Scan -Format md # one-off scan, Markdown on screen
.\tools\windows\ModDoctor.cmd -Action Report -Format json -Yes
```

The `.cmd` wrapper starts Windows PowerShell with `-ExecutionPolicy Bypass` for this one
script, so no policy change is needed. If Windows says the file was downloaded from the
internet, run `Unblock-File .\tools\windows\ModDoctor.ps1` once.

## What it finds

| Item | Where it looks |
| --- | --- |
| Game | Steam (registry + every library in `libraryfolders.vdf`, via `appmanifest_281990.acf`), then GOG / installer uninstall entries, then `X:\XboxGames\Stellaris\Content` |
| User data | `Documents\Paradox Interactive\Stellaris`, following OneDrive folder redirection |
| Workshop | `steamapps\workshop\content\281990` in the library that holds the game |
| Launcher DB, last launch, local mods, saves, error log | Inside the user data folder |

Detected paths can be overridden with menu option 9 (saved to
`%LOCALAPPDATA%\StellarisModDoctor\paths.json`) or with `-GameDir`, `-UserDir` and
`-WorkshopDir`. Arguments beat saved values, and saved values beat detection.

## Dependencies

The first scan builds `smd.exe` from this repository. That needs:

- **Rust** (rustup, stable). Installed with `winget install Rustlang.Rustup`, or from
  rust-lang.org when winget is missing.
- **Visual Studio 2022 Build Tools, C++ workload**. Rust's MSVC toolchain links with it,
  and the bundled SQLite is compiled with it. This is a large download and asks for
  administrator approval.

The script asks before installing either one (`-Yes` skips the question). It rebuilds
`smd.exe` automatically when the sources are newer than the binary.

## What it never does

It does not write to the game, the launcher database, your mods or your saves. The scan
reads a verified snapshot of `launcher-v2.sqlite` (`docs/architecture/05-ingest.md`).
Reports go to `Documents\StellarisModDoctor` and redact your home folder and account
name unless you pass `-NoRedact`.

## Limits of this release

The first release (M1) reports same-path file conflicts only, without severity scores.
The Windows file-safety checks (junctions, reparse points, OneDrive placeholders) are
not yet built (gate H22), so files behind those are not treated specially yet.
