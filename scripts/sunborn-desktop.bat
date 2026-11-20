@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0sunborn-desktop.ps1" %*
endlocal
