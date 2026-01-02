@echo off
setlocal EnableDelayedExpansion

REM ============================================================================
REM AAA MMORPG ENGINE - FULLY AUTOMATED LAUNCHER
REM Double-click to play. Zero manual steps. Ever.
REM Builds custom Vulkan renderer + game automatically.
REM ============================================================================

title AAA MMORPG Engine

echo.
echo  ================================================================
echo      AAA MMORPG ENGINE - Fully Automated Launcher
echo      Custom Vulkan Renderer + Bevy Game Engine
echo      Zero Manual Steps. Auto-Sync. Auto-Build. Auto-Launch.
echo  ================================================================
echo.

REM Configuration
set "GITHUB_OWNER=Dhenz14"
set "GITHUB_REPO=aaa-mmorpg-launcher"
set "CONFIG_URL=https://raw.githubusercontent.com/%GITHUB_OWNER%/%GITHUB_REPO%/main/native-engine/launcher/server-config.json"
set "INSTALL_DIR=%LOCALAPPDATA%\AAAEngine"
set "ENGINE_DIR=%INSTALL_DIR%\engine"
set "LOG_DIR=%INSTALL_DIR%\logs"
set "CONFIG_CACHE=%INSTALL_DIR%\server-config.json"
set "VERSION_FILE=%INSTALL_DIR%\version.txt"
set "BUILD_STATUS=%INSTALL_DIR%\build-status.txt"
set "CPP_BUILD_STATUS=%INSTALL_DIR%\cpp-build-status.txt"
set "MAX_RETRIES=2"
set "RETRY_COUNT=0"

REM Create directories
if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"
if not exist "%LOG_DIR%" mkdir "%LOG_DIR%"

echo  Install: %INSTALL_DIR%
echo.

REM ============================================================================
REM STEP 1: Check build tools
REM ============================================================================
echo  [*] Checking build tools...

set "HAS_CMAKE=0"
set "HAS_NINJA=0"
set "HAS_RUST=0"
set "HAS_VULKAN=0"

where cmake >nul 2>&1 && set "HAS_CMAKE=1"
where ninja >nul 2>&1 && set "HAS_NINJA=0"
where cargo >nul 2>&1 && set "HAS_RUST=1"

if defined VULKAN_SDK (
    if exist "%VULKAN_SDK%\Lib\vulkan-1.lib" set "HAS_VULKAN=1"
)

if "%HAS_RUST%"=="0" (
    echo  [X] Rust not found! Installing...
    curl -L -o "%TEMP%\rustup-init.exe" https://win.rustup.rs/x86_64
    "%TEMP%\rustup-init.exe" -y --default-toolchain stable
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
    del "%TEMP%\rustup-init.exe" 2>nul
)

if "%HAS_CMAKE%"=="1" echo  [OK] CMake found
if "%HAS_VULKAN%"=="1" echo  [OK] Vulkan SDK found: %VULKAN_SDK%
if "%HAS_RUST%"=="1" echo  [OK] Rust found

echo.

REM ============================================================================
REM STEP 2: Discover server URL from GitHub
REM ============================================================================
echo  [*] Discovering server...

set "SERVER_URL="
set "TEMP_CONFIG=%TEMP%\aaa_config_%RANDOM%.json"

curl -s -L -o "%TEMP_CONFIG%" "%CONFIG_URL%" 2>nul

if exist "%TEMP_CONFIG%" (
    for /f "usebackq delims=" %%i in (`powershell -NoProfile -Command "(Get-Content '%TEMP_CONFIG%' | ConvertFrom-Json).server_url" 2^>nul`) do set "SERVER_URL=%%i"
    if defined SERVER_URL (
        copy /y "%TEMP_CONFIG%" "%CONFIG_CACHE%" >nul 2>&1
        echo  [OK] Server: !SERVER_URL!
    )
    del "%TEMP_CONFIG%" 2>nul
)

if not defined SERVER_URL (
    if exist "%CONFIG_CACHE%" (
        for /f "usebackq delims=" %%i in (`powershell -NoProfile -Command "(Get-Content '%CONFIG_CACHE%' | ConvertFrom-Json).server_url" 2^>nul`) do set "SERVER_URL=%%i"
        if defined SERVER_URL echo  [!] Using cached: !SERVER_URL!
    )
)

if not defined SERVER_URL (
    echo  [X] ERROR: Could not discover server
    pause
    exit /b 1
)

