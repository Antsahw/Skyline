//! `horizon-play`: one step from nothing to playing Fabric mods in Horizon, on any system.
//!
//! Three ways in, one path through:
//! - Double-click (or run) it in a release folder: jars from `fabric-mods/` beside it, game in `game/`.
//! - `--instance <folder>`: the jars and settings of a Prism Launcher or MultiMC instance.
//! - As Prism Launcher's wrapper command (Prism sets `INST_MC_DIR`): press Play in Prism, with
//!   Prism's signed-in account. Prism keeps installing and updating the mods.
//!
//! Each finds Java and the player's own Minecraft files, prepares Horizon's resources once, and
//! starts the game. In a checkout, it builds the game first.

mod mods;

#[allow(dead_code)]
#[path = "../../horizon-jvm/src/libs.rs"]
mod libs;
#[allow(dead_code)]
#[path = "../../horizon-jvm/src/runtime.rs"]
mod runtime;

use sha1::{Digest, Sha1};
use std::ffi::OsString;
use std::io::{BufRead, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const JAVA_MAJOR: i64 = 25;
/// The game's exit code when the mods can't start (`horizon_jvm::NOT_STARTED_EXIT`).
const MODS_NOT_STARTED: i32 = 78;

const HELP: &str = "\
Plays Fabric mods in Horizon. Run it with nothing else to play the jars in the fabric-mods folder beside it.

Usage: horizon-play [--instance FOLDER] [--root FOLDER] [--dry-run] [-- GAME ARGUMENTS...]

  --instance FOLDER  Play a Prism Launcher or MultiMC instance's mods and settings
  --root FOLDER      The Horizon folder (default: this program's folder, or the checkout)
  --dry-run          Set everything up, print the game's command, don't start it

For Prism Launcher: set this program as the instance's Settings > Custom commands > Wrapper
command, then press Play.";

fn say(line: impl AsRef<str>) {
    println!("Horizon: {}", line.as_ref());
}

/// How this run was asked for.
struct Request {
    root: Option<PathBuf>,
    instance: Option<PathBuf>,
    dry_run: bool,
    game_args: Vec<OsString>,
    /// Started by Prism: its arguments are Java's, and its account comes on stdin.
    wrapper: bool,
}

fn parse(args: Vec<OsString>) -> Result<Option<Request>, String> {
    let wrapper = std::env::var_os("INST_MC_DIR").is_some_and(|d| !d.is_empty());
    let mut request = Request {
        root: None,
        instance: None,
        dry_run: false,
        game_args: Vec::new(),
        wrapper,
    };
    if wrapper {
        // Prism's arguments are Java's: these come from the environment.
        request.dry_run = std::env::var_os("HORIZON_PRISM_DRY").is_some();
        request.game_args = std::env::var("HORIZON_PRISM_ARGS")
            .unwrap_or_default()
            .split_whitespace()
            .map(OsString::from)
            .collect();
        return Ok(Some(request));
    }
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-h" | "--help") => {
                println!("{HELP}");
                return Ok(None);
            }
            Some("--dry-run") => request.dry_run = true,
            Some("--instance") => {
                request.instance = Some(args.next().ok_or("--instance needs a folder")?.into())
            }
            Some("--root") => {
                request.root = Some(args.next().ok_or("--root needs a folder")?.into())
            }
            Some("--") => request.game_args.extend(args.by_ref()),
            _ => return Err(format!("unknown option {}", arg.to_string_lossy())),
        }
    }
    Ok(Some(request))
}

/// The folders of one play: where the jars and settings come from, and the game's own.
struct Folders {
    mods: PathBuf,
    /// The launcher instance's game folder, whose `config/` the mods use.
    instance: Option<PathBuf>,
    home: PathBuf,
}

fn instance_game_folder(dir: &Path) -> PathBuf {
    [dir.join("minecraft"), dir.join(".minecraft")]
        .into_iter()
        .find(|d| d.is_dir())
        .unwrap_or_else(|| dir.to_path_buf())
}

