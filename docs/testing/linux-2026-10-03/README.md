# Linux 支持验收记录

2026-10-03，限资源 Docker（2 CPU、2 GiB、256 进程），启用 init，普通用户，隔离 HOME，测试阶段断网。仅使用人工命令和 rfig 自身，无宿主机命令扫描。

| 环境 | 矩阵执行数 | 通过 | 失败 |
| --- | --- | --- | --- |
| Debian 12 ARM64，Linux 7.0.12-linuxkit / Landlock ABI 8 | 28 | 27 | 1：已知 zsh 慢补全阻塞 |
| Debian 12 x86_64 模拟执行，同一宿主内核 | 27 | 22 | 4 项探测相关测试被不支持的 Landlock 阻断；1 项同上 |

数字是套件执行次数，包含重复运行。Shell 版本为 Bash 5.2.15、Fish 3.6.0、zsh 5.9；每个环境的 Bash/Fish 默认编辑和 Vi 插入模式各跑三轮。

- [ARM64 汇总](arm64-summary.json)、[完整日志及屏幕](arm64.tar.gz)、[zsh 失败](arm64-zsh59-strict.log)。
- [x86_64 模拟汇总](x86_64-emulated-summary.json)、[完整日志及屏幕](x86_64-emulated.tar.gz)。模拟层调用 Landlock 返回 `-1 / ENOSYS (38)`，没有以关闭隔离或特权容器绕过。
- [受测核心源码摘要](source.sha256)。

ARM64 矩阵覆盖真实 Landlock/seccomp、串行帮助分析、生成脚本语法检查、Cobra 协议、跨 Shell 补全、原生按键交互和真实编译二进制安装。模拟环境的基础交互测试通过，但不能证明原生 x86_64 沙箱可用。

矩阵之外，两种架构均完成 release 构建，并从任意名称的下载目录给 zsh、Bash、Fish 分别安装真实 release 二进制，检查可执行文件与嵌入脚本。x86_64 release 安装额外验证了不支持沙箱时的提示及 `unavailable` 状态；测试接受该降级，未将其算作帮助探测成功。

macOS 本机通过 12 个 Rust 单元测试以及隔离 HOME 的 `self_completion.py`，未执行个人命令探测或安装到宿主机。

发布工作流已加入原生 x86_64/ARM64 的隔离和安装检查。由于本轮未推送或触发 GitHub Actions，这两个远端检查尚未运行，发行包也未发布。

[功能、要求与安全边界](../../linux.md)。
