//! NEXUS FABRIC licence tool (owner-side; never shipped in a package).
//!
//! keygen / pubkey / issue / inspect for the offline Ed25519 licence system
//! (see the nf-licence library). Hand-rolled argument parsing in the style of
//! nf-server's `invocation()`: an argument that is not understood is an error
//! (exit 2); operational failures exit 1. The private seed is never printed.
//! Key files are created owner-only (0600) and never overwritten, and the
//! tool refuses key paths inside an nf-server data folder.

use nf_licence::{
    evaluate, key_id, public_key_from_seed, sign, verify_with_keys, Edition, Payload, DAY,
    VERIFY_KEYS,
};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

/// What the program does, decided from its command-line arguments.
enum Command {
    Keygen {
        out: PathBuf,
    },
    Pubkey {
        key_file: PathBuf,
    },
    Issue {
        key_file: PathBuf,
        licence_id: String,
        customer: String,
        seats: u32,
        edition: Edition,
        valid_from: u64,
        valid_until: u64,
        issuer: String,
        out: PathBuf,
    },
    Inspect {
        file: PathBuf,
    },
    Help,
    Version,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match parse(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    match execute(command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("nf-licence: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Decides what the arguments ask for. Nothing is created here: help, version
/// and an unknown argument never touch a file.
fn parse(args: &[String]) -> Result<Command, String> {
    let mut it = args.iter();
    let Some(sub) = it.next() else {
        return Err(usage_error("no command given"));
    };
    match sub.as_str() {
        "--help" | "-h" => return Ok(Command::Help),
        "--version" => return Ok(Command::Version),
        "keygen" | "pubkey" | "issue" | "inspect" => {}
        other => return Err(usage_error(&format!("unknown command '{other}'"))),
    }
    if sub == "inspect" {
        // One positional licence file path, no options.
        let Some(file) = it.next() else {
            return Err(usage_error("inspect needs a licence file path"));
        };
        if file == "--help" || file == "-h" {
            return Ok(Command::Help);
        }
        if file.starts_with('-') || it.next().is_some() {
            return Err(usage_error("inspect takes exactly one licence file path"));
        }
        return Ok(Command::Inspect {
            file: PathBuf::from(file),
        });
    }
    // Collect `--flag value` pairs for the subcommand.
    let mut values: Vec<(&str, String)> = Vec::new();
    while let Some(arg) = it.next() {
        if arg == "--help" || arg == "-h" {
            return Ok(Command::Help);
        }
        let Some(flag) = arg.strip_prefix("--") else {
            return Err(usage_error(&format!("unknown argument '{arg}'")));
        };
        let Some(value) = it.next() else {
            return Err(usage_error(&format!("{arg} needs a value")));
        };
        values.push((flag, value.clone()));
    }
    let take = |flag: &str, values: &mut Vec<(&str, String)>| -> Option<String> {
        values
            .iter()
            .position(|(f, _)| *f == flag)
            .map(|i| values.remove(i).1)
    };
    let require = |flag: &str, values: &mut Vec<(&str, String)>| -> Result<String, String> {
        take(flag, values).ok_or_else(|| usage_error(&format!("--{flag} is required")))
    };
    let command = match sub.as_str() {
        "keygen" => Command::Keygen {
            out: PathBuf::from(require("out", &mut values)?),
        },
        "pubkey" => Command::Pubkey {
            key_file: PathBuf::from(require("key-file", &mut values)?),
        },
        "issue" => {
            let seats_raw = require("seats", &mut values)?;
            let seats: u32 = seats_raw.parse().map_err(|_| {
                usage_error(&format!(
                    "--seats must be a positive integer, got '{seats_raw}'"
                ))
            })?;
            let edition_raw =
                take("edition", &mut values).unwrap_or_else(|| "standard".to_string());
            let edition = parse_edition(&edition_raw)?;
            let valid_from = parse_date(&require("valid-from", &mut values)?)?;
            let valid_until = parse_date(&require("valid-until", &mut values)?)?;
            Command::Issue {
                key_file: PathBuf::from(require("key-file", &mut values)?),
                licence_id: require("licence-id", &mut values)?,
                customer: require("customer", &mut values)?,
                seats,
                edition,
                valid_from,
                valid_until,
                issuer: take("issuer", &mut values).unwrap_or_else(|| "Chuo-Sogo".to_string()),
                out: PathBuf::from(require("out", &mut values)?),
            }
        }
        _ => return Err(usage_error(&format!("unknown command '{sub}'"))),
    };
    if let Some((flag, _)) = values.first() {
        return Err(usage_error(&format!("unknown option '--{flag}' for {sub}")));
    }
    Ok(command)
}

fn usage_error(message: &str) -> String {
    format!("nf-licence: {message}; run nf-licence --help for the usage")
}

fn parse_edition(s: &str) -> Result<Edition, String> {
    match s {
        "pilot" => Ok(Edition::Pilot),
        "standard" => Ok(Edition::Standard),
        "site" => Ok(Edition::Site),
        other => Err(usage_error(&format!(
            "unknown edition '{other}'; use pilot, standard or site"
        ))),
    }
}

fn edition_str(edition: Edition) -> &'static str {
    match edition {
        Edition::Pilot => "pilot",
        Edition::Standard => "standard",
        Edition::Site => "site",
    }
}

fn execute(command: Command) -> Result<(), String> {
    match command {
        Command::Help => {
            print!("{}", help_text());
            Ok(())
        }
        Command::Version => {
            println!("nf-licence {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Keygen { out } => cmd_keygen(&out),
        Command::Pubkey { key_file } => cmd_pubkey(&key_file),
        Command::Issue {
            key_file,
            licence_id,
            customer,
            seats,
            edition,
            valid_from,
            valid_until,
            issuer,
            out,
        } => cmd_issue(
            &key_file,
            &licence_id,
            &customer,
            seats,
            edition,
            valid_from,
            valid_until,
            &issuer,
            &out,
        ),
        Command::Inspect { file } => cmd_inspect(&file),
    }
}

fn help_text() -> String {
    format!(
        "NEXUS FABRIC licence tool, version {}. Owner-side only; never shipped in a package.

usage:
  nf-licence keygen --out <path>
      Create a new 32-byte private seed file (owner-only, 0600, never overwritten)
      and print its public key and key_id. The seed is never printed. Keep the
      file offline, outside any repository or nf-server data folder.
  nf-licence pubkey --key-file <path>
      Print the public key and key_id of a seed file, for compiling into VERIFY_KEYS.
  nf-licence issue --key-file <path> --licence-id <id> --customer <name> --seats <n>
                   [--edition pilot|standard|site] --valid-from <date> --valid-until <date>
                   [--issuer <name>] --out <file>
      Sign and write a licence file. Dates are YYYY-MM-DD (UTC midnight) or unix
      seconds. Defaults: edition standard, issuer Chuo-Sogo.
  nf-licence inspect <file>
      Verify a licence file against the compiled public keys and print its fields
      and current state.
  nf-licence --help, -h       show this help and exit
  nf-licence --version        show the version and exit
An unknown command, option or missing value is an error (exit 2); a failed
operation exits 1. Key paths inside an nf-server data folder are refused.

The public key compiled into this build is a development key; the owner replaces
it with the real key before any sale.",
        env!("CARGO_PKG_VERSION")
    )
}

/// Current unix time, seconds.
fn now_unix() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| format!("the system clock is before the unix epoch: {e}"))
}

/// Refuse key paths inside an nf-server data folder: the folder named by
/// NF_DATA_DIR, or any folder carrying the installation marker that
/// nf-server's storage writes. The licence key must live outside the data it
/// protects.
fn refuse_data_dir(path: &Path) -> Result<(), String> {
    let full =
        std::path::absolute(path).map_err(|e| format!("cannot resolve {}: {e}", path.display()))?;
    if let Ok(data_dir) = std::env::var("NF_DATA_DIR") {
        let data = std::path::absolute(&data_dir)
            .map_err(|e| format!("cannot resolve NF_DATA_DIR '{data_dir}': {e}"))?;
        if full.starts_with(&data) {
            return Err(format!(
                "refusing the key path {} inside the nf-server data folder '{data_dir}'; keep licence keys outside it",
                path.display()
            ));
        }
    }
    for ancestor in full.ancestors() {
        if ancestor.join(".installation-marker").exists() {
            return Err(format!(
                "refusing the key path {} inside an nf-server data folder ({} carries the installation marker); keep licence keys outside it",
                path.display(),
                ancestor.display()
            ));
        }
    }
    Ok(())
}

/// A new private seed file: 32 bytes from the operating system's random
/// source, owner-only (0600), created fresh (an existing file is never
/// overwritten). Mirrors nf-server's storage key creation.
fn create_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    std::io::Write::write_all(&mut f, bytes)?;
    f.sync_all()?;
    #[cfg(unix)]
    if let Some(dir) = path.parent() {
        std::fs::File::open(dir)?.sync_all()?;
    }
    Ok(())
}

fn read_seed(key_file: &Path) -> Result<[u8; 32], String> {
    let bytes = std::fs::read(key_file)
        .map_err(|e| format!("cannot read the key file {}: {e}", key_file.display()))?;
    let bytes_len = bytes.len();
    let seed: [u8; 32] = bytes.try_into().map_err(|_| {
        format!(
            "the key file {} is not a 32-byte seed ({bytes_len} bytes)",
            key_file.display()
        )
    })?;
    Ok(seed)
}

fn cmd_keygen(out: &Path) -> Result<(), String> {
    refuse_data_dir(out)?;
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed)
        .map_err(|e| format!("the operating system random source is not available: {e}"))?;
    create_private(out, &seed)
        .map_err(|e| format!("cannot create the key file {}: {e}", out.display()))?;
    let public = public_key_from_seed(&seed);
    println!("public key: {}", hex::encode(public));
    println!("key_id: {}", key_id(&public));
    println!("The seed file is owner-only (0600). Keep it offline and outside any repository or data folder; it is never printed.");
    Ok(())
}

fn cmd_pubkey(key_file: &Path) -> Result<(), String> {
    refuse_data_dir(key_file)?;
    let seed = read_seed(key_file)?;
    let public = public_key_from_seed(&seed);
    println!("public key: {}", hex::encode(public));
    println!("key_id: {}", key_id(&public));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_issue(
    key_file: &Path,
    licence_id: &str,
    customer: &str,
    seats: u32,
    edition: Edition,
    valid_from: u64,
    valid_until: u64,
    issuer: &str,
    out: &Path,
) -> Result<(), String> {
    refuse_data_dir(key_file)?;
    if seats == 0 {
        return Err("seats must be at least 1".to_string());
    }
    if valid_until <= valid_from {
        return Err("valid-until must be after valid-from".to_string());
    }
    let seed = read_seed(key_file)?;
    let public = public_key_from_seed(&seed);
    let kid = key_id(&public);
    let payload = Payload {
        v: 1,
        licence_id: licence_id.to_string(),
        customer: customer.to_string(),
        seats,
        edition,
        valid_from,
        valid_until,
        issued_at: now_unix()?,
        issuer: issuer.to_string(),
        key_id: kid.clone(),
    };
    let envelope = sign(&payload, &seed)?;
    let mut bytes =
        serde_json::to_vec(&envelope).map_err(|e| format!("cannot serialise the licence: {e}"))?;
    bytes.push(b'\n');
    std::fs::write(out, &bytes)
        .map_err(|e| format!("cannot write the licence file {}: {e}", out.display()))?;
    println!(
        "wrote {} (licence_id {licence_id}, key_id {kid})",
        out.display()
    );
    if !VERIFY_KEYS.iter().any(|(id, _)| *id == kid) {
        println!("warning: key_id {kid} is not compiled into VERIFY_KEYS; inspect will refuse this licence until the public key is added in a release");
    }
    Ok(())
}

fn cmd_inspect(file: &Path) -> Result<(), String> {
    let bytes = std::fs::read(file)
        .map_err(|e| format!("cannot read the licence file {}: {e}", file.display()))?;
    let payload = verify_with_keys(&bytes, VERIFY_KEYS)
        .map_err(|r| format!("{} is not a valid licence: {r}", file.display()))?;
    println!("licence_id: {}", payload.licence_id);
    println!("customer: {}", payload.customer);
    println!("seats: {}", payload.seats);
    println!("edition: {}", edition_str(payload.edition));
    println!("valid_from: {}", payload.valid_from);
    println!("valid_until: {}", payload.valid_until);
    println!("issued_at: {}", payload.issued_at);
    println!("issuer: {}", payload.issuer);
    println!("key_id: {}", payload.key_id);
    let status = evaluate(Some(&payload), now_unix()?, 0, None, None);
    match status.state {
        nf_licence::State::Evaluation { days_left, note } => match note {
            Some(note) => println!("state: evaluation ({note}, {days_left} days left)"),
            None => println!("state: evaluation ({days_left} days left)"),
        },
        nf_licence::State::Valid => println!("state: valid"),
        nf_licence::State::Grace { grace_until } => {
            println!("state: grace (grace_until {grace_until})")
        }
        nf_licence::State::ReadOnly { reason } => println!("state: read_only ({reason})"),
    }
    Ok(())
}

/// Parse a date: unix seconds (all digits) or YYYY-MM-DD at UTC midnight.
fn parse_date(s: &str) -> Result<u64, String> {
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
        return s
            .parse::<u64>()
            .map_err(|_| usage_error(&format!("invalid unix timestamp '{s}'")));
    }
    let bad = || {
        usage_error(&format!(
            "invalid date '{s}'; use YYYY-MM-DD (UTC midnight) or unix seconds"
        ))
    };
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return Err(bad());
    }
    let year: i64 = parts[0].parse().map_err(|_| bad())?;
    let month: i64 = parts[1].parse().map_err(|_| bad())?;
    let day: i64 = parts[2].parse().map_err(|_| bad())?;
    if !(1970..=9999).contains(&year) || !(1..=12).contains(&month) {
        return Err(bad());
    }
    if day < 1 || day > days_in_month(year, month) {
        return Err(bad());
    }
    Ok(days_from_civil(year, month, day) as u64 * DAY)
}