fn folders(request: &Request, base: &Path) -> Folders {
    let instance = if request.wrapper {
        std::env::var_os("INST_MC_DIR").map(PathBuf::from)
    } else {
        request.instance.as_deref().map(instance_game_folder)
    };
    match instance {
        Some(game) => Folders {
            mods: game.join("mods"),
            home: game.join("horizon"),
            instance: Some(game),
        },
        None => Folders {
            mods: base.join("fabric-mods"),
            home: base.join("game"),
            instance: None,
        },
    }
}

/// The Horizon folder: `--root`, `HORIZON_ROOT`, beside this program (a release), the checkout
/// it was built from, or the current folder and up.
fn find_root(request: &Request, exe_dir: &Path) -> Result<PathBuf, String> {
    let has_lock = |d: &Path| d.join("research-lock.json").is_file();
    let built_from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cwd = std::env::current_dir().unwrap_or_default();
    request
        .root
        .clone()
        .or_else(|| std::env::var_os("HORIZON_ROOT").map(PathBuf::from))
        .into_iter()
        .chain([exe_dir.to_path_buf(), built_from])
        .chain(cwd.ancestors().map(Path::to_path_buf))
        .find(|d| has_lock(d))
        .and_then(|d| horizon_prepare::java::real_path(d).into())
        .ok_or_else(|| {
            format!(
                "research-lock.json isn't beside this program. Keep every file of the Horizon \
                 download together, in the folder you extracted ({}).",
                exe_dir.display()
            )
        })
}

/// In a checkout, builds the game (a no-op when it's current). Returns the folder of programs.
fn programs(root: &Path, exe_dir: &Path, wrapper: bool) -> Result<PathBuf, String> {
    let name = |p: &str| format!("{p}{}", std::env::consts::EXE_SUFFIX);
    let release = exe_dir.join(name("horizon-client"));
    if exe_dir.join("research-lock.json").is_file() && release.is_file() {
        return Ok(exe_dir.to_path_buf());
    }
    if !root.join("Cargo.toml").is_file() {
        return Err(format!(
            "the game ({}) isn't beside this program. Keep every file of the Horizon download \
             together, in the folder you extracted.",
            name("horizon-client")
        ));
    }
    // target/<profile>/horizon-play: the game goes in target/fast.
    let in_target = exe_dir
        .file_name()
        .is_some_and(|n| n == "fast" || n == "debug" || n == "release");
    let target = if in_target {
        exe_dir.parent().unwrap_or(exe_dir).to_path_buf()
    } else {
        root.join("target")
    };
    let fast = target.join("fast");
    let built = fast.join(name("horizon-client")).is_file();
    // Started by Prism, a build would look like a hang: it builds only if there's no game yet.
    if wrapper && built {
        return Ok(fast);
    }
    say(if built {
        "Checking the game is up to date"
    } else {
        "Building the game, once: 5 to 15 minutes"
    });
    let status = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .current_dir(root)
        .env("CARGO_TARGET_DIR", &target)
        .args(["build", "--profile", "fast"])
        .args([
            "-p",
            "horizon-client",
            "-p",
            "horizon-client-lib",
            "-p",
            "horizon-prepare",
        ])
        .args(["--features", "horizon-client/java-mods"])
        .status()
        .map_err(|e| format!("couldn't run cargo: {e}. Install Rust from https://rustup.rs"))?;
    if !status.success() {
        return Err("the game didn't build: the error is above".into());
    }
    Ok(fast)
}

/// What research-lock.json pins for the game.
struct Pins {
    version: String,
    manifest_sha1: String,
}

