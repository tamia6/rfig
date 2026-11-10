//! OS services kept separate from terminal input and shell completion.
use std::io::Write;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Child, Command},
};

pub fn home() -> PathBuf {
    PathBuf::from(
        env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .unwrap_or_default(),
    )
}
pub fn initialize_environment() {
    if cfg!(windows) && env::var_os("HOME").is_none() {
        if let Some(home) = env::var_os("USERPROFILE") {
            env::set_var("HOME", home);
        }
    }
}
#[cfg(unix)]
pub type TerminalOutput = fs::File;
#[cfg(windows)]
pub type TerminalOutput = io::Stdout;

pub fn terminal_output() -> io::Result<TerminalOutput> {
    #[cfg(unix)]
    return fs::OpenOptions::new().write(true).open("/dev/tty");
    #[cfg(windows)]
    // Stdout uses the Unicode Windows console writer when attached to a console.
    Ok(io::stdout())
}

pub fn is_executable(path: &Path, metadata: fs::Metadata) -> bool {
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = path;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(windows)]
    {
        path.extension().is_some_and(|ext| {
            ["exe", "com", "cmd", "bat"]
                .iter()
                .any(|v| ext.eq_ignore_ascii_case(v))
        })
    }
}
pub fn command_name(name: &str) -> String {
    if cfg!(windows) {
        if let Some((stem, ext)) = name.rsplit_once('.') {
            if ["exe", "com", "cmd", "bat"]
                .iter()
                .any(|v| ext.eq_ignore_ascii_case(v))
            {
                return stem.into();
            }
        }
    }
    name.into()
}
pub fn private_append(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}
#[cfg(unix)]
pub fn kill_group(pid: i32) {
    if pid > 0 {
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
}
#[cfg(unix)]
pub fn spawn_completion(command: &mut Command) -> io::Result<Child> {
    command.spawn()
}
#[cfg(unix)]
pub fn kill_completion(child: &mut Child) {
    kill_group(child.id() as i32);
}

pub fn setup_powershell() -> io::Result<()> {
    let config = home().join(".config/rfig");
    fs::create_dir_all(&config)?;
    super::install_integration(&config.join("rfig.ps1"), "ps1", include_str!("../rfig.ps1"))?;
    fs::write(
        config.join("rfig-completion.ps1"),
        include_str!("../shell/rfig-completion.ps1"),
    )?;
    let profile = if let Some(profile) = env::var_os("RFIG_POWERSHELL_PROFILE") {
        PathBuf::from(profile)
    } else {
        let output = Command::new("pwsh")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "$PROFILE.CurrentUserCurrentHost",
            ])
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other("could not resolve the PowerShell profile"));
        }
        let path = String::from_utf8_lossy(&output.stdout)
            .trim()
            .trim_start_matches('\u{feff}')
            .to_owned();
        if path.is_empty() {
            return Err(io::Error::other(
                "PowerShell returned an empty profile path",
            ));
        }
        PathBuf::from(path)
    };
    if let Some(parent) = profile.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = match fs::read_to_string(&profile) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let source = format!(
        ". '{}'",
        config
            .join("rfig.ps1")
            .display()
            .to_string()
            .replace('\'', "''")
    );
    let path_line =
        "$env:PATH = (Join-Path $HOME '.local/bin') + [IO.Path]::PathSeparator + $env:PATH";
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&profile)?;
    for line in [path_line, source.as_str()] {
        if !existing.lines().any(|old| old == line) {
            writeln!(file, "\n{line}")?;
        }
    }
    println!("rfig PowerShell integration installed; open a new PowerShell terminal.");
    Ok(())
}

#[cfg(windows)]
pub use windows::{cancel_job, kill_completion, lock_file, spawn_completion};
#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        mem,
        os::windows::{
            io::{AsRawHandle, FromRawHandle, OwnedHandle},
            process::CommandExt,
        },
        sync::{Arc, Mutex},
    };
    use windows_sys::Win32::{
        Foundation::*,
        Storage::FileSystem::*,
        System::{Diagnostics::ToolHelp::*, JobObjects::*, Threading::*},
    };
    static JOB: Mutex<Option<Arc<OwnedHandle>>> = Mutex::new(None);
    pub struct CompletionChild {
        child: Child,
        job: Arc<OwnedHandle>,
    }
    impl std::ops::Deref for CompletionChild {
        type Target = Child;
        fn deref(&self) -> &Child {
            &self.child
        }
    }
    impl std::ops::DerefMut for CompletionChild {
        fn deref_mut(&mut self) -> &mut Child {
            &mut self.child
        }
    }
    pub fn cancel_job() {
        if let Ok(job) = JOB.lock() {
            if let Some(job) = job.as_ref() {
                unsafe {
                    TerminateJobObject(job.as_raw_handle() as HANDLE, 1);
                }
            }
        }
    }
    pub fn kill_completion(child: &mut CompletionChild) {
        unsafe {
            TerminateJobObject(child.job.as_raw_handle() as HANDLE, 1);
        }
    }
    impl Drop for CompletionChild {
        fn drop(&mut self) {
            kill_completion(self);
            let _ = self.child.wait();
            if let Ok(mut job) = JOB.lock() {
                if job.as_ref().is_some_and(|job| Arc::ptr_eq(job, &self.job)) {
                    *job = None;
                }
            }
        }
    }
    pub fn spawn_completion(command: &mut Command) -> io::Result<CompletionChild> {
        // Suspend before assigning the job: completion scripts cannot create unowned descendants.
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = Arc::new(OwnedHandle::from_raw_handle(handle));
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as _,
                mem::size_of_val(&limits) as u32,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            command.creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW);
            let mut child = command.spawn()?;
            let result = (|| {
                if AssignProcessToJobObject(handle, child.as_raw_handle() as HANDLE) == 0 {
                    return Err(io::Error::last_os_error());
                }
                let threads = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
                if threads == INVALID_HANDLE_VALUE {
                    return Err(io::Error::last_os_error());
                }
                let snapshot = OwnedHandle::from_raw_handle(threads);
                let mut entry: THREADENTRY32 = mem::zeroed();
                entry.dwSize = mem::size_of::<THREADENTRY32>() as u32;
                let mut found = Thread32First(snapshot.as_raw_handle() as HANDLE, &mut entry);
                while found != 0 {
                    if entry.th32OwnerProcessID == child.id() {
                        let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                        if thread.is_null() {
                            return Err(io::Error::last_os_error());
                        }
                        let thread = OwnedHandle::from_raw_handle(thread);
                        let mut active = JOB
                            .lock()
                            .map_err(|_| io::Error::other("completion job lock poisoned"))?;
                        *active = Some(job.clone());
                        if ResumeThread(thread.as_raw_handle() as HANDLE) == u32::MAX {
                            *active = None;
                            return Err(io::Error::last_os_error());
                        }
                        return Ok(());
                    }
                    found = Thread32Next(snapshot.as_raw_handle() as HANDLE, &mut entry);
                }
                Err(io::Error::other("completion thread not found"))
            })();
            if let Err(error) = result {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
            Ok(CompletionChild { child, job })
        }
    }
    pub fn lock_file(file: fs::File) -> io::Result<fs::File> {
        let mut overlapped = unsafe { mem::zeroed() };
        if unsafe {
            LockFileEx(
                file.as_raw_handle() as HANDLE,
                LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
                0,
                1,
                0,
                &mut overlapped,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(file)
    }
}
