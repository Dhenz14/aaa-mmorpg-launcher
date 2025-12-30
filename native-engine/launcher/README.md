# AAA MMORPG Engine Launcher

A native Windows executable that fully automates engine setup, building, and launching.

## Features

- **Self-elevating**: Automatically requests admin rights when needed
- **Dependency management**: Installs Rust, Vulkan SDK, VS Build Tools
- **Delta sync**: Only downloads changed files from server
- **Build automation**: Runs CMake and builds the Render Fabric
- **Validation tests**: Verifies Vulkan compatibility on your GPU
- **Logging**: All operations logged to `%LOCALAPPDATA%\AAAEngine\logs\`

## Getting the Launcher

### Option 1: Download from GitHub Releases
Download `aaa-launcher.exe` from the latest release.

### Option 2: Build from Source
1. Fork/clone this repo to GitHub
2. Go to Actions → "Build Windows Launcher"
3. Click "Run workflow"
4. Download the artifact

### Option 3: Build Locally on Windows
```powershell
cd native-engine/launcher
cargo build --release
# Output: target/release/aaa-launcher.exe
```

## Usage

```cmd
# Normal usage - double-click or run:
aaa-launcher.exe

# Test mode (checks dependencies, doesn't build):
aaa-launcher.exe --dry-run

# Verbose logging:
aaa-launcher.exe --verbose

# Show help:
aaa-launcher.exe --help
```

## What It Does

1. **Init**: Creates install directory at `%LOCALAPPDATA%\AAAEngine`
2. **Self-Update**: Checks server for launcher updates
3. **Dependency Audit**: Checks/installs Rust, Vulkan SDK, VS Build Tools
4. **Sync**: Downloads engine source from server
5. **Build**: Compiles the Render Fabric (CMake + C++)
6. **Validation**: Runs GPU validation tests
7. **Launch**: Starts the game

## Troubleshooting

### Launcher closes immediately
- Run from Command Prompt to see error messages
- Check logs at `%LOCALAPPDATA%\AAAEngine\logs\`

### Admin rights issues
- Right-click → Run as Administrator
- Or run with `--skip-elevation` (some features may not work)

### Build failures
- Ensure Visual Studio 2019/2022 Build Tools are installed
- Check that Vulkan SDK is properly installed
- Review logs for specific error messages

## Configuration

Config file: `%LOCALAPPDATA%\AAAEngine\launcher_config.json`

```json
{
  "server_url": "https://your-replit-app.replit.app",
  "install_dir": "C:\\Users\\You\\AppData\\Local\\AAAEngine",
  "vulkan_version": "1.3.290.0",
  "force_rebuild": false,
  "verbose": false
}
```
