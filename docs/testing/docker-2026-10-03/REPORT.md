# Docker 严格验收报告

日期：2026-10-03。结论：**尚未全部通过**。

最终两轮共执行 56 个测试套件（包含重复运行），54 个成功、2 个失败。
两处失败是同一缺陷在两套环境复现：zsh 原生补全脚本较慢时阻塞输入。
总运行器如实返回退出码 1，没有把失败标记为预期成功。
这些数字是套件执行次数，不是独立功能或断言数。

## 环境与结果

两套环境均为 Linux ARM64，Rust 1.91，隔离 HOME，普通 tester 用户，
只读挂载仓库、测试阶段断网；2 CPU、2 GiB 内存、256 进程上限、Docker init。
测试按顺序执行，没有扫描或执行宿主机的个人命令。

| 环境 | Shell | 结果 |
| --- | --- | --- |
| Debian Bookworm | Bash 4.4.0 / 5.2.15 | 所测交互通过 |
| Debian Bookworm | Fish 3.6.0 | 所测交互通过 |
| Debian Trixie | Bash 5.2.37 | 所测交互通过 |
| Debian Trixie | Fish 4.0.2 | 所测交互通过 |
| 两套环境 | zsh 5.9 | 慢补全输入测试失败；其余所测项目通过 |

Bash/Fish 每个版本在 Emacs、Vi 插入模式分别重复三轮；
Vi 场景使用多行提示符。每版另外执行 8 个严格案例和 2 个普通 ANSI 案例。
zsh 的严格案例中，6 个通过、1 个失败、1 个不适用而跳过：
SIGTERM 测试针对独立 Rust 编辑器进程，zsh 使用 ZLE。
Rust 的 12 个单元测试在两套环境分别通过。

基础镜像摘要：

- Bookworm：`rust@sha256:c1e5f19e773b7878c3f7a805dd00a495e747acbdc76fb2337a4ebf0418896b33`
- Trixie：`rust@sha256:f75071363e7f4771769d4cf81b1b7b290e607f4d4459e8731f6abdcee9982dc8`

Bash 4.4 从 GNU 官方源码编译，摘要见 [bash44-source.sha256](bash44-source.sha256)。
版本明细见 [Bookworm 环境](bookworm-environment.txt)、[Trixie 环境](trixie-environment.txt)。
被测源码摘要见 [tested-source.sha256](tested-source.sha256)。

## 验证方法

使用真实 tmux/PTY 启动 Shell，发送实际按键，读取渲染后的屏幕；
人工命令把实际收到的 argv 写为 JSON，与预期逐项比较。
验证 Shell 的 cd、环境变量和后续命令状态，不能仅以菜单截图判定成功。

覆盖菜单、灰字与真实输入分离、按目录历史、常用排序、嵌套补全、
短长参数过滤、原生 Tab、Ctrl+C/Ctrl+D、中文编辑、连续输入删除、换行与窗口缩放、
菜单清理、特殊字符转义、多行粘贴、慢补全、SIGTERM、子进程回收及历史文件权限。
慢补全人工延迟 5 秒，并产生子进程；要求输入在 500ms 内显示，
取消与后代进程结束分别在 750ms 内完成。时序数字是 tmux 观测值，不是正式性能基准。

安装相关测试使用真实 rfig 执行三个 Shell 的 setup，各执行两次，
检查幂等性、旧配置迁移及嵌入脚本与源码一致。
源码安装脚本使用模拟 cargo 验证流程，不等于完整发行版安装验收。
Linux 上额外验证 macOS 二进制包被拒绝，且原文件不被移动。

## 本轮发现并修复

1. Bash 空行 Ctrl+D：无效的 Readline `end-of-file` 动作改为原生 Ctrl+D 宏。
2. Bash/Fish 多行粘贴：CR/CRLF 统一为 LF；Fish 保留完整多行缓冲区。
3. Bash/Fish SIGTERM：Rust 编辑器走正常退出清理路径，取消并回收补全进程组、恢复终端模式。
4. zsh 特殊字符候选：保留 compadd 已生成的插入转义，避免重复转义造成真实 argv 错误。

上述修复均由先失败、后通过的真实交互案例验证；均位于公共路径，没有命令名特例。
另修正测试中的异步截图和临时目录清理竞态。

## 未通过项与限制

### zsh 慢补全阻塞

输入 `rfigslow ` 后，补全函数睡眠 5 秒；此时发送 `xyz`，500ms 内输入未显示。
原因是 `rfig.zsh` 在 ZLE 编辑回调内同步执行 `_main_complete`。
这是原生补全调用链问题；需要异步计算、过期结果丢弃以及取消清理，不能通过放宽测试阈值解决。
本轮保留失败回归，没有改写 zsh 的异步架构。

失败证据：[Bookworm](bookworm-zsh-failure.log)、[Trixie](trixie-zsh-failure.log)。

### Linux 自动帮助探测不可用

现有 `--help` / `-h` 优化依赖 macOS `sandbox-exec`。
Linux 无该沙箱时安全拒绝执行，测试确认人工命令没有被执行、没有生成伪成功缓存。
因此 Linux 的扫描、已存在补全定义和缓存可用，**帮助探测优化未通过功能验收**。
这项安全拒绝测试通过不代表优化功能可用。

### 未覆盖

- macOS GUI 终端、字体外观和动画视觉效果。
- macOS 二进制安装、Homebrew、macOS 沙箱中的生成器/协议/帮助探测完整链路。
- Linux x86_64、其它 Shell 版本和用户插件组合。
- SIGKILL 等无法捕获的终止信号。

`setup_safety.py`、`self_completion.py`、`zsh_integration.py` 依赖真实 macOS 沙箱，
未计入 Linux 通过数，也未使用假沙箱替代。

## 证据与复现

- [运行方法与测试边界](../../../tests/docker/README.md)
- [Bookworm 汇总](bookworm-summary.json)，[完整日志和文本屏幕](bookworm.tar.gz)
- [Trixie 汇总](trixie-summary.json)，[完整日志和文本屏幕](trixie.tar.gz)
- [首次失败基线](baseline.tar.gz)
- [中间轮日志](intermediate.tar.gz)：容器未启用 init，进程回收不充分触及上限；
  另有嵌入脚本未重新构建和安装测试平台假设错误。这轮没有作为最终产品结论。

最终容器启用 init 后，完成整轮矩阵，检查未留下运行中的测试 Shell、编辑器或补全进程。
本轮只修改本地代码与测试，没有提交、推送、发布或重新安装到宿主机。