fn pins(root: &Path) -> Result<Pins, String> {
    let path = root.join("research-lock.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let field = |name: &str| {
        json["minecraft"][name]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("{} has no minecraft {name}", path.display()))
    };
    Ok(Pins {
        version: field("version")?,
        manifest_sha1: field("manifest_sha1")?,
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Downloads `url` from Mojang into `path`, unless it's there with the right SHA-1.
fn fetch(url: &str, sha1: &str, path: &Path) -> Result<(), String> {
    let good = |bytes: &[u8]| hex(&Sha1::digest(bytes)) == sha1;
    if std::fs::read(path).is_ok_and(|b| good(&b)) {
        return Ok(());
    }
    let mut bytes = Vec::new();
    libs::agent()
        .get(url)
        .call()
        .map_err(|e| format!("couldn't download {url}: {e}. Check your internet connection."))?
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("couldn't download {url}: {e}"))?;
    if !good(&bytes) {
        return Err(format!("{url} wasn't the expected file: refusing it"));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// The folders Prism Launcher may keep its files in: the instance's own launcher first.
fn prism_folders(instance: Option<&Path>) -> Vec<PathBuf> {
    // <launcher>/instances/<name>/minecraft
    let mut found: Vec<PathBuf> = instance
        .and_then(|game| {
            game.ancestors()
                .find(|d| d.file_name().is_some_and(|n| n == "instances"))
        })
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .into_iter()
        .collect();
    if cfg!(windows) {
        found.extend(std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("PrismLauncher")));
    } else if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        found.push(home.join("Library/Application Support/PrismLauncher"));
        found.push(home.join(".local/share/PrismLauncher"));
        found.push(home.join(".var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher"));
    }
    found
}

/// The ways to name the player's own game files to the preparer, best first.
fn game_files(
    version: &str,
    manifest: &Path,
    instance: Option<&Path>,
) -> Vec<(&'static str, Vec<OsString>)> {
    let prism: Vec<(&str, Vec<OsString>)> = prism_folders(instance)
        .into_iter()
        .filter_map(|dir| {
            let jar = dir.join(format!(
                "libraries/com/mojang/minecraft/{version}/minecraft-{version}-client.jar"
            ));
            jar.is_file().then(|| {
                let args = vec![
                    "--version-json".into(),
                    manifest.into(),
                    "--client-jar".into(),
                    jar.into(),
                    "--libraries-dir".into(),
                    dir.join("libraries").into(),
                    "--assets-dir".into(),
                    dir.join("assets").into(),
                ];
                ("Prism Launcher", args)
            })
        })
        .collect();
    let launchers = [
        (
            "the official Minecraft launcher",
            Some(horizon_prepare::artifacts::minecraft_dir(None)),
        ),
        (
            "the Horizon launcher",
            runtime::launcher_dir().map(|d| d.join("minecraft")),
        ),
    ];
    let official: Vec<(&str, Vec<OsString>)> = launchers
        .into_iter()
        .filter_map(|(name, dir)| {
            let dir = dir?;
            dir.join(format!("versions/{version}/{version}.jar"))
                .is_file()
                .then(|| (name, vec!["--minecraft-dir".into(), dir.into()]))
        })
        .collect();
    // An instance's own launcher has everything: it just downloaded the game to start it.
    if instance.is_some() {
        prism.into_iter().chain(official).collect()
    } else {
        official.into_iter().chain(prism).collect()
    }
}

/// A Java home for `java` inside its `bin`.
fn home_of(java: &Path) -> Option<PathBuf> {
    java.parent()?.parent().map(Path::to_path_buf)
}

/// Java for the preparer and the mods: the instance's, one on this machine, else Mojang's own,
/// downloaded once.
fn find_java(instance: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(home) = std::env::var_os("HORIZON_JAVA_HOME").filter(|h| !h.is_empty()) {
        return Ok(home.into());
    }
    let good = |home: &Path| runtime::release_major(home).is_some_and(|m| m >= JAVA_MAJOR);
    if let Some(home) = std::env::var_os("INST_JAVA")
        .map(PathBuf::from)
        .and_then(|j| home_of(&j))
        .filter(|h| good(h))
    {
        return Ok(home);
    }
    let official = horizon_prepare::artifacts::minecraft_dir(None).join("runtime");
    let mut more = horizon_prepare::java::minecraft_runtimes(&official);
    if let Some(dir) = runtime::runtime_dir() {
        more.extend(horizon_prepare::java::minecraft_runtimes(&dir));
    }
    // Prism Launcher's downloads: <launcher>/java/<component>/bin.
    for prism in prism_folders(instance) {
        more.extend(
            std::fs::read_dir(prism.join("java"))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path()),
        );
    }
    more.retain(|h| good(h));
    // Only launchers' runtimes, none installed on the system: how a player with no Java sees it.
    let no_system = std::env::var("HORIZON_JAVA_NO_SYSTEM").is_ok_and(|v| v == "1");
    if no_system {
        if let Some(home) = more.into_iter().next() {
            return Ok(home);
        }
    } else if let Ok(jdk) = horizon_prepare::java::find_runtime(JAVA_MAJOR, &more) {
        let java = horizon_prepare::java::real_path(jdk.java);
        if let Some(home) = home_of(&java).filter(|h| good(h)) {
            return Ok(home);
        }
    }
    let dir = runtime::runtime_dir().ok_or("couldn't find your data folder for Java")?;
    say("Downloading Java from Mojang, once");
    runtime::ensure("java-runtime-epsilon", &dir, &|line| say(line)).map_err(|e| {
        format!(
            "Java {JAVA_MAJOR} couldn't be downloaded: {e}. \
             Install Java {JAVA_MAJOR} from https://adoptium.net"
        )
    })
}

