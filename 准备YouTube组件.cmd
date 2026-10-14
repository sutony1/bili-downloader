@echo off
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0prepare-youtube.ps1"
if errorlevel 1 echo Component preparation failed. See README for manual setup.
pause
