//! `draftbuilder`: the desktop application. Gathers the clock, the edit-lock owner, the data
//! directory with the user settings, recent files, window states and logs, and the files named
//! on the command line, builds the product and runs the event loop. A start-up failure is
//! reported on standard error with a non-zero exit code.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use arc_app::application::{AppConfig, Application};
use arc_app::locale::Localizer;
use arc_app::recent::RecentFiles;
use arc_app::shell::UserFiles;
use arc_app::shell::event_loop::{self, Startup};
use arc_base::id::{IdGenerator, RandomSource, TimeSource};
use arc_base::{Error, Limits};
use arc_command::Clock;
use arc_doc::package::LockOwner;
use arc_gpu::{Gpu, GpuSettings};

/// The most recent files kept.
const RECENT: usize = 20;

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// The system clock.
struct SystemClock;

impl Clock for SystemClock {
    fn now_unix_millis(&self) -> u64 {
        now_millis()
    }
}

impl TimeSource for SystemClock {
    fn unix_millis(&mut self) -> u64 {
        now_millis()
    }
}

/// SplitMix64 over a seed from the clock, the process and a counter: the random field of
/// identifiers needs uniqueness, not secrecy.
struct Mix(u64);

impl RandomSource for Mix {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// The first of the environment variables that is set, or `fallback`.
fn env(names: &[&str], fallback: &str) -> String {
    names
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| fallback.to_owned())
}

/// The per-user data directory: `DRAFTBUILDER_DATA`, else the platform's application data
/// directory with `draftbuilder` appended.
fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("DRAFTBUILDER_DATA")
        && !dir.is_empty()
    {
        return PathBuf::from(dir);
    }
    let base = if cfg!(target_os = "windows") {
        PathBuf::from(env(&["APPDATA"], "."))
    } else if cfg!(target_os = "macos") {
        PathBuf::from(env(&["HOME"], ".")).join("Library/Application Support")
    } else {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .filter(|v| !v.is_empty())
            .map_or_else(
                || PathBuf::from(env(&["HOME"], ".")).join(".local/share"),
                PathBuf::from,
            )
    };
    base.join("draftbuilder")
}

/// Writes a start-up problem that does not stop the application to standard error, once.
fn report(error: &Error, localizer: &Localizer) {
    let _ = writeln!(
        std::io::stderr(),
        "{}",
        draftbuilder::describe(error, Some(localizer))
    );
}

/// Runs the application; the error is the start-up failure as a line for standard error.
fn start() -> Result<(), String> {
    let fail = |e: Error| draftbuilder::describe(&e, None);
    let owner = LockOwner {
        user: env(&["USER", "USERNAME"], "unknown"),
        host: env(&["HOSTNAME", "COMPUTERNAME"], "localhost"),
        application: format!("draftbuilder {}", env!("CARGO_PKG_VERSION")),
        time_unix_ms: now_millis(),
    };
    let os_locales: Vec<String> = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|n| std::env::var(n).ok())
        .collect();
    let localizer = draftbuilder::localizer(&os_locales).map_err(fail)?;
    let with = |e: Error| draftbuilder::describe(&e, draftbuilder::localizer(&[]).ok().as_ref());
    let dir = data_dir();
    std::fs::create_dir_all(&dir).map_err(|e| {
        with(
            Error::new(arc_app::codes::FILE_WRITE_FAILED)
                .with_arg("path", dir.display().to_string())
                .with_source(e),
        )
    })?;
    let product = draftbuilder::product(Arc::new(SystemClock), owner).map_err(with)?;
    let recent_path = dir.join("recent.toml");
    let (recent, _) =
        RecentFiles::load(&recent_path, RECENT).unwrap_or((RecentFiles::new(RECENT), None));
    let seed = now_millis() ^ (u64::from(std::process::id()) << 32);
    let counter = AtomicU64::new(0);
    let config = AppConfig {
        data_dir: dir.clone(),
        limits: Limits::DEFAULT,
        id_generators: Box::new(move || {
            let n = counter.fetch_add(1, Ordering::Relaxed);
            IdGenerator::new(
                SystemClock,
                Mix(seed ^ n.wrapping_mul(0xA24B_AED4_963E_E407)),
            )
        }),
        recent,
    };
    let mut app = Application::new(product, config);
    for file in std::env::args_os().skip(1).map(PathBuf::from) {
        // A file that cannot be opened is reported and the application still starts.
        if let Err(e) = app.open(&file) {
            report(&e, &localizer);
        }
    }
    if app.tabs().is_empty() {
        app.new_document().map_err(with)?;
    }
    let gpu = Gpu::new(&GpuSettings::default()).map_err(with)?;
    let title = draftbuilder::title(&localizer);
    let result = event_loop::run(Startup {
        app,
        config: draftbuilder::config(seed.rotate_left(17)).map_err(with)?,
        localizer,
        gpu,
        title,
        window_states: Some(dir.join("windows.toml")),
        files: Some(UserFiles {
            settings: dir.join("settings.toml"),
            recent: recent_path,
        }),
        os_locales,
        log_dir: Some(dir.join("logs")),
        instance: None,
        automation_dir: None,
    });
    result.map_err(with)
}

fn main() -> ExitCode {
    match start() {
        Ok(()) => ExitCode::SUCCESS,
        Err(line) => {
            let _ = writeln!(std::io::stderr(), "{line}");
            ExitCode::FAILURE
        }
    }
}
