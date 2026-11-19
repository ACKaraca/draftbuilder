//! `draftbuilder-cli`: the headless CLI. Gathers the arguments, streams, clock and edit-lock
//! owner and runs [`draftbuilder_cli::run`]; every error becomes an exit code.

use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use arc_command::Clock;
use arc_command::headless::Exit;
use arc_doc::package::LockOwner;

/// The system clock.
struct SystemClock;

impl Clock for SystemClock {
    fn now_unix_millis(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
    }
}

/// The first of the environment variables that is set, or `fallback`.
fn env(names: &[&str], fallback: &str) -> String {
    names
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| fallback.to_owned())
}

fn main() -> ExitCode {
    let owner = LockOwner {
        user: env(&["USER", "USERNAME"], "unknown"),
        host: env(&["HOSTNAME", "COMPUTERNAME"], "localhost"),
        application: format!("draftbuilder-cli {}", env!("CARGO_PKG_VERSION")),
        time_unix_ms: SystemClock.now_unix_millis(),
    };
    let exit = match draftbuilder_cli::product(Arc::new(SystemClock), owner) {
        Ok(product) => draftbuilder_cli::run(
            std::env::args_os().skip(1),
            &product,
            io::stdin().lock(),
            io::stdout(),
            io::stderr(),
        ),
        Err(error) => {
            let _ = writeln!(io::stderr(), "{{\"error\":\"{}\"}}", error.code());
            Exit::Internal
        }
    };
    ExitCode::from(exit.code())
}
