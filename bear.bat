@echo off
rem Double-click to play. Uses whatever python is on PATH.
cd /d "%~dp0"
python bear-start.py %*
if errorlevel 1 pause
