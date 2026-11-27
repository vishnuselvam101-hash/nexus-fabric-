//! Isolation of the worker process, applied before it reads any input.
//!
//! Linux, in this order:
//! 1. the worker is ended with SIGKILL if its parent ends (PR_SET_PDEATHSIG);
//! 2. resource limits (setrlimit): address space, CPU time, file size, open files, no core file;
//! 3. Landlock, where the kernel has it: every file-system access right, TCP (and, on kernels
//!    with Landlock ABI 10, UDP) bind and connect, abstract UNIX sockets and signals to other
//!    processes are handled and no rule grants any of them, so the worker can open no file or
//!    directory, by name, at all;
//! 4. a seccomp allow-list: only the system calls the worker needs on its open descriptors and
//!    for memory are allowed (mmap and mprotect only without PROT_EXEC, tgkill only to itself);
//!    every other call, among them socket, socketpair, open, openat, execve, clone, ptrace and
//!    io_uring, fails with EPERM. Calls of another architecture's ABI end the process.
//!
//! Steps 1, 2 and 4 are required: if one fails the worker stops with an internal error. Step 3
//! is used where the kernel supports it; on a kernel without Landlock the allow-list of step 4
//! still refuses every file-system and socket call, without the second layer.
//!
//! Other Unix systems: step 2 only. Windows: a Job Object (memory, CPU time, one process; see the
//! `nf-pdf-winjob` crate); there is no network and no file-system isolation on Windows.
//!
//! Every number here is an implementation limit of this program, not an engineering value.

use std::path::Path;

/// Address-space limit of the worker (RLIMIT_AS; on Windows the process memory limit).
pub const ADDRESS_SPACE_BYTES: u64 = 3 << 30;
/// CPU-time limit of the worker, seconds (RLIMIT_CPU; on Windows user-mode time).
pub const CPU_SECONDS: u64 = 120;
/// Largest file the worker may write (RLIMIT_FSIZE). It writes none; its answer goes to a pipe.
#[cfg(unix)]
pub const FILE_SIZE_BYTES: u64 = 256 << 20;
/// Open descriptors of the worker (RLIMIT_NOFILE).
#[cfg(unix)]
pub const OPEN_FILES: u64 = 16;

/// One entry of the isolation summary written into the report.
pub type Summary = Vec<(&'static str, String)>;

#[cfg(unix)]
const LIMITS_TEXT: &str =
    "address space 3 GiB, CPU time 120 s, file size 256 MiB, 16 open files, no core file";

/// The signal that RLIMIT_CPU sends.
#[cfg(unix)]
pub fn is_cpu_limit_signal(sig: i32) -> bool {
    sig == rustix::process::Signal::XCPU.as_raw()
}

/// A short explanation of a signal that ended the worker.
#[cfg(unix)]
pub fn signal_hint(sig: i32) -> &'static str {
    use rustix::process::Signal;
    if sig == Signal::ABORT.as_raw() {
        " (SIGABRT: for example a memory allocation refused by the address-space limit)"
    } else if sig == Signal::KILL.as_raw() {
        " (SIGKILL)"
    } else if sig == Signal::SEGV.as_raw() {
        " (SIGSEGV)"
    } else if sig == Signal::SYS.as_raw() {
        " (SIGSYS: a system call was refused by a system-call filter of the calling service; see docs/backend/RASTER.md)"
    } else {
        ""
    }
}

/// Attempts of forbidden operations after each isolation step (`--isolation-probe`), one line
/// on stderr per attempt. "refused (EACCES)" or "refused (EPERM)" is the expected result;
/// "ALLOWED" or any other error means the step did not stop the attempt.
pub struct Probe {
    on: bool,
    #[cfg(unix)]
    input: std::path::PathBuf,
    #[cfg(unix)]
    output: std::path::PathBuf,
    /// Whether /etc/hostname exists, looked up before any isolation step.
    #[cfg(unix)]
    hostname: bool,
}