echo.

REM ============================================================================
REM STEP 3: Check version and auto-sync
REM ============================================================================
:check_version
echo  [*] Checking engine version...

set "LOCAL_VERSION="
set "REMOTE_VERSION="
set "NEED_SYNC=0"

REM Check for previous build failure - auto cleanup
if exist "%BUILD_STATUS%" (
    set /p LAST_STATUS=<"%BUILD_STATUS%"
    if "!LAST_STATUS!"=="FAILED" (
        echo  [!] Previous build failed - auto-cleaning...
        if exist "%ENGINE_DIR%" rmdir /s /q "%ENGINE_DIR%" 2>nul
        if exist "%VERSION_FILE%" del "%VERSION_FILE%" 2>nul
        del "%BUILD_STATUS%" 2>nul
        del "%CPP_BUILD_STATUS%" 2>nul
        set "NEED_SYNC=1"
    )
)

REM Get local version
if exist "%VERSION_FILE%" (
    set /p LOCAL_VERSION=<"%VERSION_FILE%"
)

REM Get remote version
set "TEMP_VER=%TEMP%\aaa_ver_%RANDOM%.json"
curl -s -L -o "%TEMP_VER%" "%SERVER_URL%/sync/version" 2>nul

if exist "%TEMP_VER%" (
    for /f "usebackq delims=" %%i in (`powershell -NoProfile -Command "(Get-Content '%TEMP_VER%' | ConvertFrom-Json).version" 2^>nul`) do set "REMOTE_VERSION=%%i"
    del "%TEMP_VER%" 2>nul
)

REM Compare versions - AUTO SYNC if different
if defined REMOTE_VERSION (
    if "!LOCAL_VERSION!" NEQ "!REMOTE_VERSION!" (
        echo  [*] Version mismatch - syncing automatically
        echo      Local:  !LOCAL_VERSION!
        echo      Remote: !REMOTE_VERSION!
        
        REM Auto-clean old engine to ensure fresh sync
        if exist "%ENGINE_DIR%" (
            echo  [*] Cleaning old engine...
            rmdir /s /q "%ENGINE_DIR%" 2>nul
        )
        set "NEED_SYNC=1"
    ) else (
        echo  [OK] Version current: !REMOTE_VERSION!
    )
) else (
    echo  [!] Could not check version - using local
)

REM Check if engine exists
if not exist "%ENGINE_DIR%\bevy-game\Cargo.toml" (
    echo  [*] Engine not found - downloading...
    set "NEED_SYNC=1"
)

echo.

REM ============================================================================
REM STEP 4: Download engine (if needed)
REM ============================================================================
if "%NEED_SYNC%"=="1" (
    echo  [*] Downloading engine (~345MB - this may take a few minutes)...
    
    if not exist "%ENGINE_DIR%" mkdir "%ENGINE_DIR%"
    
    set "ZIP_PATH=%INSTALL_DIR%\engine.zip"
    
    REM Use curl with retry and longer timeout for large download
    curl -L --retry 3 --retry-delay 5 --connect-timeout 30 --max-time 600 --progress-bar -o "!ZIP_PATH!" "%SERVER_URL%/sync/full.zip"
    set "CURL_EXIT=!ERRORLEVEL!"
    
    if !CURL_EXIT! neq 0 (
        echo  [X] Download failed with error code !CURL_EXIT!
        goto :sync_failed
    )
    
    if not exist "!ZIP_PATH!" (
        echo  [X] Download failed - file not created
        goto :sync_failed
    )
    
    REM Verify zip file size (should be at least 1MB for valid package)
    for %%A in ("!ZIP_PATH!") do set "ZIP_SIZE=%%~zA"
    if !ZIP_SIZE! LSS 1000000 (
        echo  [X] Download incomplete - file too small: !ZIP_SIZE! bytes
        del "!ZIP_PATH!" 2>nul
        goto :sync_failed
    )
    echo  [OK] Downloaded !ZIP_SIZE! bytes
    
    echo  [*] Extracting to %ENGINE_DIR%...
    
    REM Clear old engine dir first
    if exist "%ENGINE_DIR%" rmdir /s /q "%ENGINE_DIR%" 2>nul
    mkdir "%ENGINE_DIR%"
    
    REM Extract with error handling
    powershell -NoProfile -Command "try { Expand-Archive -Path '!ZIP_PATH!' -DestinationPath '%ENGINE_DIR%' -Force -ErrorAction Stop; exit 0 } catch { Write-Host $_.Exception.Message; exit 1 }"
    set "EXTRACT_EXIT=!ERRORLEVEL!"
    
    if !EXTRACT_EXIT! neq 0 (
        echo  [X] Extraction failed with error code !EXTRACT_EXIT!
        del "!ZIP_PATH!" 2>nul
        goto :sync_failed
    )
    
    REM Verify extraction succeeded
    if exist "%ENGINE_DIR%\bevy-game\Cargo.toml" (
        echo  [OK] Engine extracted successfully
        if defined REMOTE_VERSION (
            echo !REMOTE_VERSION!>"%VERSION_FILE%"
        )
        REM Force C++ rebuild on new sync
        del "%CPP_BUILD_STATUS%" 2>nul
    ) else (
        echo  [X] Extraction incomplete - Cargo.toml not found
        echo  [*] Contents of %ENGINE_DIR%:
        dir /b "%ENGINE_DIR%" 2>nul
        del "!ZIP_PATH!" 2>nul
        goto :sync_failed
    )
    
    del "!ZIP_PATH!" 2>nul
    echo  [OK] Engine synced
)

