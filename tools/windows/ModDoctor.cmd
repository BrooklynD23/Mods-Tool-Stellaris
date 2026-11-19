@echo off
rem Double-click to open the Stellaris Mod Doctor menu, or pass options through:
rem   ModDoctor.cmd -Action Scan -Format md
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0ModDoctor.ps1" %*
if "%~1"=="" pause
