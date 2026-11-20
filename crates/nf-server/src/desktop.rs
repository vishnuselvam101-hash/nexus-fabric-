//! Desktop mode: `nf-server --desktop`. The same server, run for one person on
//! their own computer. It is strictly opt-in and only ADDS a loopback path.
//!
//! - The data lives in the per-user application-data folder.
//! - The server listens on 127.0.0.1 only and answers to 127.0.0.1 / localhost.
//! - The first-run setup token is generated here and handed to the browser in
//!   the URL fragment (`#setup=...`). A fragment is never sent to the server
//!   and never written to its log; the client reads it once and removes it
//!   from the address bar. The token is only ever stored as its SHA-256 hash
//!   by the server, and the server forgets it when the first administrator is
//!   created (the same rule as server mode).
//!
//! The program makes no network calls of its own and never updates itself.

use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Preferred port when `--port` is not given. Used if free; otherwise a free port is taken.
pub(crate) const DEFAULT_PORT: u16 = 8787;

pub(crate) const USAGE: &str = "usage: nf-server --desktop [--port N] [--no-browser]
  --port N       preferred port on 127.0.0.1 (default 8787; 0 = any free port).
                 If it is taken, a free port is used instead.
  --no-browser   do not open a browser; print the address (and, on first run,
                 the one-time setup link) in this window instead.";

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Parsed {
    Help,
    Run { port: u16, open_browser: bool },
}

/// Reads the arguments after the program name. `--desktop` must be among them; it is accepted and ignored here.
pub(crate) fn parse_args(args: &[String]) -> Result<Parsed, String> {
    let mut port = DEFAULT_PORT;
    let mut open_browser = true;
    let mut help = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--desktop" => {}
            "--no-browser" => open_browser = false,
            "--help" | "-h" => help = true,
            "--port" => {
                let v = it
                    .next()
                    .ok_or_else(|| "--port needs a number".to_string())?;
                port = parse_port(v)?;
            }
            other => {
                if let Some(v) = other.strip_prefix("--port=") {
                    port = parse_port(v)?;
                } else {
                    return Err(format!("unknown option '{other}'\n{USAGE}"));
                }
            }
        }
    }
    if help {
        return Ok(Parsed::Help);
    }
    Ok(Parsed::Run { port, open_browser })
}

fn parse_port(v: &str) -> Result<u16, String> {
    v.trim()
        .parse::<u16>()
        .map_err(|_| format!("--port: '{v}' is not a port number (0 to 65535)"))
}

/// The per-user data folder. Windows: %LOCALAPPDATA%\NexusFabric\data.
/// Elsewhere: $XDG_DATA_HOME/nexus-fabric (an absolute XDG_DATA_HOME), else ~/.local/share/nexus-fabric.
/// `get` reads an environment variable; it is a parameter so the rule can be tested.
pub(crate) fn data_dir_for(
    windows: bool,
    get: &dyn Fn(&str) -> Option<String>,
) -> Result<PathBuf, String> {
    let present = |k: &str| {
        get(k)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    if windows {
        let base = match present("LOCALAPPDATA") {
            Some(l) => PathBuf::from(l),
            None => match present("USERPROFILE") {
                Some(u) => PathBuf::from(u).join("AppData").join("Local"),
                None => {
                    return Err(
                        "cannot find the application-data folder: LOCALAPPDATA is not set".into(),
                    );
                }
            },
        };
        return Ok(base.join("NexusFabric").join("data"));
    }
    let xdg = present("XDG_DATA_HOME").filter(|v| v.starts_with('/'));
    let base = match xdg {
        Some(x) => PathBuf::from(x),
        None => match present("HOME").filter(|v| v.starts_with('/')) {
            Some(h) => PathBuf::from(h).join(".local").join("share"),
            None => return Err("cannot find the data folder: HOME is not set".into()),
        },
    };
    Ok(base.join("nexus-fabric"))
}

pub(crate) fn current_data_dir() -> Result<PathBuf, String> {
    data_dir_for(cfg!(windows), &|k| std::env::var(k).ok())
}

/// Creates the data folder; on Unix it is owner-only (0700). On Windows the folder
/// inherits the per-user ACL of %LOCALAPPDATA%.
fn prepare_data_dir(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir)
        .map_err(|e| format!("cannot create the data folder {}: {e}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("cannot restrict the data folder {}: {e}", dir.display()))?;
    }
    Ok(())
}

/// Takes an exclusive lock on `<data>/desktop.lock` for the life of the process.
/// The operating system releases it when the process ends, even after a crash.
fn lock_data_dir(dir: &Path) -> Result<fs::File, String> {
    let path = dir.join("desktop.lock");
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(fs::TryLockError::WouldBlock) => Err(format!(
            "NEXUS FABRIC is already running on the data folder {}. Use the copy that is open (its browser tab), or stop it with Ctrl+C in its console window or by closing that window, and then start again. A second copy on the same data folder is refused.",
            dir.display()
        )),
        Err(fs::TryLockError::Error(e)) => Err(format!("cannot lock {}: {e}", path.display())),
    }
}

