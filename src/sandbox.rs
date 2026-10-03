//! Isolation for installation-time help probes. Live completion keeps its native context.
//! Landlock policy: no filesystem mutation except /dev/null; reads and exec are allowed.
//! seccomp fills Landlock's metadata/socket gaps and keeps descendants in the probe group.
use std::{io, process::Command};

pub fn check() -> io::Result<()> {
    #[cfg(target_os = "linux")]
    return linux::check();
    #[cfg(not(target_os = "linux"))]
    Ok(())
}

pub fn command(name: &str) -> Command {
    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("/usr/bin/sandbox-exec");
        cmd.args([
            "-p",
            "(version 1) (allow default) (deny file-write*) (deny network*)",
            name,
        ]);
        cmd
    }
    #[cfg(not(target_os = "macos"))]
    Command::new(name)
}

// Call after the process-group and general resource-limit pre_exec hook.
pub fn protect(command: &mut Command) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    return linux::protect(command);
    #[cfg(target_os = "macos")]
    {
        let _ = command;
        Ok(())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = command;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "help probing requires macOS or Linux",
        ))
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::process::CommandExt,
    };

    // Linux UAPI: landlock.h, filter.h and seccomp.h. No best-effort downgrade.
    #[repr(C)]
    struct Ruleset {
        handled_access_fs: u64,
    }
    #[repr(C, packed)]
    struct PathRule {
        allowed_access: u64,
        parent_fd: i32,
    }
    fn error() -> io::Error {
        io::Error::last_os_error()
    }
    fn filter() -> Vec<libc::sock_filter> {
        fn stmt(code: u16, k: u32) -> libc::sock_filter {
            libc::sock_filter {
                code,
                jt: 0,
                jf: 0,
                k,
            }
        }
        fn eq(k: u32, yes: u8, no: u8) -> libc::sock_filter {
            libc::sock_filter {
                code: 0x15,
                jt: yes,
                jf: no,
                k,
            }
        }
        #[cfg(target_arch = "x86_64")]
        let arch = 0xc000003e;
        #[cfg(target_arch = "aarch64")]
        let arch = 0xc00000b7;
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        let arch = 0; // Unsupported ABIs cannot execute under this policy.
        let mut code = vec![
            stmt(0x20, 4),
            eq(arch, 1, 0),
            stmt(0x06, 0x80000000),
            stmt(0x20, 0),
        ];
        #[cfg(target_arch = "x86_64")]
        {
            // x32 uses the x86_64 audit arch but a different syscall table.
            code.push(libc::sock_filter {
                code: 0x45,
                jt: 0,
                jf: 1,
                k: 0x40000000,
            });
            code.push(stmt(0x06, 0x80000000));
        }
        // Report unsupported device operations like a non-terminal. CPython can
        // then fall back from FIOCLEX to fcntl when opening ordinary script files.
        code.push(eq(libc::SYS_ioctl as u32, 0, 1));
        code.push(stmt(0x06, 0x50000 | libc::ENOTTY as u32));
        // Unknown newer syscalls must not introduce a metadata/mount bypass.
        // This baseline covers Linux syscalls through 450 on both shipped ABIs.
        code.push(libc::sock_filter {
            code: 0x35,
            jt: 0,
            jf: 1,
            k: 451,
        });
        code.push(stmt(0x06, 0x50000 | libc::ENOSYS as u32));
        let mut denied = vec![
            libc::SYS_socket,
            libc::SYS_socketpair,
            libc::SYS_connect,
            libc::SYS_bind,
            libc::SYS_sendto,
            libc::SYS_sendmsg,
            libc::SYS_sendmmsg,
            libc::SYS_setsockopt,
            libc::SYS_listen,
            libc::SYS_accept,
            libc::SYS_accept4,
            libc::SYS_setsid,
            libc::SYS_setpgid,
            libc::SYS_unshare,
            libc::SYS_setns,
            libc::SYS_mount,
            libc::SYS_umount2,
            libc::SYS_pivot_root,
            libc::SYS_chroot,
            libc::SYS_ptrace,
            libc::SYS_process_vm_writev,
            libc::SYS_process_vm_readv,
            libc::SYS_pidfd_getfd,
            libc::SYS_pidfd_send_signal,
            libc::SYS_kill,
            libc::SYS_tkill,
            libc::SYS_tgkill,
            libc::SYS_fchmod,
            libc::SYS_fchmodat,
            libc::SYS_open_tree,
            libc::SYS_move_mount,
            libc::SYS_fsopen,
            libc::SYS_fsconfig,
            libc::SYS_fsmount,
            libc::SYS_fspick,
            libc::SYS_mount_setattr,
            libc::SYS_fchown,
            libc::SYS_fchownat,
            libc::SYS_utimensat,
            libc::SYS_setxattr,
            libc::SYS_lsetxattr,
            libc::SYS_fsetxattr,
            libc::SYS_removexattr,
            libc::SYS_lremovexattr,
            libc::SYS_fremovexattr,
            libc::SYS_io_uring_setup,
            libc::SYS_io_uring_enter,
            libc::SYS_io_uring_register,
            libc::SYS_bpf,
            libc::SYS_perf_event_open,
            libc::SYS_keyctl,
            libc::SYS_add_key,
            libc::SYS_request_key,
            libc::SYS_reboot,
            libc::SYS_kexec_load,
            libc::SYS_init_module,
            libc::SYS_finit_module,
            libc::SYS_delete_module,
            libc::SYS_swapon,
            libc::SYS_swapoff,
            libc::SYS_sethostname,
            libc::SYS_setdomainname,
            libc::SYS_clock_settime,
            libc::SYS_settimeofday,
            libc::SYS_adjtimex,
        ];
        #[cfg(target_arch = "x86_64")]
        denied.extend([
            libc::SYS_chmod,
            libc::SYS_chown,
            libc::SYS_lchown,
            libc::SYS_utime,
            libc::SYS_utimes,
            libc::SYS_futimesat,
            libc::SYS_iopl,
            libc::SYS_ioperm,
        ]);
        denied.sort_unstable();
        denied.dedup();
        for syscall in denied {
            code.push(eq(syscall as u32, 0, 1));
            code.push(stmt(0x06, 0x50000 | libc::EPERM as u32));
        }
        // clone3's flags live behind a pointer; force libc's inspectable clone fallback.
        code.push(eq(libc::SYS_clone3 as u32, 0, 1));
        code.push(stmt(0x06, 0x50000 | libc::ENOSYS as u32));
        // Do not allow children to create namespaces or detach from our parent hierarchy.
        code.push(eq(libc::SYS_clone as u32, 0, 3));
        code.push(stmt(0x20, 16)); // seccomp_data.args[0], clone flags on supported ABIs
        let flags = libc::CLONE_PARENT
            | libc::CLONE_NEWNS
            | libc::CLONE_NEWCGROUP
            | libc::CLONE_NEWUTS
            | libc::CLONE_NEWIPC
            | libc::CLONE_NEWUSER
            | libc::CLONE_NEWPID
            | libc::CLONE_NEWNET;
        code.push(libc::sock_filter {
            code: 0x45,
            jt: 0,
            jf: 1,
            k: flags as u32,
        });
        code.push(stmt(0x06, 0x50000 | libc::EPERM as u32));
        code.push(stmt(0x06, 0x7fff0000));
        code
    }

    pub fn check() -> io::Result<()> {
        let abi = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                std::ptr::null::<u8>(),
                0usize,
                1u32,
            )
        };
        if abi < 3 {
            return Err(io::Error::new(io::ErrorKind::Unsupported,
                "Linux help probing needs Landlock ABI 3+ (Linux 6.2+ with Landlock enabled); existing completion definitions remain available"));
        }
        Ok(())
    }

    pub fn protect(command: &mut Command) -> io::Result<()> {
        check()?;
        let attr = Ruleset {
            handled_access_fs: 0x7fff & !(1 | 4 | 8),
        };
        let fd = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                &attr,
                std::mem::size_of::<Ruleset>(),
                0u32,
            )
        } as i32;
        if fd < 0 {
            return Err(error());
        }
        let rules = unsafe { OwnedFd::from_raw_fd(fd) };
        let null_fd = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
        if null_fd < 0 {
            return Err(error());
        }
        let null = unsafe { OwnedFd::from_raw_fd(null_fd) };
        let path = PathRule {
            allowed_access: (1 << 1) | (1 << 14),
            parent_fd: null.as_raw_fd(),
        };
        if unsafe {
            libc::syscall(
                libc::SYS_landlock_add_rule,
                rules.as_raw_fd(),
                1u32,
                &path,
                0u32,
            )
        } < 0
        {
            return Err(error());
        }
        let code = filter(); // Allocate in the parent, not in the post-fork child.
        unsafe {
            command.pre_exec(move || {
                for (resource, limit) in [
                    (libc::RLIMIT_AS, 2 * 1024 * 1024 * 1024),
                    (libc::RLIMIT_NPROC, 256),
                    (libc::RLIMIT_CORE, 0),
                ] {
                    let limit = libc::rlimit {
                        rlim_cur: limit,
                        rlim_max: limit,
                    };
                    if libc::setrlimit(resource, &limit) < 0 {
                        return Err(error());
                    }
                }
                if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) < 0
                    || libc::syscall(libc::SYS_landlock_restrict_self, rules.as_raw_fd(), 0u32) < 0
                {
                    return Err(error());
                }
                let program = libc::sock_fprog {
                    len: code.len() as u16,
                    filter: code.as_ptr() as *mut _,
                };
                if libc::prctl(libc::PR_SET_SECCOMP, 2, &program, 0, 0) < 0 {
                    return Err(error());
                }
                Ok(())
            });
        }
        Ok(())
    }
}
