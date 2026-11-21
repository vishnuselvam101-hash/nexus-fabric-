//! Job Object limits for the `nf-pdf2dxf` worker process on Windows.
//!
//! [`limit_current_process`] puts the calling process into a new, unnamed Job Object that
//! limits its committed memory, its user-mode CPU time and the number of active processes in
//! the job (1: the worker cannot start child processes), and ends it on an unhandled exception
//! instead of opening an error dialogue. On other systems this crate is empty.
//!
//! Why this is a separate crate with `unsafe` code: the Job Object functions
//! (`CreateJobObjectW`, `SetInformationJobObject`, `AssignProcessToJobObject`) are reachable from
//! Rust only as `extern "system"` functions (windows-sys, windows), and calling them is `unsafe`.
//! The safe wrapper crate available (win32job 2.1.0) cannot set the process memory limit, the
//! per-process CPU-time limit or the active-process limit (its limit structure is private), so
//! it cannot provide these limits. `nf-pdf` keeps `#![forbid(unsafe_code)]`; the three calls
//! below are the only `unsafe` code of `nf-pdf2dxf`, and they pass no pointer that outlives the
//! call.
#![cfg_attr(not(windows), forbid(unsafe_code))]
#![deny(unsafe_op_in_unsafe_fn)]

/// Places the calling process in a new Job Object with the given limits. The job handle is
/// kept open until the process ends, so the limits last as long as the process.
#[cfg(windows)]
pub fn limit_current_process(
    memory_bytes: usize,
    cpu_seconds: u64,
    active_processes: u32,
) -> std::io::Result<()> {
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION,
        JOB_OBJECT_LIMIT_PROCESS_MEMORY, JOB_OBJECT_LIMIT_PROCESS_TIME,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    // SAFETY: null security attributes and a null name are documented inputs (default security,
    // unnamed job). The call returns a handle, or null on failure, which is checked.
    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if job.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_PROCESS_MEMORY
        | JOB_OBJECT_LIMIT_PROCESS_TIME
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
        | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
    // CPU time in 100-nanosecond units.
    info.BasicLimitInformation.PerProcessUserTimeLimit =
        i64::try_from(cpu_seconds.saturating_mul(10_000_000)).unwrap_or(i64::MAX);
    info.BasicLimitInformation.ActiveProcessLimit = active_processes;
    info.ProcessMemoryLimit = memory_bytes;
    let size = u32::try_from(std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
        .map_err(|_| std::io::Error::other("job limit structure size"))?;
    // SAFETY: `job` is a valid job handle; the pointer is to `info`, a properly initialised
    // JOBOBJECT_EXTENDED_LIMIT_INFORMATION that lives until the call returns, and `size` is its
    // size. The function only reads the structure.
    let ok = unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            std::ptr::addr_of!(info).cast(),
            size,
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `job` is a valid job handle and GetCurrentProcess returns the pseudo-handle of the
    // calling process, which needs no closing.
    let ok = unsafe { AssignProcessToJobObject(job, GetCurrentProcess()) };
    if ok == 0 {
        return Err(std::io::Error::last_os_error());
    }
    // The handle is deliberately not closed: the job lasts until the process ends.
    Ok(())
}