/// Prepares Horizon's resources from the player's own game, unless they're ready.
fn prepare(
    root: &Path,
    programs: &Path,
    pins: &Pins,
    java: &Path,
    instance: Option<&Path>,
) -> Result<PathBuf, String> {
    let resources = root.join(".local/horizon-client").join(&pins.version);
    let preparer = programs.join(format!("horizon-prepare{}", std::env::consts::EXE_SUFFIX));
    let command = || {
        let mut c = Command::new(&preparer);
        c.current_dir(root)
            .env("JAVA_HOME", java)
            .arg("--root")
            .arg(root);
        c
    };
    let ready = command()
        .arg("--check")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("{}: {e}", preparer.display()))?
        .success();
    if ready {
        return Ok(resources);
    }
    let version = &pins.version;
    say(format!(
        "Preparing Minecraft {version}, once: about a minute"
    ));
    let downloads = root.join(".local/downloads");
    let manifest = downloads.join(format!("{version}.json"));
    let sha1 = &pins.manifest_sha1;
    fetch(
        &format!("https://piston-meta.mojang.com/v1/packages/{sha1}/{version}.json"),
        sha1,
        &manifest,
    )?;
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| format!("{}: {e}", manifest.display()))?;
    let server = &json["downloads"]["server"];
    let (Some(url), Some(server_sha1)) = (server["url"].as_str(), server["sha1"].as_str()) else {
        return Err(format!("{} names no server download", manifest.display()));
    };
    let jar = downloads.join(format!("server-{version}.jar"));
    fetch(url, server_sha1, &jar)?;
    let sources = game_files(version, &manifest, instance);
    if sources.is_empty() {
        return Err(format!(
            "Minecraft {version} isn't on this computer yet. Open the official Minecraft launcher \
             (or Prism Launcher), play {version} once until the title screen, close it, and try \
             again."
        ));
    }
    for (name, args) in sources {
        say(format!("Using Minecraft's files from {name}"));
        let done = command()
            .args(["--prepare", "--complete", "--server-jar"])
            .arg(&jar)
            .args(&args)
            .status()
            .map_err(|e| format!("{}: {e}", preparer.display()))?;
        if done.success() {
            return Ok(resources);
        }
    }
    Err(format!(
        "Minecraft's files couldn't be prepared: the reason is above. Playing {version} once in \
         your launcher, to the title screen, fixes most of these."
    ))
}

/// Makes `link` a link to the folder `target`, with no special privilege: a junction on Windows.
fn link_folder(target: &Path, link: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        // Launchers pass paths with forward slashes, which cmd takes for switches.
        let windows = |p: &Path| p.to_string_lossy().replace('/', "\\");
        let done = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(windows(link))
            .arg(windows(target))
            .stdout(std::process::Stdio::null())
            .status()?;
        if done.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("mklink failed"))
        }
    }
    #[cfg(not(windows))]
    std::os::unix::fs::symlink(target, link)
}

/// The account a launcher passed: `--username`, `--uuid`, `--accessToken`, `--xuid`.
#[derive(Debug, Default, PartialEq, Eq)]
struct Account {
    name: Option<String>,
    uuid: Option<String>,
    token: Option<String>,
    xuid: Option<String>,
}