echo.

REM ============================================================================
REM STEP 5: Verify engine exists
REM ============================================================================
set "GAME_DIR=%ENGINE_DIR%\bevy-game"
set "CPP_DIR=%ENGINE_DIR%\atom-bridge\cpp"

if not exist "%GAME_DIR%\Cargo.toml" (
    echo  [X] Engine code missing after sync
    goto :sync_failed
)

REM ============================================================================
REM STEP 6: Build C++ Vulkan renderer (if available)
REM ============================================================================
set "CPP_LIB=%ENGINE_DIR%\atom-bridge\lib\atom_bridge.lib"
set "CPP_BUILD_DIR=%CPP_DIR%\build"

REM Check if C++ lib already built
if exist "%CPP_LIB%" (
    echo  [OK] Vulkan renderer library found
    goto :build_rust
)

REM Check if we already tried and failed C++ build
if exist "%CPP_BUILD_STATUS%" (
    set /p CPP_STATUS=<"%CPP_BUILD_STATUS%"
    if "!CPP_STATUS!"=="SKIP" (
        echo  [!] Skipping C++ build (using Bevy wgpu renderer)
        goto :build_rust
    )
)

REM Try to build C++ if cmake available
if "%HAS_CMAKE%"=="1" if "%HAS_VULKAN%"=="1" (
    echo.
    echo  ================================================================
    echo      Building Custom Vulkan Renderer
    echo  ================================================================
    echo.
    
    if not exist "%ENGINE_DIR%\atom-bridge\lib" mkdir "%ENGINE_DIR%\atom-bridge\lib"
    
    pushd "%CPP_DIR%"
    
    REM Try Ninja first, fall back to NMake or default
    set "CMAKE_GEN="
    where ninja >nul 2>&1 && set "CMAKE_GEN=-G Ninja"
    
    echo  [*] Configuring CMake...
    cmake -B build !CMAKE_GEN! -DCMAKE_BUILD_TYPE=Release 2>&1
    
    if !ERRORLEVEL! EQU 0 (
        echo  [*] Building C++ library (this takes a few minutes)...
        cmake --build build --config Release -j %NUMBER_OF_PROCESSORS% 2>&1
        
        if !ERRORLEVEL! EQU 0 (
            REM Find and copy the library
            if exist "build\lib\atom_bridge.lib" (
                copy /y "build\lib\atom_bridge.lib" "%ENGINE_DIR%\atom-bridge\lib\" >nul
                echo  [OK] Custom Vulkan renderer built!
            ) else if exist "build\Release\atom_bridge.lib" (
                copy /y "build\Release\atom_bridge.lib" "%ENGINE_DIR%\atom-bridge\lib\" >nul
                echo  [OK] Custom Vulkan renderer built!
            ) else if exist "build\atom_bridge.lib" (
                copy /y "build\atom_bridge.lib" "%ENGINE_DIR%\atom-bridge\lib\" >nul
                echo  [OK] Custom Vulkan renderer built!
            ) else (
                echo  [!] C++ build completed but library not found
                echo  [!] Falling back to Bevy wgpu renderer
                echo SKIP>"%CPP_BUILD_STATUS%"
            )
        ) else (
            echo  [!] C++ build failed - using Bevy wgpu renderer
            echo SKIP>"%CPP_BUILD_STATUS%"
        )
    ) else (
        echo  [!] CMake configure failed - using Bevy wgpu renderer
        echo SKIP>"%CPP_BUILD_STATUS%"
    )
    
    popd
) else (
    if "%HAS_CMAKE%"=="0" echo  [!] CMake not found - skipping C++ build
    if "%HAS_VULKAN%"=="0" echo  [!] Vulkan SDK not found - skipping C++ build
    echo  [*] Game will use Bevy's wgpu renderer (still works great!)
    echo SKIP>"%CPP_BUILD_STATUS%"
)

