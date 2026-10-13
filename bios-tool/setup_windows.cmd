@echo off
REM Thin wrapper around setup_windows.ps1 for double-click / cmd users.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0setup_windows.ps1" %*
