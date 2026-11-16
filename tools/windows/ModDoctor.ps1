<#
.SYNOPSIS
    Stellaris Mod Doctor for Windows: finds Stellaris, your mods and saves, sets up the
    build tools, builds `smd`, and runs a read-only conflict scan.

.DESCRIPTION
    Run with no arguments for an interactive menu. Every action is read-only with respect
    to the game, the launcher, your mods and your saves: the scan reads a verified
    snapshot of the launcher database and never writes to it (docs/architecture/05-ingest.md).

    The only things this script installs, and only after asking, are the Rust toolchain
    (rustup) and the Visual Studio C++ Build Tools that Rust needs on Windows.

.EXAMPLE
    .\ModDoctor.cmd
    Opens the menu.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\ModDoctor.ps1 -Action Scan -Format md
    Detects everything, builds if needed, and prints a Markdown report.

.EXAMPLE
    .\ModDoctor.ps1 -Action Report -Format json -Yes
    Saves a JSON report under Documents\StellarisModDoctor without prompting.
#>
[CmdletBinding()]
param(
    [ValidateSet('Menu', 'Detect', 'Mods', 'Saves', 'Scan', 'Report', 'ErrorLog', 'Setup', 'Build')]
    [string]$Action = 'Menu',
    [string]$GameDir,
    [string]$UserDir,
    [string]$WorkshopDir,
    [ValidateSet('table', 'json', 'md')]
    [string]$Format = 'table',
    [string]$OutFile,
    [switch]$NoRedact,
    [switch]$Rebuild,
    [switch]$Yes
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

$StellarisAppId = '281990'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$ConfigDir = Join-Path $env:LOCALAPPDATA 'StellarisModDoctor'
$ConfigFile = Join-Path $ConfigDir 'paths.json'
$CargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
$ExePath = Join-Path $RepoRoot 'target\release\smd.exe'

# ---------------------------------------------------------------- output helpers

function Write-Title([string]$Text) {
    Write-Host ''
    Write-Host $Text -ForegroundColor Cyan
    Write-Host ('-' * $Text.Length) -ForegroundColor DarkCyan
}
function Write-Ok([string]$Text) { Write-Host "  [ok]   $Text" -ForegroundColor Green }
function Write-Warn2([string]$Text) { Write-Host "  [warn] $Text" -ForegroundColor Yellow }
function Write-Bad([string]$Text) { Write-Host "  [miss] $Text" -ForegroundColor Red }
function Write-Info([string]$Text) { Write-Host "         $Text" -ForegroundColor Gray }

function Confirm-Step([string]$Question) {
    if ($Yes) { return $true }
    $answer = Read-Host "$Question [y/N]"
    return ($answer -match '^(y|yes)$')
}

function Format-Size([long]$Bytes) {
    if ($Bytes -ge 1GB) { return '{0:N1} GB' -f ($Bytes / 1GB) }
    if ($Bytes -ge 1MB) { return '{0:N1} MB' -f ($Bytes / 1MB) }
    if ($Bytes -ge 1KB) { return '{0:N0} KB' -f ($Bytes / 1KB) }
    return "$Bytes B"
}

function Get-RegValue([string]$Key, [string]$Name) {
    try {
        $item = Get-ItemProperty -Path $Key -Name $Name -ErrorAction Stop
        return $item.$Name
    } catch {
        return $null
    }
}

# ---------------------------------------------------------------- saved overrides

function Get-SavedPaths {
    if (Test-Path $ConfigFile) {
        try { return Get-Content -Encoding UTF8 $ConfigFile -Raw | ConvertFrom-Json } catch { Write-Warn2 "Ignoring unreadable $ConfigFile" }
    }
    return $null
}

function Save-Paths($Paths) {
    if (-not (Test-Path $ConfigDir)) { New-Item -ItemType Directory -Path $ConfigDir | Out-Null }
    [pscustomobject]@{
        GameDir     = $Paths.GameDir
        UserDir     = $Paths.UserDir
        WorkshopDir = $Paths.WorkshopDir
    } | ConvertTo-Json | Set-Content -Path $ConfigFile -Encoding UTF8
}

# ---------------------------------------------------------------- detection

function Get-SteamRoot {
    $candidates = @(
        (Get-RegValue 'HKCU:\Software\Valve\Steam' 'SteamPath'),
        (Get-RegValue 'HKLM:\SOFTWARE\WOW6432Node\Valve\Steam' 'InstallPath'),
        (Get-RegValue 'HKLM:\SOFTWARE\Valve\Steam' 'InstallPath'),
        (Join-Path ${env:ProgramFiles(x86)} 'Steam')
    )
    foreach ($c in $candidates) {
        if ($c -and (Test-Path (Join-Path $c 'steamapps'))) { return (Resolve-Path $c).Path }
    }
    return $null
}

function Get-SteamLibraries([string]$SteamRoot) {
    $libs = New-Object System.Collections.Generic.List[string]
    if (-not $SteamRoot) { return $libs }
    $libs.Add($SteamRoot)
    $vdf = Join-Path $SteamRoot 'steamapps\libraryfolders.vdf'
    if (Test-Path $vdf) {
        foreach ($m in [regex]::Matches((Get-Content -Encoding UTF8 $vdf -Raw), '"path"\s+"([^"]+)"')) {
            $p = $m.Groups[1].Value -replace '\\\\', '\'
            if ((Test-Path $p) -and -not ($libs -contains $p)) { $libs.Add($p) }
        }
    }
    return $libs
}

function Find-SteamInstall([string]$SteamRoot) {
    foreach ($lib in (Get-SteamLibraries $SteamRoot)) {
        $manifest = Join-Path $lib "steamapps\appmanifest_$StellarisAppId.acf"
        if (-not (Test-Path $manifest)) { continue }
        $installDir = 'Stellaris'
        $m = [regex]::Match((Get-Content -Encoding UTF8 $manifest -Raw), '"installdir"\s+"([^"]+)"')
        if ($m.Success) { $installDir = $m.Groups[1].Value }
        $game = Join-Path $lib "steamapps\common\$installDir"
        if (Test-Path $game) {
            return [pscustomobject]@{
                Store       = 'Steam'
                GameDir     = $game
                WorkshopDir = (Join-Path $lib "steamapps\workshop\content\$StellarisAppId")
            }
        }
    }
    return $null
}

function Find-OtherInstall {
    # GOG Galaxy and plain installers register an uninstall entry; Game Pass installs under XboxGames.
    $uninstallKeys = @(
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
        'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
        'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
    )
    foreach ($key in $uninstallKeys) {
        $entries = Get-ItemProperty $key -ErrorAction SilentlyContinue |
            Where-Object { $_.PSObject.Properties['DisplayName'] -and $_.DisplayName -like 'Stellaris*' -and $_.PSObject.Properties['InstallLocation'] -and $_.InstallLocation }
        foreach ($e in $entries) {
            if (Test-Path (Join-Path $e.InstallLocation 'stellaris.exe')) {
                $store = 'Other'
                if ($e.PSObject.Properties['Publisher'] -and $e.Publisher -like '*GOG*') { $store = 'GOG' }
                return [pscustomobject]@{ Store = $store; GameDir = $e.InstallLocation.TrimEnd('\'); WorkshopDir = $null }
            }
        }
    }
    foreach ($drive in (Get-PSDrive -PSProvider FileSystem -ErrorAction SilentlyContinue)) {
        $p = Join-Path $drive.Root 'XboxGames\Stellaris\Content'
        if (Test-Path $p) { return [pscustomobject]@{ Store = 'Xbox / Game Pass'; GameDir = $p; WorkshopDir = $null } }
    }
    return $null
}

function Find-UserDir {
    $docs = [Environment]::GetFolderPath('MyDocuments')   # follows OneDrive folder redirection
    $candidates = @(
        (Join-Path $docs 'Paradox Interactive\Stellaris'),
        (Join-Path $env:USERPROFILE 'Documents\Paradox Interactive\Stellaris'),
        (Join-Path $env:USERPROFILE 'OneDrive\Documents\Paradox Interactive\Stellaris')
    )
    foreach ($c in $candidates) { if (Test-Path $c) { return $c } }
    return $null
}

function Get-GameVersion([string]$Game) {
    if (-not $Game) { return $null }
    $settings = Join-Path $Game 'launcher-settings.json'
    if (-not (Test-Path $settings)) { return $null }
    try {
        $j = Get-Content -Encoding UTF8 $settings -Raw | ConvertFrom-Json
        foreach ($name in 'rawVersion', 'version') {
            if ($j.PSObject.Properties[$name] -and $j.$name) { return [string]$j.$name }
        }
    } catch { }
    return $null
}

function Resolve-Paths {
    $saved = Get-SavedPaths
    $r = [ordered]@{ Store = $null; GameDir = $null; UserDir = $null; WorkshopDir = $null; Version = $null; Source = @{} }

    $found = Find-SteamInstall (Get-SteamRoot)
    if (-not $found) { $found = Find-OtherInstall }
    if ($found) {
        $r.Store = $found.Store
        $r.GameDir = $found.GameDir; $r.Source.GameDir = 'detected'
        if ($found.WorkshopDir) { $r.WorkshopDir = $found.WorkshopDir; $r.Source.WorkshopDir = 'detected' }
    }
    $u = Find-UserDir
    if ($u) { $r.UserDir = $u; $r.Source.UserDir = 'detected' }

    # Saved choices beat detection; command-line arguments beat both.
    foreach ($name in 'GameDir', 'UserDir', 'WorkshopDir') {
        if ($saved -and $saved.PSObject.Properties[$name] -and $saved.$name) { $r[$name] = $saved.$name; $r.Source[$name] = 'saved' }
        $arg = Get-Variable -Name $name -ValueOnly
        if ($arg) { $r[$name] = $arg; $r.Source[$name] = 'argument' }
    }
    $r.Version = Get-GameVersion $r.GameDir
    return [pscustomobject]$r
}

function Show-PathLine([string]$Label, [string]$Path, [string]$Source, [switch]$Optional) {
    $tag = ''
    if ($Source) { $tag = " ($Source)" }
    if ($Path -and (Test-Path $Path)) { Write-Ok ("{0,-16}{1}{2}" -f $Label, $Path, $tag) }
    elseif ($Path) { Write-Bad ("{0,-16}{1}{2} - not found" -f $Label, $Path, $tag) }
    elseif ($Optional) { Write-Warn2 ("{0,-16}not found" -f $Label) }
    else { Write-Bad ("{0,-16}not found" -f $Label) }
}

function Show-Detection($P) {
    Write-Title 'Stellaris installation'
    if ($P.Store) { Write-Info "Store: $($P.Store)" }
    if ($P.Version) { Write-Info "Game version: $($P.Version)" }
    Show-PathLine 'Game' $P.GameDir $P.Source['GameDir']
    Show-PathLine 'User data' $P.UserDir $P.Source['UserDir']
    Show-PathLine 'Workshop' $P.WorkshopDir $P.Source['WorkshopDir'] -Optional

    if ($P.UserDir) {
        Write-Title 'Inside your user data folder'
        Show-PathLine 'Launcher DB' (Join-Path $P.UserDir 'launcher-v2.sqlite') '' -Optional
        Show-PathLine 'Last launch' (Join-Path $P.UserDir 'dlc_load.json') '' -Optional
        Show-PathLine 'Local mods' (Join-Path $P.UserDir 'mod') '' -Optional
        Show-PathLine 'Saves' (Join-Path $P.UserDir 'save games') '' -Optional
        Show-PathLine 'Error log' (Join-Path $P.UserDir 'logs\error.log') '' -Optional
        if ($P.UserDir -like '*OneDrive*') {
            Write-Warn2 'Your Documents folder is synced by OneDrive. Files that are online-only are reported as unread, never downloaded.'
        }
    }
    $running = Get-Process -Name 'stellaris', 'Paradox Launcher' -ErrorAction SilentlyContinue
    if ($running) {
        Write-Warn2 'Stellaris or the Paradox Launcher is running. Scanning is still safe (it reads a verified snapshot), but reorder in the launcher before scanning, not during.'
    }
}

# ---------------------------------------------------------------- mods and saves

function Read-DescriptorField([string]$Text, [string]$Field) {
    $m = [regex]::Match($Text, '(?m)^\s*' + $Field + '\s*=\s*"([^"]*)"')
    if ($m.Success) { return $m.Groups[1].Value }
    return $null
}

function Get-LastLaunchEnabled([string]$User) {
    $set = @{}
    $file = Join-Path $User 'dlc_load.json'
    if (Test-Path $file) {
        try {
            $j = Get-Content -Encoding UTF8 $file -Raw | ConvertFrom-Json
            if ($j.PSObject.Properties['enabled_mods']) { foreach ($m in $j.enabled_mods) { $set[[string]$m] = $true } }
        } catch { Write-Warn2 'dlc_load.json could not be read.' }
    }
    return $set
}

function Show-Mods($P) {
    Write-Title 'Mods'
    if (-not $P.UserDir) { Write-Bad 'No user data folder, so no launcher mod list.'; return }
    $enabled = Get-LastLaunchEnabled $P.UserDir
    $rows = @()
    $modDir = Join-Path $P.UserDir 'mod'
    if (Test-Path $modDir) {
        foreach ($f in (Get-ChildItem $modDir -Filter '*.mod' -File -ErrorAction SilentlyContinue)) {
            $text = Get-Content -Encoding UTF8 $f.FullName -Raw -ErrorAction SilentlyContinue
            if ($null -eq $text) { $text = '' }
            $rows += [pscustomobject]@{
                Name      = (Read-DescriptorField $text 'name')
                Workshop  = (Read-DescriptorField $text 'remote_file_id')
                Supported = (Read-DescriptorField $text 'supported_version')
                LastLaunch = $(if ($enabled.ContainsKey("mod/$($f.Name)")) { 'enabled' } else { '' })
                File      = $f.Name
            }
        }
    }
    if ($rows.Count -eq 0) { Write-Warn2 'No launcher .mod files found.'; return }
    $rows | Sort-Object Name | Format-Table -AutoSize Name, Workshop, Supported, LastLaunch | Out-Host
    $on = @($rows | Where-Object { $_.LastLaunch }).Count
    Write-Info "$($rows.Count) mods registered with the launcher; $on enabled at the last launch (dlc_load.json)."
    Write-Info 'The launcher database is the authority on the current playset; the scan reads it.'
    if ($P.WorkshopDir -and (Test-Path $P.WorkshopDir)) {
        $ws = @(Get-ChildItem $P.WorkshopDir -Directory -ErrorAction SilentlyContinue).Count
        Write-Info "$ws subscribed Workshop items on disk."
    }
}

function Show-Saves($P) {
    Write-Title 'Save games'
    $dir = $null
    if ($P.UserDir) { $dir = Join-Path $P.UserDir 'save games' }
    if (-not $dir -or -not (Test-Path $dir)) { Write-Bad 'No save games folder found.'; return }
    $rows = foreach ($empire in (Get-ChildItem $dir -Directory -ErrorAction SilentlyContinue)) {
        $saves = @(Get-ChildItem $empire.FullName -Filter '*.sav' -File -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending)
        if ($saves.Count -eq 0) { continue }
        [pscustomobject]@{
            Empire   = $empire.Name
            Saves    = $saves.Count
            Latest   = $saves[0].Name
            Modified = $saves[0].LastWriteTime.ToString('yyyy-MM-dd HH:mm')
            Size     = (Format-Size (($saves | Measure-Object Length -Sum).Sum))
        }
    }
    $rows = @($rows)
    if ($rows.Count -eq 0) { Write-Warn2 'The folder exists but holds no .sav files.'; return }
    $rows | Sort-Object Modified -Descending | Format-Table -AutoSize | Out-Host
    Write-Info 'Save files are only listed; they are never opened or changed.'
}

function Show-ErrorLog($P) {
    Write-Title 'Last game error log'
    $log = $null
    if ($P.UserDir) { $log = Join-Path $P.UserDir 'logs\error.log' }
    if (-not $log -or -not (Test-Path $log)) { Write-Warn2 'No logs\error.log yet. Start the game once to create it.'; return }
    $item = Get-Item $log
    Write-Info "$log  (written $($item.LastWriteTime.ToString('yyyy-MM-dd HH:mm')), $(Format-Size $item.Length))"
    Get-Content -Encoding UTF8 $log -Tail 40 | Out-Host
}

# ---------------------------------------------------------------- dependencies

function Update-SessionPath {
    $machine = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    $user = [Environment]::GetEnvironmentVariable('Path', 'User')
    $env:Path = "$machine;$user"
    if ((Test-Path $CargoBin) -and ($env:Path -notlike "*$CargoBin*")) { $env:Path = "$CargoBin;$env:Path" }
}

function Test-Cargo { return [bool](Get-Command cargo -ErrorAction SilentlyContinue) }

function Test-MsvcTools {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) { return $false }
    $found = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    return [bool]$found
}

function Test-Winget { return [bool](Get-Command winget -ErrorAction SilentlyContinue) }

function Install-MsvcTools {
    Write-Info 'Installing Visual Studio 2022 Build Tools (C++ workload). This is a large download and asks for administrator approval.'
    $override = '--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
    if (Test-Winget) {
        & winget install --id Microsoft.VisualStudio.2022.BuildTools -e --source winget --accept-package-agreements --accept-source-agreements --override $override
    } else {
        $installer = Join-Path $env:TEMP 'vs_BuildTools.exe'
        Invoke-WebRequest -Uri 'https://aka.ms/vs/17/release/vs_BuildTools.exe' -OutFile $installer -UseBasicParsing
        Start-Process -FilePath $installer -ArgumentList $override -Verb RunAs -Wait
    }
}

function Install-Rustup {
    Write-Info 'Installing Rust (rustup, stable, MSVC host).'
    if (Test-Winget) {
        & winget install --id Rustlang.Rustup -e --source winget --accept-package-agreements --accept-source-agreements
    } else {
        $init = Join-Path $env:TEMP 'rustup-init.exe'
        Invoke-WebRequest -Uri 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe' -OutFile $init -UseBasicParsing
        & $init -y --default-toolchain stable --profile minimal
    }
    Update-SessionPath
    if (Test-Cargo) { & rustup default stable | Out-Host }
}

function Invoke-Setup {
    Write-Title 'Dependencies'
    Update-SessionPath
    $needMsvc = -not (Test-MsvcTools)
    $needRust = -not (Test-Cargo)
    if ($needMsvc) { Write-Bad 'Visual Studio C++ Build Tools (needed to link Rust programs and build the bundled SQLite)' } else { Write-Ok 'Visual Studio C++ Build Tools' }
    if ($needRust) { Write-Bad 'Rust toolchain (cargo)' } else { Write-Ok ('Rust toolchain: ' + (& cargo --version)) }
    if (-not ($needMsvc -or $needRust)) { return $true }
    if (-not (Test-Winget)) { Write-Warn2 'winget is not available; installers will be downloaded directly from Microsoft and rust-lang.org.' }
    if (-not (Confirm-Step 'Install the missing tools now?')) {
        Write-Warn2 'Skipped. Install them yourself, or run: .\ModDoctor.cmd -Action Setup'
        return $false
    }
    if ($needMsvc) { Install-MsvcTools }
    if ($needRust) { Install-Rustup }
    Update-SessionPath
    $ok = (Test-MsvcTools) -and (Test-Cargo)
    if ($ok) { Write-Ok 'All dependencies are installed.' }
    else { Write-Bad 'Something is still missing. Open a new PowerShell window and run Setup again; installers sometimes need a fresh session or a reboot.' }
    return $ok
}

# ---------------------------------------------------------------- build

function Test-BuildStale {
    if (-not (Test-Path $ExePath)) { return $true }
    $built = (Get-Item $ExePath).LastWriteTime
    $newest = Get-ChildItem (Join-Path $RepoRoot 'crates'), (Join-Path $RepoRoot 'profiles') -Recurse -File -Include '*.rs', '*.toml' -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    return ($newest -and $newest.LastWriteTime -gt $built)
}

function Invoke-Build([switch]$Force) {
    if (-not $Force -and -not (Test-BuildStale)) { return $true }
    if (-not (Invoke-Setup)) { return $false }
    Write-Title 'Building smd (release)'
    Write-Info 'The first build downloads crates and takes a few minutes; later builds are quick.'
    Push-Location $RepoRoot
    try {
        & cargo build --release --locked -p smd-cli
        $code = $LASTEXITCODE
    } finally {
        Pop-Location
    }
    if ($code -ne 0 -or -not (Test-Path $ExePath)) {
        Write-Bad "Build failed (cargo exit code $code). The output above says why."
        return $false
    }
    Write-Ok "Built $ExePath"
    return $true
}

# ---------------------------------------------------------------- scan

function Get-ScanArgs($P, [string]$Fmt) {
    $a = @('scan', '--game-dir', $P.GameDir, '--user-dir', $P.UserDir, '--format', $Fmt)
    if ($P.WorkshopDir -and (Test-Path $P.WorkshopDir)) { $a += @('--workshop-dir', $P.WorkshopDir) }
    if ($NoRedact) { $a += '--no-redact' }
    return $a
}

function Test-ScanReady($P) {
    $missing = @()
    if (-not $P.GameDir -or -not (Test-Path $P.GameDir)) { $missing += 'game folder' }
    if (-not $P.UserDir -or -not (Test-Path $P.UserDir)) { $missing += 'user data folder' }
    if ($missing.Count) {
        Write-Bad ("Cannot scan without the " + ($missing -join ' and ') + '. Use "Set custom paths" or pass -GameDir / -UserDir.')
        return $false
    }
    return $true
}

function Write-ExitMeaning([int]$Code) {
    switch ($Code) {
        0 { Write-Ok 'Scan finished.' }
        1 { Write-Bad 'The scan could not run. The message above says why.' }
        2 { Write-Bad 'smd rejected the command line (this is a bug in this script; please report it).' }
        3 { Write-Warn2 'Scan finished, but reported an Error-class diagnostic. Check the completeness section: some files were not read.' }
        default { Write-Bad "smd exited with code $Code." }
    }
}

function Invoke-Scan($P, [string]$Fmt) {
    if (-not (Test-ScanReady $P)) { return }
    if (-not (Invoke-Build -Force:$Rebuild)) { return }
    Write-Title "Scan ($Fmt)"
    & $ExePath @(Get-ScanArgs $P $Fmt)
    Write-ExitMeaning $LASTEXITCODE
    Write-Info 'Severity scores are not part of this release; findings are listed with how certain each one is.'
}

function Invoke-Report($P, [string]$Fmt) {
    if ($Fmt -eq 'table') { $Fmt = 'md' }
    if (-not (Test-ScanReady $P)) { return }
    if (-not (Invoke-Build -Force:$Rebuild)) { return }
    $target = $OutFile
    if (-not $target) {
        $dir = Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'StellarisModDoctor'
        if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
        $target = Join-Path $dir ('report-{0}.{1}' -f (Get-Date -Format 'yyyyMMdd-HHmmss'), $Fmt)
    }
    Write-Title 'Writing report'
    $lines = & $ExePath @(Get-ScanArgs $P $Fmt)
    $code = $LASTEXITCODE
    # UTF-8 without a BOM, so JSON parsers and Markdown viewers read it cleanly on 5.1 too.
    [IO.File]::WriteAllLines($target, [string[]]$lines, (New-Object Text.UTF8Encoding($false)))
    Write-ExitMeaning $code
    Write-Ok "Saved $target"
    if (-not $NoRedact) { Write-Info 'Your home folder and account name are redacted, so the file is safe to share.' }
    if ($Action -eq 'Menu' -and (Confirm-Step 'Show it in Explorer?')) { Start-Process explorer.exe -ArgumentList "/select,`"$target`"" }
}

# ---------------------------------------------------------------- menu

function Set-CustomPaths($P) {
    Write-Title 'Set custom paths'
    Write-Info 'Press Enter to keep the current value. Type - to clear a saved value and go back to detection.'
    $new = [ordered]@{ GameDir = $P.GameDir; UserDir = $P.UserDir; WorkshopDir = $P.WorkshopDir }
    foreach ($name in @($new.Keys)) {
        $value = Read-Host ("{0} [{1}]" -f $name, $new[$name])
        if ($value -eq '-') { $new[$name] = $null }
        elseif ($value) {
            $value = $value.Trim('"')
            if (Test-Path $value) { $new[$name] = (Resolve-Path $value).Path } else { Write-Warn2 "$value does not exist; keeping the old value." }
        }
    }
    Save-Paths ([pscustomobject]$new)
    Write-Ok "Saved to $ConfigFile"
}

function Open-Folder($P) {
    $choices = [ordered]@{ '1' = $P.GameDir; '2' = $P.UserDir; '3' = $P.WorkshopDir }
    if ($P.UserDir) { $choices['4'] = Join-Path $P.UserDir 'mod'; $choices['5'] = Join-Path $P.UserDir 'save games' }
    Write-Host '  1 Game  2 User data  3 Workshop  4 Local mods  5 Saves'
    $pick = Read-Host 'Open which folder'
    if ($choices.Contains($pick) -and $choices[$pick] -and (Test-Path $choices[$pick])) { Start-Process explorer.exe -ArgumentList "`"$($choices[$pick])`"" }
    else { Write-Warn2 'Nothing to open.' }
}

function Show-Menu {
    while ($true) {
        $P = Resolve-Paths
        Write-Host ''
        Write-Host '=== Stellaris Mod Doctor ===' -ForegroundColor Cyan
        $ver = ''
        if ($P.Version) { $ver = "  (Stellaris $($P.Version))" }
        Write-Host ("Game: {0}{1}" -f $(if ($P.GameDir) { $P.GameDir } else { 'not found' }), $ver) -ForegroundColor Gray
        Write-Host '  1  Show detected paths'
        Write-Host '  2  List mods'
        Write-Host '  3  List save games'
        Write-Host '  4  Scan for conflicts (on screen)'
        Write-Host '  5  Save a report (Markdown)'
        Write-Host '  6  Save a report (JSON)'
        Write-Host '  7  Show the game''s last error log'
        Write-Host '  8  Open a folder'
        Write-Host '  9  Set custom paths'
        Write-Host '  10 Check / install dependencies'
        Write-Host '  11 Rebuild smd'
        Write-Host '  Q  Quit'
        $choice = (Read-Host 'Choose').Trim()
        try {
            switch ($choice) {
                '1' { Show-Detection $P }
                '2' { Show-Mods $P }
                '3' { Show-Saves $P }
                '4' { Invoke-Scan $P 'table' }
                '5' { Invoke-Report $P 'md' }
                '6' { Invoke-Report $P 'json' }
                '7' { Show-ErrorLog $P }
                '8' { Open-Folder $P }
                '9' { Set-CustomPaths $P }
                '10' { Invoke-Setup | Out-Null }
                '11' { Invoke-Build -Force | Out-Null }
                { $_ -in 'q', 'Q' } { return }
                default { Write-Warn2 "Unknown choice '$choice'." }
            }
        } catch {
            Write-Bad $_.Exception.Message
        }
    }
}

# ---------------------------------------------------------------- entry point

Update-SessionPath
if (-not (Test-Path (Join-Path $RepoRoot 'Cargo.toml'))) {
    Write-Bad "This script must stay inside the Mods-Tool-Stellaris repository (expected Cargo.toml in $RepoRoot)."
    exit 1
}

$paths = Resolve-Paths
switch ($Action) {
    'Menu' { Show-Detection $paths; Show-Menu }
    'Detect' { Show-Detection $paths }
    'Mods' { Show-Mods $paths }
    'Saves' { Show-Saves $paths }
    'ErrorLog' { Show-ErrorLog $paths }
    'Setup' { if (-not (Invoke-Setup)) { exit 1 } }
    'Build' { if (-not (Invoke-Build -Force)) { exit 1 } }
    'Scan' { Invoke-Scan $paths $Format }
    'Report' { Invoke-Report $paths $Format }
}