/// Reads Prism's launch stream (`param <value>` lines up to `launch`) into game arguments.
fn launch_params(stdin: impl BufRead) -> Vec<String> {
    let mut params = Vec::new();
    for line in stdin.lines().map_while(Result::ok) {
        if let Some(value) = line.strip_prefix("param ") {
            params.push(value.to_string());
        } else if line == "launch" || line == "abort" {
            break;
        }
    }
    params
}

fn account(args: &[String]) -> Account {
    let mut account = Account::default();
    for pair in args.windows(2) {
        let value = Some(pair[1].clone());
        match pair[0].as_str() {
            "--username" => account.name = value,
            "--uuid" => account.uuid = value,
            "--accessToken" => account.token = value,
            "--xuid" => account.xuid = value,
            _ => {}
        }
    }
    account
}

/// Writes the account for the game, which reads this file once and deletes it.
fn write_session(home: &Path, account: &Account) -> Option<PathBuf> {
    let (token, uuid) = (account.token.as_deref()?, account.uuid.as_deref()?);
    if token == "0" || token.is_empty() {
        return None;
    }
    let mut json = serde_json::json!({
        "name": account.name, "uuid": uuid, "access_token": token, "user_type": "msa",
    });
    if let Some(xuid) = account.xuid.as_deref().filter(|x| !x.is_empty()) {
        json["xuid"] = xuid.into();
    }
    let path = home.join("session.json");
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&path).ok()?;
    file.write_all(json.to_string().as_bytes()).ok()?;
    Some(path)
}

/// Why the game's log says the mods didn't start: that line and the lines after it, up to the
/// next log line (a compiler's errors take several).
fn not_started_reason(log: &str) -> Option<String> {
    let mut lines = log.lines().rev().collect::<Vec<_>>();
    lines.reverse();
    let start = lines
        .iter()
        .rposition(|l| l.contains("Java mods: not started: "))?;
    let (_, first) = lines[start].split_once("Java mods: not started: ")?;
    let rest = lines[start + 1..]
        .iter()
        .take_while(|l| !l.starts_with('['))
        .take(12);
    let mut reason = first.to_string();
    for line in rest {
        reason.push('\n');
        reason.push_str(line);
    }
    Some(reason)
}

