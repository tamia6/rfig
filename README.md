# rfig

rfig 是一个面向 macOS zsh 的终端补全菜单。输入命令时，它会在当前提示符下方显示候选；按方向键选择后，候选会插入命令行，继续输入即可进入下一层补全。

候选来自当前 zsh 会话的补全定义，因此分支、路径等动态内容会随环境变化。rfig 不内置一份固定的命令补全库。

## 功能列表

| 功能 | 状态 | 说明 |
| --- | --- | --- |
| 输入时自动显示补全菜单 | ✅ | 不需要按 `Tab` |
| 多级动态补全 | ✅ | 使用当前 zsh 的补全定义，例如 Git 分支和文件路径 |
| 分类图标与终端主题配色 | ✅ | 区分命令、子命令、参数、选项和开关 |
| 本机命令扫描与后台分析 | ✅ | 安装时扫描，并尝试从帮助信息改进选项分类 |
| 全终端支持 | ⏳ | 当前仅 zsh 集成 ✅；其他 Shell 与终端环境留待后续实现，现阶段优先完善 zsh |
| 按使用习惯调整排序 | ⏳ | 当前尚未记录输入和选择历史 |

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

`rfig setup` 会根据当前 `$PATH` 扫描命令，读取 zsh 补全定义，在 `~/.zshrc`（或 `$ZDOTDIR/.zshrc`）加入 shell 集成，并在后台分析帮助信息。可重复运行，更新命令目录时不会重复添加 `source` 行。

如果之前用 `./install.sh` 安装过，`~/.local/bin/rfig` 可能排在 Homebrew 前面。可用 `command -v rfig` 检查；若要配置 Homebrew 版本，运行 `"$(brew --prefix rfig)/bin/rfig" setup`。

安装时先扫描 `$PATH` 中的可执行命令，随后在后台分析具有 zsh 补全定义的命令。扫描结束即可使用基础补全，不必等待分析完成。

## 使用

例如，输入 `git` 时会出现子命令；选中 `checkout` 后，可以继续补全当前仓库的分支或路径。输入 `kubectl --` 会看到选项，后台分析完成后可进一步区分带值选项与布尔开关。

| 按键 | 行为 |
| --- | --- |
| `↑` / `↓` | 移动选中项 |
| `→` | 将选中项插入当前命令行，并继续显示下一层候选 |
| `Enter` | 执行**已输入的命令**，不会强制选中候选 |
| `Tab` | 保留 zsh 原有的补全行为 |

候选最多显示五行，子命令排在 `--` 选项之前。图标用于提示类别：

| 类别 | 图标 | 颜色 |
| --- | --- | --- |
| 命令 | `⌘` | 青色 |
| 子命令 | `↳` | 洋红 |
| 参数 | `●` | 绿色 |
| 带值选项 | `◇` | 蓝色 |
| 布尔开关 | `⚑` | 黄色 |

颜色取自终端的 ANSI 调色板；选中行使用终端的反色样式。后台尚未分析到的选项暂按前缀分类：`--` 显示为 `◇`，单 `-` 显示为 `⚑`。

## 补全来源与本地数据

rfig 实时读取当前 zsh 的补全定义。若命令没有补全定义，就不会显示该命令的候选；当前 shell 中加载的自定义补全也可以使用。补全层级由命令自身的定义决定，没有固定层数。

安装和分析会在本机保存以下数据：

| 路径 | 用途 |
| --- | --- |
| `~/.config/rfig/commands.txt` | 安装时扫描到的可执行命令名 |
| `~/.config/rfig/supported.txt` | 同时具有 zsh 补全定义的命令名，供批量分析使用 |
| `~/.config/rfig/options/*.tsv` | 帮助信息分析出的选项类别 |

后台分析会对支持的命令尝试 `--help`，必要时尝试 `-h`；`kubectl` 的全局选项还会读取 `kubectl options`。每次查询限时 500 毫秒，结果逐个写入缓存；正在使用的 zsh 会话会在后续输入时读取新结果。帮助信息不足时，分类仍可能不准确。

手动更新：

```sh
rfig setup             # PATH 变化后重新扫描并后台分析
rfig analyze           # 前台重新分析所有受支持命令
rfig analyze kubectl   # 只更新一个命令
```

当前版本**尚未记录用户输入或候选选择习惯**，也不会据此调整候选顺序。

## 卸载

从 `~/.zshrc`（或 `$ZDOTDIR/.zshrc`）删除 `rfig setup` 添加的 `source` 行。源码安装再删除 `~/.local/bin/rfig`；Homebrew 安装运行 `brew uninstall rfig`。如需清理本地缓存，可删除 `~/.config/rfig/`。

## 开发验证

```sh
cargo test
cargo build
python3 tests/zsh_integration.py
```
