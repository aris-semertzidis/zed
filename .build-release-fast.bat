@echo off
setlocal

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
cargo build --profile release-fast

echo.
if errorlevel 1 (
    echo [FAIL] Build failed.
    pause
    exit /b 1
)

echo [OK] Build succeeded. Launching zed.exe...
echo.

REM ---- Run ----
"target\release-fast\zed.exe"
echo.
echo [INFO] zed.exe exited with code %ERRORLEVEL%.
pause
endlocal