# Linux 支持

当前源码支持 Linux 的 zsh、Bash 4.4+ 和 Fish 3.6+：菜单、灰字、目录历史、习惯排序、Shell 集成及后台补全优化共用现有逻辑。

## 安装与版本要求

源码目录内执行：

```sh
sh install.sh --shell bash
# 或 --shell zsh / --shell fish
```

安装到 `~/.local/bin/rfig`，自动运行 setup。已有二进制时可单独执行 `rfig setup --shell <shell>`。

发布工作流新增两种 Linux 归档：`linux-x86_64`、`linux-aarch64`。
每个包包含二进制、共用安装脚本、三个 Shell 脚本、TARGET 架构标记、许可证与 SHA256 校验文件。
下载目录不限，解压进入包目录后执行 `sh install.sh`。不匹配的平台/架构以及校验失败都会在移动二进制前被拒绝。

- 预编译包的构建基线是 Debian 12 / glibc 2.36；更旧用户空间可尝试源码构建。
- 自动或手动帮助探测需要启用 **Landlock ABI 3+**（通常 Linux 6.2+）和允许安装 seccomp 过滤器。
- 不需要 root、Docker 或额外的沙箱程序才能使用 rfig。建议以实际使用 Shell 的普通用户安装。
- 内核能力不足时，setup 仍安装 Shell 集成，但明确提示后台优化不可用，并在 `enrich.status` 写入 `unavailable`。不会退回无隔离执行。已有定义、缓存、菜单与历史仍可用。
- 容器共享宿主内核。升级容器发行版并不能补上宿主或模拟层不提供的系统调用。
- Alpine/musl、其他 CPU 架构和 Windows 未验证。

**这次是本地开发，尚未发布新的 GitHub Release 或更新 Homebrew Tap。** 下载渠道以实际发布文件为准。

## 帮助探测的隔离方式

所有后台优化按命令串行执行；`rfig analyze` 对单个命令使用同样的隔离。动态原生补全仍信任用户安装的脚本，不套用这个禁止网络的帮助探测策略。

1. **Landlock**：禁止文件写入、创建、删除、移动和截断，允许读取及执行。`/dev/null` 可写；stdout/stderr 使用父进程预先打开的输出文件。
2. **seccomp**：补充限制文件元数据修改、网络/Unix socket、设备 ioctl、进程组逃逸和部分内核/进程控制接口。过滤器检查 CPU ABI，拒绝 x32 绕过；未纳入基线的较新系统调用返回不支持。
3. **继承**：限制在 exec 前生效，后续子进程继承；不能通过 setsid/setpgid 逃离受管理进程组。普通子进程和线程可用，受资源限制约束。
4. **资源**：每次调用墙钟限时 500ms、CPU 2 秒、输出文件最大 512 KiB（最多读取 256 KiB）；Linux 另设 2 GiB 虚拟地址空间、256 进程/线程上限及禁用 core dump。进程数限制遵循系统按用户计数的语义；高虚拟内存需求的运行时可能被拒绝。不能将这些阈值视为整个机器的内存配额。
5. **验证**：补全生成器和生成脚本的语法检查均有隔离及超时；语法检查使用真实退出状态，不将非零退出当作通过。

这套限制用于减少第三方帮助程序的副作用，不承诺能抵御内核漏洞或所有恶意资源耗尽攻击。读取文件仍被允许，沙箱也可能使依赖网络、写缓存或大型运行时的帮助程序无法工作；此时使用原生定义或已有缓存。

实现依据：[Linux Landlock 文档](https://www.kernel.org/doc/html/v6.8/userspace-api/landlock.html)、[seccomp 手册](https://www.man7.org/linux/man-pages/man2/seccomp.2.html)。平台资源限制使用 libc 常量，避免 macOS/Linux 编号差异。

## 本轮验证

- Linux ARM64，Debian 12、内核 `7.0.12-linuxkit`，Landlock ABI 8。普通用户、Docker 默认安全配置，2 CPU / 2 GiB / 256 进程，测试阶段断网。
- 真实隔离测试覆盖创建/修改/删除/重命名文件、chmod、时间戳、扩展属性、IPv4/IPv6/Unix socket、setsid/setpgid；普通 Python 帮助脚本可执行并正确生成候选。
- 额外 seccomp 测试屏蔽 Landlock 系统调用，确认 `analyze` 明确失败、setup 提示不可用并保留 Shell 集成。没有假冒 sandbox-exec。
- 安装后串行优化、超时与子进程清理、生成器、Cobra 协议、复用其他 Shell 定义、rfig 自身补全、真实 release 二进制安装均已验证。
- Bash 5.2、Fish 3.6 运行默认编辑和 Vi 插入模式各三轮，另含严格边界测试；zsh 5.9 同时验收。
- x86_64 Docker 模拟环境已构建并验证基础交互；该模拟层 Landlock 返回 `ENOSYS`，所以不能作为原生 x86_64 沙箱通过的证据。发布流程在原生 x86_64 和 ARM64 runner 上增加真实隔离、后台优化及二进制安装检查；这些远端检查尚未运行。
- macOS 本机重新通过 Rust 单元测试与隔离 HOME 的 rfig 自身补全检查，保留原 sandbox-exec 路径。

**已知未修复**：zsh 原生慢补全仍可能阻塞输入，与 Linux 沙箱适配无关。完整测试运行器保留此失败并返回非零，不宣称全量验收通过。

ARM64 最终矩阵 28 次套件执行中 27 次通过；x86_64 模拟环境 27 次中 22 次通过（4 项隔离能力缺失，1 项 zsh 已知问题）。详细结果与日志见 [本轮记录](testing/linux-2026-10-03/README.md)。
