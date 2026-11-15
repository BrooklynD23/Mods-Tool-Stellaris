@echo off
rem ==========================================================================
rem  Stellaris Mod Doctor - one-click runner
rem
rem  Double-click this file to:
rem    1. check for the Rust toolchain and the VS C++ Build Tools (offers to
rem       install them, asking first),
rem    2. build smd (release),
rem    3. list every installed mod: Steam workshop in every library, launcher
rem       .mod files, hand-copied folders, loose zips, Paradox launcher DB mods,
rem    4. scan for conflicts, save reports\latest.md and open it.
rem
rem  Read-only: nothing here writes to the game, the launcher, mods or saves.
rem
rem  From a terminal:  dev.bat [command] [options]
rem    run (default)  menu  paths  mods  scan  report  saves  errorlog
rem    setup  rebuild  test  help
rem  Options pass through to tools\windows\ModDoctor.ps1, for example:
rem    dev.bat scan -NoRedact -GameDir "D:\Games\Stellaris"
rem    dev.bat report -Format json
rem
rem  Every argument goes to PowerShell untouched (the command binds to -Action),
rem  so paths with spaces, parentheses or & are safe. Keep this file free of
rem  parenthesized blocks: cmd.exe expands %%VARS%% inside them at parse time,
rem  and a ")" in the repository path would then end the block early.
rem ==========================================================================
setlocal EnableExtensions
title Stellaris Mod Doctor
cd /d "%~dp0"
rem rustup installs here; a fresh install is not on PATH until a new session.
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

if exist "%~dp0tools\windows\ModDoctor.ps1" goto :have_script
echo [miss] tools\windows\ModDoctor.ps1 not found. Keep dev.bat in the repository root.
set "RC=1"
goto :end
:have_script

where powershell.exe >nul 2>&1
if not errorlevel 1 goto :have_ps
echo [miss] Windows PowerShell was not found on PATH.
set "RC=1"
goto :end
:have_ps

if [%1]==[] goto :default
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\windows\ModDoctor.ps1" %*
set "RC=%errorlevel%"
goto :end

:default
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\windows\ModDoctor.ps1" -Action Run
set "RC=%errorlevel%"

:end
if not "%RC%"=="0" echo [fail] Finished with exit code %RC%. The messages above say why.
rem Keep the window open when started by double-click (no arguments).
if [%1]==[] pause
endlocal & exit /b %RC%
