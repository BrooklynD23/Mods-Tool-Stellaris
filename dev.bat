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
rem    4. scan for conflicts and save reports\latest.md,
rem    5. open the report.
rem
rem  Read-only: nothing here writes to the game, the launcher, mods or saves.
rem
rem  From a terminal:
rem    dev.bat              the full run above
rem    dev.bat menu         interactive menu
rem    dev.bat paths        show detected game / user / workshop folders
rem    dev.bat mods         list every installed mod
rem    dev.bat scan         scan and print the table in this window
rem    dev.bat report       scan and save reports\latest.md (then open it)
rem    dev.bat json         scan and save reports\latest.json
rem    dev.bat setup        check / install Rust and the VS Build Tools
rem    dev.bat rebuild      force a release rebuild
rem    dev.bat test         developer gate: fmt, tests, clippy, xtask lint
rem    dev.bat help         this text
rem  Extra options pass through to ModDoctor.ps1, for example:
rem    dev.bat scan -NoRedact -GameDir "D:\Games\Stellaris"
rem ==========================================================================
setlocal EnableExtensions
title Stellaris Mod Doctor
cd /d "%~dp0"

set "ROOT=%~dp0"
set "PSFILE=%ROOT%tools\windows\ModDoctor.ps1"
set "REPORTS=%ROOT%reports"
set "ARG1=%~1"
set "RC=0"
rem rustup installs here; a fresh install is not on PATH until a new session.
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

if not exist "%PSFILE%" (
    echo [miss] %PSFILE% not found. Keep dev.bat in the repository root.
    set "RC=1"
    goto :end
)
where powershell.exe >nul 2>&1
if errorlevel 1 (
    echo [miss] Windows PowerShell was not found on PATH.
    set "RC=1"
    goto :end
)

set "CMD=%~1"
if "%CMD%"=="" set "CMD=run"

rem Everything after the command is passed through to ModDoctor.ps1.
set "REST="
:collect
shift
if "%~1"=="" goto :dispatch
set "REST=%REST% %1"
goto :collect

:dispatch
if /i "%CMD%"=="run"     goto :run
if /i "%CMD%"=="menu"    goto :menu
if /i "%CMD%"=="paths"   goto :paths
if /i "%CMD%"=="mods"    goto :mods
if /i "%CMD%"=="scan"    goto :scan
if /i "%CMD%"=="report"  goto :report_md
if /i "%CMD%"=="json"    goto :report_json
if /i "%CMD%"=="setup"   goto :setup
if /i "%CMD%"=="rebuild" goto :rebuild
if /i "%CMD%"=="test"    goto :test
if /i "%CMD%"=="help"    goto :help
if /i "%CMD%"=="-h"      goto :help
if /i "%CMD%"=="/?"      goto :help
echo Unknown command "%CMD%".
echo.
set "RC=2"
goto :help

rem -------------------------------------------------------------- commands

:run
echo === Step 1 of 3: build smd ===
call :ps -Action Build
if errorlevel 1 goto :failed
echo.
echo === Step 2 of 3: every installed mod ===
call :ps -Action Mods
echo.
echo === Step 3 of 3: conflict report ===
call :save md
goto :end

:menu
call :ps -Action Menu
goto :end

:paths
call :ps -Action Detect
goto :end

:mods
call :ps -Action Mods
goto :end

:scan
call :ps -Action Scan -Format table
goto :end

:report_md
call :save md
goto :end

:report_json
call :save json
goto :end

:setup
call :ps -Action Setup
goto :end

:rebuild
call :ps -Action Build
goto :end

:test
where cargo >nul 2>&1
if errorlevel 1 (
    echo [miss] cargo not found. Run "dev.bat setup" first.
    set "RC=1"
    goto :end
)
echo === cargo fmt --check ===
cargo fmt --all --check || goto :failed
echo === cargo test ===
cargo test --workspace || goto :failed
echo === cargo clippy ===
cargo clippy --workspace --all-targets -- -D warnings || goto :failed
echo === cargo xtask lint ===
cargo xtask lint || goto :failed
echo.
echo [ok] every gate passed
goto :end

:help
echo usage: dev.bat [run^|menu^|paths^|mods^|scan^|report^|json^|setup^|rebuild^|test^|help] [ModDoctor.ps1 options]
echo.
echo   run       build, list every installed mod, save and open reports\latest.md (default)
echo   menu      interactive menu
echo   paths     show the detected game, user and workshop folders
echo   mods      list every installed mod and where it came from
echo   scan      scan and print the conflict table here
echo   report    save reports\latest.md and open it
echo   json      save reports\latest.json
echo   setup     check / install Rust and the VS C++ Build Tools
echo   rebuild   force a release build of smd
echo   test      fmt, tests, clippy and xtask lint
echo.
echo options pass through, e.g.  dev.bat scan -NoRedact -GameDir "D:\Games\Stellaris"
goto :end

rem -------------------------------------------------------------- helpers

rem :ps <ModDoctor.ps1 arguments>  - runs the script, returns its exit code.
:ps
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%PSFILE%" %* %REST%
exit /b %errorlevel%

rem :save md|json  - writes reports\latest.<ext>, opens it when a report was produced.
:save
set "EXT=%~1"
set "OUT=%REPORTS%\latest.%EXT%"
if not exist "%REPORTS%" mkdir "%REPORTS%"
rem Remove the previous report so a failed scan never leaves a stale one to open.
if exist "%OUT%" del /q "%OUT%"
call :ps -Action Report -Format %EXT% -OutFile "%OUT%" -Yes
set "RC=%errorlevel%"
rem 0 = clean, 3 = finished with an Error-class diagnostic; both produce a report.
if not exist "%OUT%" (
    echo [miss] No report was written ^(exit code %RC%^). The messages above say why.
    exit /b %RC%
)
echo [ok]   Report: %OUT%
start "" notepad.exe "%OUT%"
exit /b %RC%

:failed
set "RC=%errorlevel%"
if "%RC%"=="0" set "RC=1"
echo.
echo [fail] Stopped with exit code %RC%. The messages above say why.
goto :end

:end
rem Keep the window open when started by double-click (no arguments).
if "%ARG1%"=="" pause
endlocal & exit /b %RC%
