# rfig

rfig 是一个面向 macOS zsh 的终端补全菜单。输入命令时，它会在当前提示符下方显示候选；按方向键选择后，候选会插入命令行，继续输入即可进入下一层补全。

![rfig 在 zsh 中自动补全 Git 命令和目录路径的演示](assets/rfig-demo.gif)

候选优先来自当前 zsh 会话的补全定义，也可复用本机安装的 zsh、Fish、Bash 补全脚本和命令自身的补全入口，因此分支、路径等动态内容会随环境变化。rfig 不内置一份固定的命令补全库。

## 功能列表

| 功能 | 状态 | 说明 |
| --- | --- | --- |
| 输入时自动显示补全菜单 | ✅ | 输入或删除后立即更新，不需要按 `Tab` |
| 多级动态补全 | ✅ | 使用当前 zsh 的补全定义，例如 Git 分支和文件路径 |
| 分类图标与终端主题配色 | ✅ | 区分命令、子命令、参数、选项和开关 |
| 本机命令扫描与后台分析 | ✅ | 安装时扫描可执行文件和 zsh/Fish/Bash 补全定义；后台逐个验证和补齐候选 |
| 复用本机补全脚本 | ✅ | 当前 zsh 定义优先；可发现尚未注册的 zsh 脚本，复用 Fish/Bash 定义 |
| 全终端支持 | ⏳ | 当前仅 zsh 集成 ✅；其他 Shell 与终端环境留待后续实现，现阶段优先完善 zsh |
| 按使用习惯调整排序 | ✅ | 记录选中项和执行过的命令，当前命令上下文的常用候选优先展示 |
| 历史灰字建议 | ✅ | 自行记录执行过的命令，按当前目录匹配；灰字和下方菜单可同时显示 |

## 安装

当前支持 macOS zsh。安装后打开新的 zsh 终端即可使用。

### 下载二进制安装（无需 Rust）