impl Probe {
    pub fn new(on: bool, input: &Path, output: &Path) -> Self {
        #[cfg(not(unix))]
        let _ = (input, output);
        Probe {
            on,
            #[cfg(unix)]
            input: input.to_path_buf(),
            #[cfg(unix)]
            output: output.to_path_buf(),
            #[cfg(unix)]
            hostname: on && Path::new("/etc/hostname").is_file(),
        }
    }

    fn line(&self, stage: &str, check: &str, result: &str) {
        if self.on {
            eprintln!("nf-raster probe: {stage}: {check}: {result}");
        }
    }

    #[cfg(unix)]
    fn attempt<T>(&self, stage: &str, check: &str, r: std::io::Result<T>) {
        const EPERM: i32 = 1;
        const EACCES: i32 = 13;
        let text = match r {
            Ok(_) => "ALLOWED".to_string(),
            Err(e) => match e.raw_os_error() {
                Some(EPERM) => "refused (EPERM)".to_string(),
                Some(EACCES) => "refused (EACCES)".to_string(),
                _ => format!("error ({e})"),
            },
        };
        self.line(stage, check, &text);
    }

    #[cfg(unix)]
    fn limits(&self) {
        use rustix::process::{getrlimit, Resource};
        if !self.on {
            return;
        }
        let show = |v: Option<u64>| v.map_or("unlimited".to_string(), |n| n.to_string());
        for (name, res) in [
            ("AS", Resource::As),
            ("CPU", Resource::Cpu),
            ("FSIZE", Resource::Fsize),
            ("NOFILE", Resource::Nofile),
            ("CORE", Resource::Core),
        ] {
            let l = getrlimit(res);
            self.line(
                "limits",
                &format!("rlimit {name}"),
                &format!("{}/{}", show(l.current), show(l.maximum)),
            );
        }
    }

    /// Opening files by name: /etc/hostname, the worker's own input and output, and a new
    /// file beside the output (removed again if the attempt succeeded).
    #[cfg(unix)]
    fn files(&self, stage: &str) {
        use std::fs::{File, OpenOptions};
        if !self.on {
            return;
        }
        if self.hostname {
            self.attempt(stage, "read /etc/hostname", File::open("/etc/hostname"));
        } else {
            self.line(stage, "read /etc/hostname", "absent on this system");
        }
        self.attempt(stage, "read the input by name", File::open(&self.input));
        self.attempt(
            stage,
            "write the output by name",
            OpenOptions::new().write(true).open(&self.output),
        );
        let beside = self
            .output
            .parent()
            .unwrap_or(Path::new("."))
            .join(format!("nf-raster-probe-{}.tmp", std::process::id()));
        let created = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&beside);
        if created.is_ok() {
            let _ = std::fs::remove_file(&beside);
        }
        self.attempt(stage, "create a file beside the output", created);
    }

    #[cfg(unix)]
    fn exec(&self, stage: &str) {
        if self.on {
            let r = std::process::Command::new("/bin/true").env_clear().status();
            self.attempt(stage, "run /bin/true", r);
        }
    }

    #[cfg(target_os = "linux")]
    fn after_landlock(&self, status: &str) {
        if !self.on {
            return;
        }
        self.line("landlock", "status", status);
        self.files("landlock");
        self.attempt(
            "landlock",
            "tcp connect 127.0.0.1:9",
            std::net::TcpStream::connect(("127.0.0.1", 9)),
        );
        self.exec("landlock");
    }

    #[cfg(target_os = "linux")]
    fn after_seccomp(&self) {
        use std::os::unix::net::{UnixDatagram, UnixStream};
        if !self.on {
            return;
        }
        self.attempt(
            "seccomp",
            "socket inet tcp",
            std::net::TcpListener::bind(("127.0.0.1", 0)),
        );
        self.attempt(
            "seccomp",
            "socket inet6 udp",
            std::net::UdpSocket::bind(("::1", 0)),
        );
        self.attempt("seccomp", "socket unix", UnixDatagram::unbound());
        self.attempt("seccomp", "socketpair unix", UnixStream::pair());
        self.files("seccomp");
        self.exec("seccomp");
    }
}

