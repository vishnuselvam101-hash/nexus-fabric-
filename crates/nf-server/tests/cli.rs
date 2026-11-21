//! The server binary's command line. Help, version and an unknown argument are answered at once:
//! they exit with the documented status and create nothing (no data folder, key or setup token) and
//! open no port. The data locations are pointed at a folder that does not exist yet, so a regression
//! that starts the server would show up as a created folder and a listening port.

use std::fs;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

struct Run {
    status: i32,
    stdout: String,
    stderr: String,
    data_dir: PathBuf,
    xdg_data_dir: PathBuf,
    listen: SocketAddr,
}

/// Runs the binary with `args` and waits for it to exit. A program still running after 20 seconds
/// is killed and the test fails: a server would never exit on its own.
fn run(name: &str, args: &[&str]) -> Run {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("cli-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let data_dir = root.join("data-does-not-exist");
    let home = root.join("home");
    let xdg_data_dir = root.join("xdg");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&xdg_data_dir).unwrap();
    // A port that was free a moment ago. Nothing should ever answer on it.
    let listen = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let out = root.join("stdout.txt");
    let err = root.join("stderr.txt");
    let mut child = Command::new(env!("CARGO_BIN_EXE_nf-server"))
        .args(args)
        .env("NF_DATA_DIR", &data_dir)
        .env("NF_LISTEN", listen.to_string())
        .env("HOME", &home)
        .env("XDG_DATA_HOME", &xdg_data_dir)
        .stdin(Stdio::null())
        .stdout(fs::File::create(&out).unwrap())
        .stderr(fs::File::create(&err).unwrap())
        .spawn()
        .expect("the server binary starts");
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("nf-server {args:?} is still running after 20 s; it should have exited");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    Run {
        status: status.code().unwrap_or(-1),
        stdout: fs::read_to_string(&out).unwrap_or_default(),
        stderr: fs::read_to_string(&err).unwrap_or_default(),
        data_dir,
        xdg_data_dir,
        listen,
    }
}

fn nothing_created_or_listening(r: &Run) {
    assert!(
        !r.data_dir.exists(),
        "the data folder was created: {}",
        r.data_dir.display()
    );
    assert!(
        fs::read_dir(&r.xdg_data_dir).unwrap().next().is_none(),
        "a desktop data folder was created under {}",
        r.xdg_data_dir.display()
    );
    assert!(
        TcpStream::connect_timeout(&r.listen, Duration::from_millis(300)).is_err(),
        "something listens on {}",
        r.listen
    );
}

#[test]
fn help_prints_usage_and_exits_zero_without_side_effects() {
    for flag in ["--help", "-h"] {
        let r = run(&format!("help{flag}"), &[flag]);
        assert_eq!(r.status, 0, "stderr: {}", r.stderr);
        for text in [
            "NF_LISTEN",
            "NF_DATA_DIR",
            "NF_KEY_FILE",
            "NF_SETUP_TOKEN",
            "--desktop",
            "--no-browser",
            "Development build. Not validated",
        ] {
            assert!(
                r.stdout.contains(text),
                "{flag}: help does not mention {text}"
            );
        }
        assert!(
            r.stderr.is_empty(),
            "{flag}: unexpected stderr: {}",
            r.stderr
        );
        nothing_created_or_listening(&r);
    }
}

#[test]
fn desktop_help_prints_the_desktop_options_and_exits_zero() {
    let r = run("desktop-help", &["--desktop", "--help"]);
    assert_eq!(r.status, 0, "stderr: {}", r.stderr);
    assert!(r.stdout.contains("--port N"), "stdout: {}", r.stdout);
    assert!(r.stdout.contains("NF_DATA_DIR"));
    nothing_created_or_listening(&r);
}

#[test]
fn version_prints_the_version_and_exits_zero_without_side_effects() {
    let r = run("version", &["--version"]);
    assert_eq!(r.status, 0, "stderr: {}", r.stderr);
    let first = r.stdout.lines().next().unwrap_or_default();
    assert!(first.starts_with("nf-server 0."), "first line: {first}");
    assert!(first.contains("nf-engine"), "first line: {first}");
    assert!(r.stdout.contains("Not validated"));
    nothing_created_or_listening(&r);
}

#[test]
fn unknown_argument_exits_two_and_starts_nothing() {
    let r = run("unknown", &["--open-everywhere"]);
    assert_eq!(r.status, 2);
    assert!(
        r.stderr.contains("unknown argument '--open-everywhere'"),
        "stderr: {}",
        r.stderr
    );
    nothing_created_or_listening(&r);
}

#[test]
fn help_next_to_an_unknown_argument_exits_two() {
    let r = run("help-unknown", &["--help", "--bogus"]);
    assert_eq!(r.status, 2);
    assert!(r.stdout.is_empty());
    nothing_created_or_listening(&r);
}

#[test]
fn desktop_options_without_desktop_exit_two() {
    for args in [&["--port", "8861"][..], &["--no-browser"][..]] {
        let r = run("desktop-option", args);
        assert_eq!(r.status, 2, "{args:?}");
        assert!(
            r.stderr.contains("needs --desktop"),
            "{args:?}: stderr: {}",
            r.stderr
        );
        nothing_created_or_listening(&r);
    }
}

#[test]
fn unknown_desktop_option_exits_two_before_the_data_folder_is_made() {
    let r = run("desktop-unknown", &["--desktop", "--open-everywhere"]);
    assert_eq!(r.status, 2);
    assert!(
        r.stderr.contains("unknown argument '--open-everywhere'"),
        "stderr: {}",
        r.stderr
    );
    nothing_created_or_listening(&r);
}

#[test]
fn bad_desktop_port_exits_two_before_the_data_folder_is_made() {
    let r = run("desktop-port", &["--desktop", "--port", "70000"]);
    assert_eq!(r.status, 2);
    assert!(r.stderr.contains("70000"), "stderr: {}", r.stderr);
    nothing_created_or_listening(&r);
}