/// Days in a month of the proleptic Gregorian calendar.
fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
            if leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Days from 1970-01-01 to year-month-day (Howard Hinnant's days_from_civil).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_from_civil_known_epochs() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(days_from_civil(2024, 2, 29), 19_782);
        assert_eq!(days_from_civil(2026, 1, 1), 20_454);
        assert_eq!(days_from_civil(2026, 11, 24), 20_781);
    }

    #[test]
    fn parse_date_valid() {
        assert_eq!(parse_date("1970-01-01").unwrap(), 0);
        assert_eq!(parse_date("2000-02-29").unwrap(), 951_782_400);
        assert_eq!(parse_date("2024-02-29").unwrap(), 1_709_164_800);
        assert_eq!(parse_date("2026-01-01").unwrap(), 1_767_225_600);
        assert_eq!(parse_date("2026-11-24").unwrap(), 1_795_478_400);
        assert_eq!(parse_date("1767225600").unwrap(), 1_767_225_600);
        assert_eq!(parse_date("0").unwrap(), 0);
    }

    #[test]
    fn parse_date_invalid() {
        for s in [
            "2026-02-29", // not a leap year
            "2100-02-29", // century, not a leap year
            "2026-13-01",
            "2026-00-10",
            "2026-01-00",
            "2026-01-32",
            "2026-04-31",
            "2026-1-01",
            "26-01-01",
            "2026-01-01-01",
            "1969-12-31",
            "not-a-date",
            "",
            "-5",
            "2026-0a-01",
        ] {
            assert!(parse_date(s).is_err(), "accepted invalid date '{s}'");
        }
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_usage_errors() {
        assert!(parse(&args(&[])).is_err());
        assert!(parse(&args(&["bogus"])).is_err());
        assert!(parse(&args(&["keygen"])).is_err()); // missing --out
        assert!(parse(&args(&["keygen", "--out"])).is_err()); // missing value
        assert!(parse(&args(&["keygen", "--out", "k", "--extra", "x"])).is_err());
        assert!(parse(&args(&["inspect"])).is_err());
        assert!(parse(&args(&["inspect", "a", "b"])).is_err());
        assert!(parse(&args(&["inspect", "--out", "a"])).is_err());
        assert!(parse(&args(&["pubkey", "--key-file", "k"])).is_ok());
        assert!(parse(&args(&["inspect", "licence.nflic"])).is_ok());
        assert!(parse(&args(&["--help"])).is_ok());
        assert!(parse(&args(&["keygen", "--help"])).is_ok());
        assert!(parse(&args(&["--version"])).is_ok());
    }

    #[test]
    fn parse_issue_defaults_and_dates() {
        let command = parse(&args(&[
            "issue",
            "--key-file",
            "k",
            "--licence-id",
            "L-1",
            "--customer",
            "Acme",
            "--seats",
            "2",
            "--valid-from",
            "2026-01-01",
            "--valid-until",
            "1769817600",
            "--out",
            "licence.nflic",
        ]))
        .unwrap();
        match command {
            Command::Issue {
                seats,
                edition,
                valid_from,
                valid_until,
                issuer,
                ..
            } => {
                assert_eq!(seats, 2);
                assert_eq!(edition, Edition::Standard);
                assert_eq!(valid_from, 1_767_225_600);
                assert_eq!(valid_until, 1_769_817_600);
                assert_eq!(issuer, "Chuo-Sogo");
            }
            _ => panic!("expected issue"),
        }
        // Bad seats / edition / dates are usage errors.
        for extra in [
            vec!["--seats", "x"],
            vec!["--seats", "-1"],
            vec!["--edition", "enterprise"],
            vec!["--valid-from", "2026-02-30"],
        ] {
            let mut list = args(&[
                "issue",
                "--key-file",
                "k",
                "--licence-id",
                "L-1",
                "--customer",
                "Acme",
                "--seats",
                "2",
                "--valid-from",
                "2026-01-01",
                "--valid-until",
                "2026-02-01",
                "--out",
                "o",
            ]);
            let flag = extra[0];
            if let Some(i) = list.iter().position(|a| a == flag) {
                list[i + 1] = extra[1].to_string();
            } else {
                list.extend(extra.iter().map(|s| s.to_string()));
            }
            assert!(parse(&list).is_err(), "accepted {extra:?}");
        }
    }

    #[test]
    fn refuse_data_dir_marker() {
        let dir = std::env::temp_dir().join(format!("nf-licence-marker-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Outside any data folder: accepted.
        assert!(refuse_data_dir(&dir.join("seed.key")).is_ok());
        // The installation marker makes it an nf-server data folder.
        std::fs::write(dir.join(".installation-marker"), b"test").unwrap();
        assert!(refuse_data_dir(&dir.join("seed.key")).is_err());
        assert!(refuse_data_dir(&dir.join("nested").join("seed.key")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