fn run(request: Request) -> Result<i32, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe
        .parent()
        .ok_or("this program has no folder")?
        .to_path_buf();
    let root = find_root(&request, &exe_dir)?;
    let params = if request.wrapper && !std::io::stdin().is_terminal() {
        launch_params(std::io::stdin().lock())
    } else {
        Vec::new()
    };
    let programs = programs(&root, &exe_dir, request.wrapper)?;
    // A release plays beside itself; a checkout, in its .local folder, away from the sources.
    let base = if exe_dir.join("research-lock.json").is_file() {
        exe_dir.clone()
    } else {
        root.join(".local/play")
    };
    let places = folders(&request, &base);
    let pins = pins(&root)?;
    let instance = places.instance.as_deref();
    let java = find_java(instance)?;
    say(format!("Java: {}", java.display()));
    let resources = prepare(&root, &programs, &pins, &java, instance)?;

    std::fs::create_dir_all(&places.mods).map_err(|e| format!("{}: {e}", places.mods.display()))?;
    std::fs::create_dir_all(&places.home).map_err(|e| format!("{}: {e}", places.home.display()))?;
    let count = mods::sync(&places.mods, &places.home.join("mods-java"), &|l| say(l))?;
    say(format!("{count} mods from {}", places.mods.display()));
    if count == 0 && places.instance.is_none() {
        say(format!(
            "Put Fabric mods for Minecraft {} in {} to play them",
            pins.version,
            places.mods.display()
        ));
    }
    // A modpack ships its settings: the mods read the instance's own.
    if let Some(game) = places.instance.as_deref() {
        for name in ["config", "defaultconfigs"] {
            let (target, link) = (game.join(name), places.home.join(name));
            let _ = std::fs::create_dir_all(&target);
            if std::fs::symlink_metadata(&link).is_err()
                && let Err(e) = link_folder(&target, &link)
            {
                say(format!("The mods get their own {name} folder ({e})"));
            }
        }
    }

    let who = account(&params);
    let session = write_session(&places.home, &who);
    match (&session, &who.name) {
        (Some(_), Some(name)) => say(format!("Signed in as {name}")),
        (None, Some(name)) => say(format!("Playing offline as {name}")),
        _ => {}
    }
    let client = programs.join(format!("horizon-client{}", std::env::consts::EXE_SUFFIX));
    let mut game = Command::new(&client);
    game.current_dir(&places.home)
        .env("HORIZON_HOME", &places.home)
        .env("HORIZON_ROOT", &root)
        .env("HORIZON_JAVA_HOME", &java)
        .arg("--resources")
        .arg(&resources);
    if let Some(name) = &who.name {
        game.args(["--name", name]);
    }
    if let Some(session) = &session {
        game.env("HORIZON_SESSION_FILE", session);
    }
    game.args(&request.game_args);
    if request.dry_run {
        if let Some(session) = &session {
            let _ = std::fs::remove_file(session);
        }
        println!("{game:?}");
        return Ok(0);
    }
    say(if count > 0 {
        "Starting the game. The first start with new mods takes a few minutes"
    } else {
        "Starting the game"
    });
    let status = game
        .status()
        .map_err(|e| format!("{}: {e}", client.display()))?;
    let log = places.home.join("logs/latest.log");
    if status.code() == Some(MODS_NOT_STARTED) {
        let reason = std::fs::read_to_string(&log)
            .ok()
            .and_then(|text| not_started_reason(&text))
            .unwrap_or_else(|| "the reason is in the log".into());
        return Err(format!(
            "your mods couldn't start, so the game stopped to keep your worlds safe.\n\n{reason}\n\n\
             If this names a mod, try removing that mod. The full log is {}",
            log.display()
        ));
    }
    if !status.success() {
        return Err(format!(
            "the game stopped with an error. Its log is {}",
            log.display()
        ));
    }
    Ok(status.code().unwrap_or(0))
}

fn main() -> ExitCode {
    let wrapper = std::env::var_os("INST_MC_DIR").is_some_and(|d| !d.is_empty());
    let result = match parse(std::env::args_os().skip(1).collect()) {
        Ok(Some(request)) => run(request),
        Ok(None) => Ok(0),
        Err(e) => Err(format!("{e}\n\n{HELP}")),
    };
    match result {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(e) => {
            eprintln!("\nHorizon couldn't start: {e}");
            // A double-clicked window would close before this could be read.
            if !wrapper && std::io::stdin().is_terminal() {
                eprintln!("\nPress Enter to close.");
                let _ = std::io::stdin().read_line(&mut String::new());
            }
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_prisms_launch_stream() {
        let stream = "mainClass net.minecraft.client.main.Main\nparam --username\nparam Steve\n\
                      param --uuid\nparam abc\nparam --accessToken\nparam secret\nparam --xuid\n\
                      param 42\nlaunch\nparam --late\n";
        let params = launch_params(stream.as_bytes());
        assert!(!params.contains(&"--late".to_string()));
        assert_eq!(
            account(&params),
            Account {
                name: Some("Steve".into()),
                uuid: Some("abc".into()),
                token: Some("secret".into()),
                xuid: Some("42".into()),
            }
        );
    }

    #[test]
    fn the_reason_takes_a_compilers_lines_but_not_the_next_log_line() {
        let log = "[01:00:00] [main/ERROR]: Java mods: not started: javac failed:\n\
                   Foo.java:1: error\n\
                   1 error\n\
                   [01:00:01] [main/INFO]: later\n";
        assert_eq!(
            not_started_reason(log).as_deref(),
            Some("javac failed:\nFoo.java:1: error\n1 error")
        );
        assert_eq!(not_started_reason("[01:00:00] [main/INFO]: fine\n"), None);
    }

    #[test]
    fn an_offline_account_has_no_session() {
        let dir = std::env::temp_dir().join(format!("horizon-play-session-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let offline = Account {
            name: Some("Steve".into()),
            uuid: Some("abc".into()),
            token: Some("0".into()),
            xuid: None,
        };
        assert!(write_session(&dir, &offline).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
