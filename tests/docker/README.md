# Docker 验收

从项目根目录运行：

```sh
sh tests/docker/run.sh
RFIG_TEST_IMAGE=rust:1.91-slim BUILD_BASH44=0 sh tests/docker/run.sh
```

需要 Docker。准备阶段安装依赖并构建，测试阶段断网，以普通用户串行运行。
容器启用 init 回收退出的子进程，限制 2 CPU、2 GiB 内存、256 个进程；
只读挂载仓库，每个交互测试有独立 HOME 和人工命令，不读取个人 Shell 配置。
Bash 4.4 从 GNU 官方源码构建，源码 SHA256 会写入容器 `/work/bash44-source.sha256`。

结果保存到 `target/docker-results/`，包括逐套日志、JSON 汇总和严格测试的文本屏幕。
任意失败返回非零退出码，结果仍会保留，容器自动清理。

## 覆盖范围

- Rust 单元测试、三个 Shell 的 setup 重复安装与生成脚本一致性。
- 源码安装脚本的流程（模拟 cargo）；Linux/macOS 识别平台及架构，拒绝错误安装包。
- Bash/Fish：Emacs、Vi 插入模式各重复三轮，Vi 使用多行提示符。
- 真正的 tmux/PTY 交互：菜单、嵌套候选、灰字与输入分离、目录历史、使用排序、
  短长选项、Tab 原生补全、Ctrl+C、Ctrl+D、中文、宽行、窗口缩放、菜单清理。
- 严格案例：空格及特殊字符候选的实际 argv、多行粘贴、Shell 状态保留、
  临时目录与历史权限、慢补全输入/取消、SIGTERM 与子进程清理。
- `xterm-256color` 完整测试和 `xterm` 的候选/宽行测试。
- Linux 使用真实 Landlock + seccomp 验证写入、元数据、网络、进程组限制，以及不支持沙箱时拒绝执行。
- setup 串行优化、补全生成器、协议和跨 Shell 定义的集成测试。

## 边界

Docker 不验证 macOS GUI 终端、字体外观、macOS 安装包和 sandbox-exec。
Linux 帮助探测要求启用 Landlock ABI 3+（通常为 Linux 6.2+）及 seccomp。Docker 使用宿主内核；更换发行版镜像不会升级内核。测试不使用特权容器，也不以假沙箱替代真实隔离。旧内核上的探测测试会失败，应保留失败原因。

已知未通过项及本次版本和结果见
[验收报告](../../docs/testing/docker-2026-10-03/REPORT.md)。

Linux 支持后的补测见 [Linux 支持说明](../../docs/linux.md)。
