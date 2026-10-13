@echo off
REM setup_windows.cmd - double-clickable wrapper.
REM Does not change machine-wide execution policy. If scripts are blocked,
REM run: powershell -ExecutionPolicy Bypass -File "%~dp0setup_windows.ps1" -CheckOnly

setlocal
cd /d "%~dp0"

echo bios-tool Windows setup wrapper
echo.
echo If you see an execution-policy error, open an elevated or normal PowerShell and run:
echo   Set-ExecutionPolicy -Scope CurrentUser RemoteSigned
echo or invoke explicitly:
echo   powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0setup_windows.ps1" -CheckOnly
echo.

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0setup_windows.ps1" %*
set EXITCODE=%ERRORLEVEL%
echo.
echo Exit code: %EXITCODE%
pause
exit /b %EXITCODE%