从 [最新发布页](https://github.com/tamia6/rfig/releases/latest)下载 `rfig-v0.1.2-macos-universal.tar.gz`。下载位置不限，解压后运行包内的安装脚本：

```sh
tar -xzf rfig-v0.1.2-macos-universal.tar.gz
cd rfig-v0.1.2
sh install.sh
```

安装包内含通用二进制、`install.sh` 和 zsh 集成脚本。`install.sh` 会校验文件，把二进制移动到 `~/.local/bin/rfig`，并运行 `rfig setup`。如果 `~/.local/bin` 不在 `$PATH`，脚本会在 zsh 配置中添加它。

### 从源码安装

需要 Rust 和 Cargo。在项目目录运行：

```sh
./install.sh
```

同一个 `install.sh` 在源码目录中会用 Cargo 编译，再放到 `~/.local/bin/rfig` 并运行 `rfig setup`。

### Homebrew 安装

使用 Homebrew Tap 安装：

```sh
brew install tamia6/tap/rfig
rfig setup
```

`rfig setup` 会根据当前 `$PATH` 扫描可执行命令（包括符号链接），索引 zsh 已注册定义及本机 Fish/Bash 补全脚本，在 `~/.zshrc`（或 `$ZDOTDIR/.zshrc`）加入 shell 集成，并启动后台优化。可重复运行；后台任务不会并发执行，更新命令目录也不会重复添加 `source` 行。

如果之前用 `./install.sh` 安装过，`~/.local/bin/rfig` 可能排在 Homebrew 前面。可用 `command -v rfig` 检查；若要配置 Homebrew 版本，运行 `"$(brew --prefix rfig)/bin/rfig" setup`。

安装会快速返回。后台先跳过已有补全定义的命令，再对常见用户安装目录中的命令逐个读取 `--help` / `-h` 并验证补全生成器。系统目录和未知来源目录仍会进入命令目录，但不会自动执行；后台命令在 macOS 沙盒中禁止写文件和访问网络，每次运行限时 500 毫秒，并限制 CPU 时间和输出大小。输入时只读取已生成的帮助缓存；后台尚未完成的命令可能暂时没有候选。可运行 `cat ~/.config/rfig/enrich.status` 查看进度，或用 `rfig analyze <命令>` 单独更新缓存。第三方程序即使传入 `--help` 也可能有副作用，这些隔离措施不能保证其绝对安全。

## 使用

运行 `rfig -h` 或 `rfig --help` 可查看命令、参数和示例；`rfig setup --help` 只显示帮助，不会重新扫描。rfig 自身也支持输入时补全：首级显示公开命令，`rfig analyze ` 后显示本机扫描到的命令名。

例如，输入 `git` 时会出现子命令；选中 `checkout` 后，只提示当前仓库的本地或远端分支。需要检出文件时，输入 `git checkout -- ` 可使用路径补全。输入 `kubectl --` 会看到选项；对单个命令运行 `rfig analyze kubectl` 后可进一步区分带值选项与布尔开关。

| 按键 | 行为 |
| --- | --- |
| `↑` / `↓` | 移动选中项 |
| `→` | 有菜单时插入选中项；没有菜单时接受灰字建议 |
| `Ctrl+E` | 接受灰字建议，不执行命令 |
| `Enter` | 有灰字时执行补全后的命令；否则执行当前输入，不会选中菜单项 |
| `Tab` | 保留 zsh 原有的补全行为 |

灰字来自 rfig 自己记录的命令历史，只匹配当前工作目录和当前输入，并显示尚未输入的剩余部分；它不会参与菜单候选的计算。例如输入 `gi` 时，灰字可能是 `t checkout master`，菜单仍只根据 `gi` 展示命令。切换目录后会使用新目录的记录；新安装时历史从空记录开始，不会把无法确定目录的旧 zsh 历史误归入当前目录。以空格开头的命令在开启 zsh `HIST_IGNORE_SPACE` 时不会记录。使用 rfig 内置灰字功能时，请先从 shell 配置中移除旧 `zsh-autosuggestions` 插件的加载行，再按需卸载插件。

候选最多显示五行，超出的仍可用方向键查看。按当前命令上下文的使用次数排序；选中一次的权重高于执行一次，次数相同时子命令排在选项之前。最多前三个有使用记录的候选以 `① ② ③` 标记；若只有两个常用候选，只显示 `① ②`，其他候选照常显示类别图标。输入 `-` 时只显示单横线选项，输入 `--` 时只显示双横线选项；这条规则适用于所有命令。其余图标用于提示类别：

| 类别 | 图标 | 颜色 |
| --- | --- | --- |
| 命令 | `⌘` | 青色 |
| 子命令 | `↳` | 洋红 |
| 参数 | `●` | 绿色 |
| 带值选项 | `◇` | 蓝色 |
| 布尔开关 | `⚑` | 黄色 |

颜色取自终端的 ANSI 调色板，数字图标沿用候选原有类别的颜色；选中项由图标前的 `→` 指针和青色文字标识。上下切换时，箭头会短暂右跳并变粗，当前选项和相邻选项会右移、加粗后复位，模拟程序坞的回弹效果。尚未分析到的选项暂按前缀分类：`--` 显示为 `◇`，单 `-` 显示为 `⚑`。

## 补全来源与本地数据

rfig 的候选顺序是：当前 zsh 已注册的定义 → `$fpath` 中尚未注册的 zsh 脚本 → 命令生成的 zsh 脚本 → 已识别的命令补全协议（目前支持 Cobra `__complete`）→ 已安装的 Fish/Bash 补全脚本 → `--help` / `-h` 解析缓存。Fish 与 Bash 脚本在输入时执行，因此它们提供的动态参数也能随当前目录或命令上下文变化；这并不意味着 rfig 已集成到 Fish 或 Bash 的交互提示符。没有可用来源时不显示菜单。

安装和分析会在本机保存以下数据：

| 路径 | 用途 |
| --- | --- |
| `~/.config/rfig/commands.txt` | 安装时扫描到的可执行命令名 |
| `~/.config/rfig/supported.txt` | 扫描到的 zsh 已注册定义及 Fish/Bash 补全脚本对应的命令名 |
| `~/.config/rfig/generated/*.zsh` | 后台从命令自身的补全生成入口获取的 zsh 脚本 |
| `~/.config/rfig/protocol/*` | 后台识别出的命令补全协议 |
| `~/.config/rfig/options/*.tsv` | 帮助信息分析出的选项类别 |
| `~/.config/rfig/fallback/*.tsv` | 没有 zsh 补全定义时，从帮助信息提取的候选 |
| `~/.config/rfig/usage.log` | 本地的候选选择与命令执行记录，用于调整排序 |
| `~/.config/rfig/history.tsv` | rfig 自行记录的命令历史，包含执行时的工作目录，仅供灰字建议使用，权限为 `0600` |

后台对没有现成补全定义的用户安装命令尝试 `--help`，必要时尝试 `-h`；若帮助文本明确指出独立的全局选项列表，也会读取该列表。它会检测 `completion zsh`、`completion -s zsh`、`completion --shell zsh` 等生成器并检查脚本语法，也会识别 Cobra `__complete`。缓存早于可执行文件修改时间时，下次运行 `setup` 会重新分析。系统目录中的命令仍可通过原生补全定义使用，但不会在后台执行。Fish/Bash 动态补全在输入时由对应 Shell 生成，可能仍有少量开销。

手动更新：

```sh
rfig setup             # PATH 变化后重新扫描，并启动串行后台优化
rfig analyze kubectl   # 明确更新一个命令的帮助和生成器缓存
```

排序和历史记录只保存在本机；删除 `~/.config/rfig/usage.log` 可重置候选排序，删除 `~/.config/rfig/history.tsv` 可清空灰字历史。重开 zsh 后生效。

## 卸载

从 `~/.zshrc`（或 `$ZDOTDIR/.zshrc`）删除 `rfig setup` 添加的 `source` 行。源码安装再删除 `~/.local/bin/rfig`；Homebrew 安装运行 `brew uninstall rfig`。如需清理本地缓存，可删除 `~/.config/rfig/`。

## 开发验证

```sh
cargo test
cargo build
python3 tests/zsh_integration.py
python3 tests/zsh_usage.py
python3 tests/zsh_history_suggestions.py
python3 tests/setup_safety.py
python3 tests/self_completion.py
```
