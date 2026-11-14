# Skyline on Windows: build and run

The launcher can't run the 1.20.1 bridge, so on Windows you build the game yourself and run it
from a terminal. **Nobody has tested this on Windows yet**: if a step fails, report the error.

## 1. Install these once

| What | Where | Note |
|---|---|---|
| Minecraft Java Edition | the official launcher | start 1.20.1 and 26.3 once each |
| Git for Windows | <https://git-scm.com> | also gives you **Git Bash** (step 4) |
| Rust | <https://rustup.rs> | let it install the Visual Studio C++ build tools |
| JDK 25 | <https://adoptium.net> (Temurin 25, MSI) | tick "Set JAVA_HOME" |
| Python 3 | <https://python.org> | tick "Add python.exe to PATH" |

Open a new PowerShell after installing, so it sees them.

## 2. Get the code (PowerShell)

```powershell
git clone https://github.com/Antsahw/Skyline.git
cd Skyline
git checkout multiversion
```

## 3. Game files from your Minecraft install (PowerShell)

```powershell
cargo run --release -p horizon-prepare -- --prepare
```

This reads your own Minecraft install into `.local\`. Never share or commit that folder.

## 4. The 1.20.1 runtime

In PowerShell, download Minecraft 1.20.1 and the Fabric stack into
`%USERPROFILE%\.cache\horizon-120`:

```powershell
py -3 -I tools\java_bridge\fetch120.py
```

Then in **Git Bash**, in the Skyline folder, build the bridge's Java side:

```bash
tools/java_bridge/build120.sh
```

Run it again after every update that changes `crates/horizon-jvm/java/`.

## 5. Build the game (PowerShell)

```powershell
cargo build --profile fast -p horizon-client --features java-mods,java-mods-120
```

The first build takes a long time. Later ones only rebuild what changed.

## 6. Mods and play

Inside the Skyline folder:

```
mods-java-1.20.1\   Fabric 1.20.1 mods, plus Fabric API for 1.20.1 and their dependencies
mods-java\          Fabric 26.3 mods (optional)
```

```powershell
.\target\fast\horizon-client.exe --resources .local\horizon-client\26.3 --world MyWorld
```

The first start with new mods converts them, which takes a few minutes, and is cached after
that. Quit from the game's menu, not with Ctrl+C: the 1.20.1 mods' mobs are saved on quit.

If the game can't find the 1.20.1 runtime, set where step 4 put it before starting:

```powershell
$env:HORIZON_120_CACHE = "$env:USERPROFILE\.cache\horizon-120"
```

## Making a Windows release on GitHub

1. Build: `cargo build --release -p horizon-client --features java-mods,java-mods-120`
2. Zip `target\release\horizon-client.exe` (with this file, for the players' steps 1, 3 and 4).
3. Upload: `gh release create v0.1.0 skyline-windows.zip --title "Skyline v0.1.0"`, or on
   github.com under Releases > Draft a new release.

Never put `.local\`, the `.cache\horizon-120` folder or any Minecraft file in a release. Every
player prepares those from their own copy of Minecraft (steps 3 and 4).