echo.

REM ============================================================================
REM STEP 7: Build Rust game
REM ============================================================================
:build_rust
set "EXE_PATH="

REM Check for existing executable
for %%e in (mmo-engine.exe bevy-game.exe aaa-mmorpg.exe) do (
    if exist "%GAME_DIR%\target\release\%%e" (
        set "EXE_PATH=%GAME_DIR%\target\release\%%e"
        goto :launch_game
    )
)

echo.
echo  ================================================================
echo      Building Game Engine
echo  ================================================================
echo.
echo  [*] First build takes 5-15 minutes...
echo.

REM Mark build as in-progress
echo BUILDING>"%BUILD_STATUS%"

pushd "%GAME_DIR%"

set CARGO_INCREMENTAL=1
set RUSTFLAGS=-C codegen-units=%NUMBER_OF_PROCESSORS%
set CARGO_BUILD_JOBS=%NUMBER_OF_PROCESSORS%

cargo build --release 2>&1

if !ERRORLEVEL! NEQ 0 (
    popd
    goto :build_failed
)

popd

REM Mark build as successful
echo SUCCESS>"%BUILD_STATUS%"

REM Find executable
for %%e in (mmo-engine.exe bevy-game.exe aaa-mmorpg.exe) do (
    if exist "%GAME_DIR%\target\release\%%e" (
        set "EXE_PATH=%GAME_DIR%\target\release\%%e"
        goto :launch_game
    )
)

echo  [X] Build succeeded but no executable found
goto :build_failed

REM ============================================================================
REM STEP 8: Launch game
REM ============================================================================
:launch_game
echo.
echo  ================================================================
echo      LAUNCHING GAME
echo  ================================================================
echo.

REM Show renderer status
if exist "%CPP_LIB%" (
    echo  [*] Renderer: Custom Vulkan (Render Fabric)
) else (
    echo  [*] Renderer: Bevy wgpu
)

echo  [*] Executable: %EXE_PATH%
echo.

start "" "%EXE_PATH%"

echo  [OK] Game launched!
echo.

timeout /t 3 >nul
exit /b 0

REM ============================================================================
REM ERROR HANDLERS - Auto-retry logic
REM ============================================================================
:build_failed
echo.
echo  [X] Build failed!

REM Mark as failed for next run
echo FAILED>"%BUILD_STATUS%"

set /a RETRY_COUNT+=1
if %RETRY_COUNT% LSS %MAX_RETRIES% (
    echo  [*] Auto-retry %RETRY_COUNT%/%MAX_RETRIES%: Cleaning and re-syncing...
    
    REM Clean everything
    if exist "%ENGINE_DIR%" rmdir /s /q "%ENGINE_DIR%" 2>nul
    if exist "%VERSION_FILE%" del "%VERSION_FILE%" 2>nul
    del "%CPP_BUILD_STATUS%" 2>nul
    
    echo.
    goto :check_version
)

echo  [X] Build failed after %MAX_RETRIES% attempts
echo  [X] Check that Rust is installed: https://rustup.rs
echo.
pause
exit /b 1

:sync_failed
echo.
echo  [X] Sync failed!

set /a RETRY_COUNT+=1
if %RETRY_COUNT% LSS %MAX_RETRIES% (
    echo  [*] Auto-retry %RETRY_COUNT%/%MAX_RETRIES%...
    
    if exist "%ENGINE_DIR%" rmdir /s /q "%ENGINE_DIR%" 2>nul
    if exist "%VERSION_FILE%" del "%VERSION_FILE%" 2>nul
    del "%CPP_BUILD_STATUS%" 2>nul
    
    timeout /t 3 >nul
    goto :check_version
)

echo  [X] Sync failed after %MAX_RETRIES% attempts
echo  [X] Check your internet connection
echo.
pause
exit /b 1