/// A one-time setup token: 32 bytes from the operating system's random source, hex-encoded (64 characters).
pub(crate) fn new_setup_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|e| format!("the operating system random source is not available: {e}"))?;
    Ok(hex::encode(bytes))
}

/// Socket addresses to try, in order: the preferred port on 127.0.0.1, then any free port.
pub(crate) fn bind_candidates(preferred: u16) -> Vec<SocketAddr> {
    let lo = IpAddr::V4(Ipv4Addr::LOCALHOST);
    if preferred == 0 {
        vec![SocketAddr::new(lo, 0)]
    } else {
        vec![SocketAddr::new(lo, preferred), SocketAddr::new(lo, 0)]
    }
}

/// The address the browser opens. With a token it carries the token in the fragment only.
pub(crate) fn start_url(port: u16, setup_token: Option<&str>) -> String {
    match setup_token {
        Some(t) => format!("http://127.0.0.1:{port}/#setup={t}"),
        None => format!("http://127.0.0.1:{port}/"),
    }
}

/// Opens the default browser without waiting for it. Returns false if no opener could be started.
fn open_browser(url: &str) -> bool {
    let opener = if cfg!(windows) {
        "explorer.exe"
    } else {
        "xdg-open"
    };
    Command::new(opener).arg(url).spawn().is_ok()
}

