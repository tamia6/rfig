# rfig — Rust 终端自动补全（zsh、Bash、Fish）

[English](README.md) · **简体中文** · [官网](https://tamia6.github.io/rfig/zh/)

**开源终端自动补全工具：灰色内联推荐与历史建议就在命令行中。**

rfig 使用 Rust 开发，为 macOS 与 Linux 上的 zsh、Bash 和 Fish 提供 IDE 风格的命令行补全。输入后只显示一条低对比度灰色的内联推荐；用 Command 加方向键或数字切换，再按 Tab 插入。普通光标移动、历史浏览和命令执行仍交给原 Shell。

rfig 是受 Fig 启发的独立项目，与 Fig 无隶属关系。它使用本机可用的补全定义和脚本，并在满足隔离条件时分析命令帮助作为后备来源；不承诺覆盖所有命令。

内置历史命令建议按当前目录和输入前缀匹配，与普通补全共用同一条灰色内联推荐，**无需另外安装历史建议插件**。回车始终只执行实际输入。

![rfig 在 zsh 中展示 Git 逐级补全、动态分支、Brew 服务和 npm 脚本](assets/rfig-showcase.gif)

动图展示 Git 从 `g`、`gi`、`git` 逐级补全到 `checkout`，再选择当前仓库中的真实分支；同时演示 Brew 服务、npm 脚本、键盘选择、插入、执行和取消推荐。

[安装](#安装) · [使用](#使用) · [Shell 支持](#shell-支持) · [补全来源](#补全来源与后台优化) · [测试验证](#测试验证)

> rfig v0.3.1 支持 macOS 和 Linux，以及 zsh、Bash 4.4+、Fish 3.6+。Homebrew 与二进制安装方式见下文。

**快速了解：**终端自动补全与多级 CLI 补全 · 按目录匹配的内联历史建议 · 本地使用习惯排序 · 无需 AI 服务或云端账号。

## 功能列表

| 功能 | 当前能力 |
| --- | --- |
| 输入即显示内联推荐 | 输入、删除后触发更新，无需 `Tab`，没有人为防抖等待 |
| 多级动态补全 | 根据可用补全定义查询子命令、分支、路径等；层级与内容取决于来源 |
| 历史命令自动建议 | 按目录和输入前缀匹配历史，作为灰色内联推荐显示，Tab 显式接受 |
| 使用习惯排序 | 选择与执行记录只改变排序，不再叠加圈号等视觉标记 |
| 安静的灰色推荐 | 只显示一条 ANSI 灰色补全后缀，不弹出候选列表，不闪烁变色 |
| 多种补全来源 | 原生定义、已安装脚本、命令生成器、Cobra 协议与帮助缓存 |
| 安装后后台优化 | 快速扫描 `$PATH`，macOS/Linux 后台串行分析缺少补全的部分用户命令（需可用沙箱） |
| 全 Shell 支持进度 | zsh ✅、Bash 4.4+ ✅、Fish 3.6+ ✅；其他 Shell 待实现，差异见下表 |

## 安装

### 从当前源码安装

需要 Rust 和 Cargo。在项目目录执行以下一种方式：

```sh
sh install.sh                 # 根据 $SHELL 选择集成
# 或明确指定要启用的 Shell
sh install.sh --shell zsh
sh install.sh --shell bash
sh install.sh --shell fish
```

安装脚本编译到 `~/.local/bin/rfig`，随后执行 `rfig setup`。rfig 不会修改默认 Shell。macOS 自带 Bash 3.2 不支持，使用 Bash 集成前需要安装 Bash 4.4+。

### Homebrew Tap

```sh
brew install tamia6/tap/rfig
"$(brew --prefix rfig)/bin/rfig" setup
```

Homebrew 安装执行 setup 后，zsh、Bash、Fish 的集成脚本会链接到软件包的稳定 `opt` 路径。以后执行 `brew upgrade rfig`，打开新 Shell 会话即可加载新版脚本，无需再次 setup。旧安装需用更新后的版本执行一次 setup，将原来的脚本副本迁移为链接。已经打开的会话需重启才能更新已加载的函数。

### 下载二进制（无需 Rust）

从 [GitHub Releases](https://github.com/tamia6/rfig/releases) 选择与系统、CPU 架构匹配的包。当前打包目标为 `macos-universal`、`linux-x86_64`、`linux-aarch64`；以发布页实际提供的文件为准。解压后进入包含 `install.sh` 的目录执行：

```sh
sh install.sh
```

下载目录不限。压缩包包含二进制与安装脚本；脚本校验文件，将二进制移动到 `~/.local/bin/rfig`，再执行 setup。源码与二进制共用同一个安装脚本。新包通过 `TARGET` 标记平台与架构，错误包会在移动前被拒绝。Linux 预编译包要求 glibc 2.36+。命令选项和包内容以所下载版本为准。

### 启用多个 Shell

当前源码支持分别配置：

```sh
rfig setup --shell zsh
rfig setup --shell bash
rfig setup --shell fish
```

配置完成后打开对应 Shell 的新会话。`setup` 可重复运行，安装集成脚本（Homebrew 使用稳定的软件包链接，源码或二进制安装使用内置脚本副本）并按需加入 `~/.local/bin`，相同加载行不会重复添加；同一安装目录的后台优化任务串行执行。

| Shell | 配置位置 |
| --- | --- |
| zsh | `$ZDOTDIR/.zshrc`，默认 `~/.zshrc` |
| Bash | `~/.bashrc`；登录配置未提及 `.bashrc` 时补充加载行 |
| Fish | `$XDG_CONFIG_HOME/fish/conf.d/rfig.fish`，默认 `~/.config/fish/conf.d/rfig.fish` |

macOS 上执行一次 `rfig terminal-keys`，再打开新的 Terminal.app 窗口。该命令把 `Command+方向键` 和 `Command+1…5` 映射为终端按键序列，同时保留其他 Terminal.app 快捷键，并输出偏好设置备份路径。其他终端可以让 rfig 使用 Option/Alt，或将 Super/Command 序列转发给 Shell。

用 `command -v rfig` 检查实际使用的安装入口。如果之前安装的 `~/.local/bin/rfig` 覆盖了 Homebrew，先将该文件备份到 `$PATH` 之外，再移除重复的可执行文件。保留一种安装方式，避免程序与集成脚本版本不一致。为使用的各个 Shell 执行 `"$(brew --prefix rfig)/bin/rfig" setup --shell zsh`（或 `bash` / `fish`），再打开新会话。`~/.config/rfig/` 中的历史和使用记录会保留。

## 使用

直接输入命令即可看到一条灰色内联推荐。已安装的补全定义决定能推荐哪些子命令和动态参数；没有可用来源时不显示灰字。例如，有相应定义时，Git 补全可查询当前仓库的分支。

| 按键 | 行为 |
| --- | --- |
| `Command+↑/↓` 或 `Command+←/→` | 在已配置的 macOS 终端中切换灰色内联推荐 |
| `Command+1` … `Command+5` | 跳到排序中的指定推荐 |
| `Option/Alt+↑/↓`、`Option/Alt+←/→`、`Option/Alt+1…5` | 终端不转发 Command 时使用的兼容方式 |
| `Tab` | 插入灰色推荐；没有推荐时使用 Shell 原生补全 |
| `↑` / `↓` | 使用 Shell 原生历史导航 |
| `→` / `←` | 使用 Shell 原生光标移动 |
| `Ctrl+E` | 移到行尾，不接受灰色推荐 |
| `Enter` | 只执行实际输入，不会自动接受灰色推荐 |
| `Ctrl+C` | 取消当前输入 |

只显示当前推荐，并用 `2/7` 这样的小计数器表示它在排序中的位置，不展开候选列表。macOS Terminal.app 默认会拦截 Command+方向键，因此需运行 `rfig terminal-keys`；其他终端可配置 Super/Command 转发，Option/Alt 始终可用。输入 `-` 只推荐单横线选项，输入 `--` 只推荐双横线选项。

### 历史与常用排序

- **历史命令建议**：rfig 按当前目录和实际输入前缀查找最近匹配项。例如输入 `gi`，匹配的历史命令会成为灰色内联推荐。用 Command 或 Option/Alt 加方向键在历史与普通补全之间切换，按 Tab 填入，再按回车执行。
- **新安装**：历史从空记录开始，不自动导入缺少工作目录信息的旧 Shell 历史。
- **排序**：按命令上下文累计使用分数，选择候选加 3，记录执行加 1；同分时非选项候选优先。排序只改变顺序，不在标签上叠加额外圈号。
- **本地保存**：历史和排序记录写入本机文件，不依赖云端。Bash/Fish 不记录以空格开头的命令；zsh 在启用 `HIST_IGNORE_SPACE` 时遵循该规则。

使用 rfig 的历史命令自动建议功能后，可从 `.zshrc` 或插件管理器列表移除 `zsh-autosuggestions` 的加载配置，再开启新会话。Fish 集成会关闭 Fish 自带的命令自动建议，改用 rfig 按目录记录的历史。

### 内联显示

推荐内容以 ANSI 灰色显示在光标之后，后面附有 `2/7` 这类紧凑排序位置。不显示候选下拉列表、图标行、指针动画或颜色闪烁。没有可用补全来源时不显示灰字。

## Shell 支持

以下为当前源码支持的交互能力。

| 能力 | zsh | Bash 4.4+ | Fish 3.6+ |
| --- | --- | --- | --- |
| 灰色内联推荐、多级候选与实际输入分离 | ✅ | ✅ | ✅ |
| 按目录历史、使用排序、Command/Alt 快捷键 | ✅ | ✅ | ✅ |
| setup 集成与 rfig 自身补全 | ✅ | ✅ | ✅ |
| 多行粘贴不自动执行 | ✅ | ✅ 交回原生编辑器 | ✅ 交回原生编辑器 |

zsh 使用 ZLE，Bash/Fish 使用共享 Rust 编辑层。`cd`、函数和变量修改仍由当前 Shell 处理。Bash/Fish 支持默认 Emacs 编辑及 Vi 插入模式，`Esc` 交回原生编辑器，`Ctrl+R` 使用原生历史搜索；过期候选结果会丢弃。

**Linux 要求**：支持 x86_64 和 ARM64，Shell 版本要求同上。帮助分析使用内置 Landlock + seccomp，无需安装额外沙箱工具；需要启用 Landlock ABI 3+（通常为 Linux 6.2+），且运行环境允许相关系统调用。较老内核或受限容器会拒绝帮助探测，灰色内联推荐、历史和现有补全定义仍可使用。可用 `uname -r` 查看内核，再用 `rfig analyze <命令名>` 检查实际探测结果。不要仅凭发行版名称判断内核能力。

macOS 继续使用系统 `sandbox-exec`。Linux 二进制以 glibc 2.36 为构建基线；更老的用户空间可尝试源码构建。`rfig terminal-keys` 仅用于 macOS Terminal.app。Windows、其他 Shell 尚未支持。

## 补全来源与后台优化

### 输入时：使用当前环境的候选

rfig 不维护一份覆盖所有命令的固定补全库，各适配层按可用来源获取候选：

| 入口 | 查询方式 |
| --- | --- |
| zsh | 当前会话已注册定义优先；未注册时查找 `$fpath` 脚本、生成的 zsh 脚本，再尝试外部来源和帮助缓存 |
| Bash/Fish | 当前 Shell 的定义与自动加载脚本、相应 Shell 的生成脚本，再尝试外部来源和帮助缓存 |
| 外部来源 | 已识别的 Cobra `__complete` 协议，以及本机 Fish/Bash 补全脚本 |
| 帮助缓存 | 无可用补全来源时使用各命令路径下的子命令和选项缓存；动态参数仍需补全提供器 |

帮助分析按层串行遍历声明的子命令，对每条路径查询 `--help` / `-h`，直到叶子节点，不限制固定层数。每层分别缓存子命令和选项，由 zsh、Bash、Fish 共用。探测继续使用沙箱与已有的单进程资源限制；重复返回父级帮助时停止该分支，失败节点保留已有缓存并报告。输入时只读缓存，不额外扫描帮助；动态资源名称和位置参数值仍需其他补全来源。已知子命令分组中的纯文件兜底不会覆盖缓存；原生结果为空时也会读取缓存。叶子命令的原生动态参数结果仍优先保留。

不同 Shell 的定义和加载方式不同，候选不保证完全一致。后台优化不意味着输入时完全不运行查询：动态补全脚本仍会在需要时执行，补全脚本应来自可信来源。

### 安装后：快速扫描，再串行优化

1. **扫描**：枚举当前 `$PATH` 中的可执行文件及符号链接，索引 zsh 定义和常见目录里的 zsh/Fish/Bash 脚本。不是遍历整块磁盘，未进入 `$PATH` 的程序不会全部被发现。
2. **筛选**：后台跳过已有定义的命令，只对允许的常见用户安装目录进行探测。系统目录和其他来源的命令可进入目录，但不会自动执行。
3. **补齐**：macOS/Linux 在可用沙箱内逐个读取 `--help`，必要时读取 `-h`；根据帮助信息识别生成器，为本机可用的 Shell 生成并检查脚本语法，尝试识别 Cobra 协议，缓存子命令和选项类别。识别和语法检查不等于所有候选语义都正确。
4. **限制**：探测禁止文件写入与网络访问，每次进程调用限时 500ms，并限制 CPU 时间和输出。Linux 额外限制文件元数据修改、进程组逃逸、地址空间和进程数；允许写入预先打开的输出管道/文件及 `/dev/null`。生成脚本的语法检查也有沙箱和超时。复杂、资源需求较大或依赖网络的命令可能没有分析结果。隔离实现和边界见 [Linux 支持说明](docs/linux.md)。
5. **刷新**：再次运行 setup 时，对早于目标程序或 rfig 更新时间的缓存重新分析。后台尚未完成或没有可用定义时，部分命令可能没有内联推荐。

```sh
rfig setup                         # PATH 或命令安装变化后重新扫描
cat ~/.config/rfig/enrich.status    # 查看 analyzed / skipped / failed 计数
rfig analyze <命令名>               # 刷新一个命令及其多层子命令缓存（需可用沙箱）
rfig --help                        # 查看公开命令和用法
```

`enrich.status` 中的 `done` 表示本轮处理结束，不表示所有命令均分析成功；`unavailable` 表示当前环境缺少所需隔离能力。`rfig setup --help` 只显示帮助，不会触发安装或扫描。

## 本地数据与卸载

| 路径（相对于 `~/.config/rfig/`） | 用途 |
| --- | --- |
| `commands.txt` / `supported.txt` | 扫描的命令名 / 已发现补全定义的命令名 |
| `rfig.zsh` / `rfig.bash` / `rfig.fish` | Shell 集成脚本：Homebrew 使用稳定的软件包链接，源码或二进制安装使用内置副本 |
| `generated/*.{zsh,bash,fish}` / `protocol/*` | 生成并检查的补全脚本 / 识别的协议 |
| `options/*.tsv` / `fallback/*.tsv` | 首层选项分类和旧版首层候选 |
| `help/<command>/` | 各命令路径的帮助缓存，含多层子命令和叶子选项 |
| `enrich.status` / `enrich.lock` | 后台进度 / 防止重复后台任务的锁 |
| `usage.log` | 候选选择与命令执行记录，用于排序 |
| `history.tsv` | 包含工作目录的命令历史，文件权限 `0600` |

删除 `usage.log` 可重置排序，删除 `history.tsv` 可清空用于自动建议的历史记录；重开 Shell 后生效。历史包含输入的命令文本，应按自己的需要管理。

卸载时，从已启用 Shell 的配置中移除 rfig 加载行，Fish 可移除 `conf.d/rfig.fish`。源码或二进制安装删除 `~/.local/bin/rfig`，Homebrew 安装执行 `brew uninstall rfig`。确认不再需要数据后可删除 `~/.config/rfig/`，重开 Shell。

## 常见问题

### rfig 是 Fig 的替代品吗？

rfig 是受 Fig 启发的独立开源终端自动补全项目，与 Fig 无隶属关系，也不声称兼容 Fig 的全部功能或规范。

### rfig 支持所有命令吗？

不保证。候选范围取决于本机可用的补全定义、脚本、生成器和帮助缓存。有些命令可能没有候选，或只能获得基于帮助文本的基础候选。

### rfig 会使用 AI 吗？

不会。补全、历史建议和使用排序均在本机运行，无需 AI 服务或云端账号。

## 测试验证

测试通过 tmux/PTY 发送真实按键，检查屏幕及命令实际收到的参数，覆盖灰色内联自动建议、Command 键选择、排序、目录历史、特殊字符、多行粘贴和进程清理。

```sh
cargo test
cargo build
sh tests/docker/run.sh
RFIG_TEST_IMAGE=rust:1.91-slim BUILD_BASH44=0 sh tests/docker/run.sh
```

Docker 使用隔离 HOME、普通用户、资源限制和断网测试，结果保存在 `target/docker-results/`。运行方式见 [测试说明](tests/docker/README.md)。