/// Isolates the worker; returns what was applied, for the report.
#[cfg(target_os = "linux")]
pub fn isolate(parent: Option<u32>, probe: &mut Probe) -> Result<Summary, String> {
    tie_to_parent(parent)?;
    let mut summary: Summary = vec![("platform", "linux".to_string())];
    resource_limits()?;
    probe.limits();
    summary.push(("resource_limits", LIMITS_TEXT.to_string()));
    let landlock = landlock()?;
    probe.after_landlock(&landlock);
    summary.push(("landlock", landlock));
    seccomp()?;
    probe.after_seccomp();
    summary.push((
        "seccomp",
        "allow-list enforced: every other system call fails with EPERM (no socket, no file opened by name, no new process)".to_string(),
    ));
    Ok(summary)
}

#[cfg(all(unix, not(target_os = "linux")))]
pub fn isolate(_parent: Option<u32>, probe: &mut Probe) -> Result<Summary, String> {
    resource_limits()?;
    probe.limits();
    Ok(vec![
        ("platform", std::env::consts::OS.to_string()),
        ("resource_limits", LIMITS_TEXT.to_string()),
        ("network", "not isolated on this system".to_string()),
        ("file_system", "not isolated on this system".to_string()),
    ])
}

#[cfg(windows)]
pub fn isolate(_parent: Option<u32>, probe: &mut Probe) -> Result<Summary, String> {
    let memory = usize::try_from(ADDRESS_SPACE_BYTES).unwrap_or(usize::MAX);
    nf_pdf_winjob::limit_current_process(memory, CPU_SECONDS, 1)
        .map_err(|e| format!("job object: {e}"))?;
    probe.line(
        "windows",
        "isolation",
        "job object only: no network or file-system isolation",
    );
    Ok(vec![
        ("platform", "windows".to_string()),
        (
            "job_object",
            "process memory 3 GiB, CPU time 120 s, one process (no child processes)".to_string(),
        ),
        ("network", "not isolated on Windows".to_string()),
        ("file_system", "not isolated on Windows".to_string()),
    ])
}

/// The worker ends with its parent; if the parent has already gone, it stops now.
#[cfg(target_os = "linux")]
fn tie_to_parent(parent: Option<u32>) -> Result<(), String> {
    use rustix::process::{getppid, set_parent_process_death_signal, Pid, Signal};
    set_parent_process_death_signal(Some(Signal::KILL))
        .map_err(|e| format!("parent-death signal: {e}"))?;
    if let Some(expected) = parent {
        let now = Pid::as_raw(getppid());
        if u32::try_from(now).ok() != Some(expected) {
            return Err("the parent process has already ended".to_string());
        }
    }
    Ok(())
}

#[cfg(unix)]
fn resource_limits() -> Result<(), String> {
    use rustix::process::{setrlimit, Resource, Rlimit};
    let set = |res: Resource, v: u64| -> Result<(), String> {
        setrlimit(
            res,
            Rlimit {
                current: Some(v),
                maximum: Some(v),
            },
        )
        .map_err(|e| format!("{res:?}: {e}"))
    };
    set(Resource::As, ADDRESS_SPACE_BYTES)?;
    set(Resource::Cpu, CPU_SECONDS)?;
    set(Resource::Fsize, FILE_SIZE_BYTES)?;
    set(Resource::Nofile, OPEN_FILES)?;
    set(Resource::Core, 0)?;
    Ok(())
}

