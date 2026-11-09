@echo off
rem OptiRAR Setup - double-click to install for the current user (no admin needed)
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1" %*
if errorlevel 1 pause
