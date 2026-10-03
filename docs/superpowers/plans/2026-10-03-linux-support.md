# Linux support implementation plan

Goal: support Linux installation, release archives and safe serial help enrichment alongside macOS, for zsh/Bash/Fish.

Architecture: a platform probe module wraps sandbox-exec on macOS and applies Landlock ABI >=3 (Linux 6.2+, enabled) and seccomp on Linux x86_64/aarch64 before exec. Fail closed if unavailable. No personal command probing in tests. Older kernels retain existing definitions/caches and interactive support. Shared native resource constants replace numeric FFI constants.

- [x] Add failing Linux fixtures for filesystem mutation/network/session escape protection, help and completion generation, unsupported sandbox diagnostics, and portable installation.
- [x] Implement platform sandbox and resource constants; keep serial worker, deadlines and process-group cleanup. Sandbox init occurs before any target instruction. No unsandboxed retries.
- [x] Package Linux x86_64/aarch64 glibc binaries and one shared installer with platform/architecture manifest; collect release assets before publishing once.
- [x] Run sandbox fixtures, installer and all-shell interaction tests in resource-limited, non-root Docker, including x86_64 if emulation available. Record unrelated existing zsh slow-provider failure.
- [x] Update README/website with Linux requirements and verified limits; no commit, push, publish or host installation.

Security scope: help probes may read installed files and emit stdout/stderr. Deny filesystem writes (including metadata), socket creation/network, process-group escape and selected process/kernel mutation interfaces. Preserve support for normal child processes and threads. Restrict memory/process/CPU/output resources. This is defense in depth, not a claim of protection against kernel exploits or hostile resource-exhaustion workloads.

Verification caveat: local x86_64 emulation returns ENOSYS for Landlock; native isolation is a release gate, not locally claimed as passing. The existing zsh slow-completion failure remains recorded.
