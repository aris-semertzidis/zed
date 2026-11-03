@echo off
setlocal

:: --- Self-elevate if not running as admin ---
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo Requesting administrative privileges...
    powershell -Command "Start-Process '%~f0' -Verb RunAs"
    exit /b
)
:: --- End of elevation check ---

REM ---- MSVC environment ----
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
if errorlevel 1 (
    echo [ERROR] vcvars64.bat failed.
    pause
    exit /b 1
)

REM ---- Temp on D: to avoid C: permission issues ----
if not exist "D:\Temp" mkdir "D:\Temp"
set "TMP=D:\Temp"
set "TEMP=D:\Temp"

REM ---- Build ----
cd /d "%~dp0"
set "LK_CUSTOM_WEBRTC=D:\Coding\zed-webrtc-release"
set RELEASE_CHANNEL=stable
cargo build --profile release
::cargo build --profile release-fast -v 2>&1 | findstr /i "Dirty Fresh"

echo.
if errorlevel 1 (
    echo [FAIL] Build failed.
    pause
    exit /b 1
)

echo [OK] Build succeeded. Launching zed.exe...
echo.

REM ---- Run ----
"target\release\zed.exe"
echo.
echo [INFO] zed.exe exited with code %ERRORLEVEL%.
pause
endlocal