/// Runs desktop mode until Ctrl+C, SIGTERM or the console window closes.
pub(crate) async fn run(args: &[String]) -> Result<(), String> {
    let (port, want_browser) = match parse_args(args)? {
        Parsed::Help => {
            println!("{USAGE}");
            return Ok(());
        }
        Parsed::Run { port, open_browser } => (port, open_browser),
    };
    let data_dir = current_data_dir()?;
    prepare_data_dir(&data_dir)?;
    let _lock = lock_data_dir(&data_dir)?;
    let token = new_setup_token()?;
    let cfg = crate::Config {
        key_path: data_dir.join("storage.key"),
        data_dir: data_dir.clone(),
        cookie_secure: false,
        allowed_hosts: vec!["127.0.0.1".into(), "localhost".into()],
        trusted_proxies: Vec::new(),
        setup_token: Some(token.clone()),
        request_timeout: Duration::from_secs(120),
    };
    // The same limits as server mode, without reading the NF_* server settings.
    let limits = crate::Limits {
        header_read: Duration::from_secs(15),
        max_connections: 512,
        shutdown_grace: Duration::from_secs(20),
    };
    let app = crate::build_app(cfg)?;
    let needs_setup = app
        .setup_token
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some();
    let mut bound = None;
    for addr in bind_candidates(port) {
        if let Ok(l) = tokio::net::TcpListener::bind(addr).await {
            bound = Some(l);
            break;
        }
    }
    let listener =
        bound.ok_or_else(|| "cannot listen on 127.0.0.1: no port could be bound".to_string())?;
    let bound_port = listener
        .local_addr()
        .map_err(|e| format!("cannot read the listening address: {e}"))?
        .port();

    println!(
        "NEXUS FABRIC desktop mode. Development build. Not validated. Outputs must not be used for manufacturing."
    );
    if port != 0 && bound_port != port {
        println!("Port {port} is in use; using port {bound_port} instead.");
    }
    println!("Address: http://127.0.0.1:{bound_port}/  (this computer only)");
    println!("Data folder: {}", data_dir.display());
    println!(
        "Keep this window open while you work. Press Ctrl+C or close the window to stop; the database is then closed cleanly."
    );

    let url = start_url(bound_port, needs_setup.then_some(token.as_str()));
    let opened = want_browser && open_browser(&url);
    if want_browser && !opened {
        println!("Could not start a browser. Open the address above yourself.");
    }
    if needs_setup && (!opened) {
        // Without a browser the setup link, with its one-time token, is printed so it can be used once.
        println!(
            "First run: no administrator exists yet. Setup link (one time, keep it private): {url}"
        );
    } else if needs_setup {
        println!(
            "First run: the browser opened the setup page. The setup token is handed over there and is not shown here."
        );
    }
    drop(token);

    let keeper = app.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(3600));
        loop {
            tick.tick().await;
            if let Err(e) = crate::housekeeping(&keeper) {
                eprintln!("nf-server: housekeeping failed: {e}");
            }
        }
    });
    crate::serve(listener, crate::router(app.clone()), limits, true).await?;
    if let Err(e) = app.db().execute_batch("PRAGMA wal_checkpoint(TRUNCATE)") {
        eprintln!("nf-server: could not checkpoint the database at shutdown: {e}");
    }
    drop(app);
    println!("NEXUS FABRIC stopped. The database is closed.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k: &str| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        }
    }

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn windows_data_dir_is_under_localappdata() {
        let get = env(&[("LOCALAPPDATA", r"C:\Users\ann\AppData\Local")]);
        assert_eq!(
            data_dir_for(true, &get).unwrap(),
            PathBuf::from(r"C:\Users\ann\AppData\Local")
                .join("NexusFabric")
                .join("data")
        );
    }

    #[test]
    fn windows_data_dir_falls_back_to_userprofile() {
        let get = env(&[("USERPROFILE", r"C:\Users\ann")]);
        assert_eq!(
            data_dir_for(true, &get).unwrap(),
            PathBuf::from(r"C:\Users\ann")
                .join("AppData")
                .join("Local")
                .join("NexusFabric")
                .join("data")
        );
        assert!(data_dir_for(true, &env(&[])).is_err());
    }

    #[test]
    fn linux_data_dir_uses_xdg_or_home() {
        let xdg = env(&[("XDG_DATA_HOME", "/srv/xdg"), ("HOME", "/home/ann")]);
        assert_eq!(
            data_dir_for(false, &xdg).unwrap(),
            PathBuf::from("/srv/xdg/nexus-fabric")
        );
        let home = env(&[("HOME", "/home/ann")]);
        assert_eq!(
            data_dir_for(false, &home).unwrap(),
            PathBuf::from("/home/ann/.local/share/nexus-fabric")
        );
    }

    #[test]
    fn relative_xdg_data_home_is_ignored() {
        let get = env(&[("XDG_DATA_HOME", "relative/dir"), ("HOME", "/home/ann")]);
        assert_eq!(
            data_dir_for(false, &get).unwrap(),
            PathBuf::from("/home/ann/.local/share/nexus-fabric")
        );
        assert!(data_dir_for(false, &env(&[])).is_err());
    }

    #[test]
    fn arguments_default_and_flags() {
        assert_eq!(
            parse_args(&args(&["--desktop"])).unwrap(),
            Parsed::Run {
                port: DEFAULT_PORT,
                open_browser: true
            }
        );
        assert_eq!(
            parse_args(&args(&["--desktop", "--port", "8861", "--no-browser"])).unwrap(),
            Parsed::Run {
                port: 8861,
                open_browser: false
            }
        );
        assert_eq!(
            parse_args(&args(&["--desktop", "--port=0"])).unwrap(),
            Parsed::Run {
                port: 0,
                open_browser: true
            }
        );
        assert_eq!(
            parse_args(&args(&["--desktop", "--help"])).unwrap(),
            Parsed::Help
        );
    }

    #[test]
    fn arguments_reject_bad_input() {
        assert!(parse_args(&args(&["--desktop", "--port"])).is_err());
        assert!(parse_args(&args(&["--desktop", "--port", "70000"])).is_err());
        assert!(parse_args(&args(&["--desktop", "--port=abc"])).is_err());
        let e = parse_args(&args(&["--desktop", "--open-everywhere"])).unwrap_err();
        assert!(e.contains("unknown option"));
    }

    #[test]
    fn bind_plan_prefers_the_port_then_any_free_port() {
        let plan = bind_candidates(8787);
        assert_eq!(plan.len(), 2);
        assert_eq!(plan[0], "127.0.0.1:8787".parse().unwrap());
        assert_eq!(plan[1], "127.0.0.1:0".parse().unwrap());
        assert_eq!(
            bind_candidates(0),
            vec!["127.0.0.1:0".parse::<SocketAddr>().unwrap()]
        );
        assert!(plan.iter().all(|a| a.ip().is_loopback()));
    }

    #[test]
    fn setup_token_is_long_hex_and_fresh() {
        let a = new_setup_token().unwrap();
        let b = new_setup_token().unwrap();
        assert!(a.len() >= 32 && a.len() == 64);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn start_url_carries_the_token_only_in_the_fragment() {
        assert_eq!(start_url(8860, None), "http://127.0.0.1:8860/");
        assert_eq!(
            start_url(8860, Some("abc")),
            "http://127.0.0.1:8860/#setup=abc"
        );
    }

    #[test]
    fn second_copy_on_the_same_folder_is_refused() {
        let dir = std::env::temp_dir().join(format!("nf-desktop-lock-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let first = lock_data_dir(&dir).expect("first copy takes the lock");
        let second = lock_data_dir(&dir);
        assert!(second.is_err());
        assert!(second.unwrap_err().contains("already running"));
        drop(first);
        assert!(
            lock_data_dir(&dir).is_ok(),
            "the lock is released with its handle"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