/// Landlock with no rule: nothing in the file system may be opened, executed, created or
/// removed, no TCP port bound or connected, no abstract UNIX socket reached, no other process
/// signalled. Returns the enforcement status for the report.
#[cfg(target_os = "linux")]
fn landlock() -> Result<String, String> {
    use landlock::{
        Access, AccessFs, AccessNet, LandlockStatus, Ruleset, RulesetAttr, RulesetStatus, Scope,
        ABI,
    };
    fn err(e: landlock::RulesetError) -> String {
        format!("landlock: {e}")
    }
    let abi = ABI::V10;
    let status = Ruleset::default()
        .handle_access(AccessFs::from_all(abi))
        .map_err(err)?
        .handle_access(AccessNet::from_all(abi))
        .map_err(err)?
        .scope(Scope::from_all(abi))
        .map_err(err)?
        .create()
        .map_err(err)?
        .restrict_self()
        .map_err(err)?;
    let kernel = match status.landlock {
        LandlockStatus::Available { effective_abi, .. } => format!("kernel ABI {effective_abi:?}"),
        LandlockStatus::NotEnabled => "Landlock is built into the kernel but not enabled".into(),
        LandlockStatus::NotImplemented => "the kernel has no Landlock".into(),
    };
    Ok(match status.ruleset {
        RulesetStatus::FullyEnforced => format!("fully enforced ({kernel})"),
        RulesetStatus::PartiallyEnforced => format!(
            "enforced with the rights this kernel supports ({kernel}; rights of later ABIs are not available)"
        ),
        _ => format!("not enforced ({kernel}); the seccomp allow-list still refuses file and socket calls"),
    })
}

/// The seccomp allow-list. Everything not listed fails with EPERM.
#[cfg(target_os = "linux")]
// `i64::from` is a no-op where `c_long` is 64 bits, and needed where it is 32 bits.
#[allow(clippy::useless_conversion)]
fn seccomp() -> Result<(), String> {
    use std::collections::BTreeMap;

    use seccompiler::{
        BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
        SeccompRule, TargetArch,
    };
    fn err(e: impl std::fmt::Display) -> String {
        format!("seccomp: {e}")
    }
    let arch = TargetArch::try_from(std::env::consts::ARCH).map_err(err)?;
    // Reading the input from standard input, writing the answer and messages, memory, time,
    // random keys for hash maps, locks, signal return, and ending the process.
    let plain: [libc::c_long; 22] = [
        libc::SYS_read,
        libc::SYS_readv,
        libc::SYS_write,
        libc::SYS_writev,
        libc::SYS_close,
        libc::SYS_munmap,
        libc::SYS_mremap,
        libc::SYS_madvise,
        libc::SYS_brk,
        libc::SYS_futex,
        libc::SYS_getrandom,
        libc::SYS_clock_gettime,
        libc::SYS_sched_yield,
        libc::SYS_rt_sigreturn,
        libc::SYS_rt_sigprocmask,
        libc::SYS_rt_sigaction,
        libc::SYS_sigaltstack,
        libc::SYS_restart_syscall,
        libc::SYS_getpid,
        libc::SYS_gettid,
        libc::SYS_exit,
        libc::SYS_exit_group,
    ];
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    for nr in plain {
        rules.insert(i64::from(nr), Vec::new());
    }
    // No executable memory: mmap and mprotect only when PROT_EXEC is clear.
    let prot_exec = u64::try_from(libc::PROT_EXEC).map_err(err)?;
    for nr in [libc::SYS_mmap, libc::SYS_mprotect] {
        let no_exec = SeccompCondition::new(
            2,
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::MaskedEq(prot_exec),
            0,
        )
        .map_err(err)?;
        rules.insert(
            i64::from(nr),
            vec![SeccompRule::new(vec![no_exec]).map_err(err)?],
        );
    }
    // Signals only to this process (abort raises SIGABRT on itself).
    let own = SeccompCondition::new(
        0,
        SeccompCmpArgLen::Dword,
        SeccompCmpOp::Eq,
        u64::from(std::process::id()),
    )
    .map_err(err)?;
    rules.insert(
        i64::from(libc::SYS_tgkill),
        vec![SeccompRule::new(vec![own]).map_err(err)?],
    );
    let eperm = u32::try_from(libc::EPERM).map_err(err)?;
    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Errno(eperm),
        SeccompAction::Allow,
        arch,
    )
    .map_err(err)?;
    let program: BpfProgram = filter.try_into().map_err(err)?;
    seccompiler::apply_filter(&program).map_err(err)
}
