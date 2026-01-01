# MiceTimer

**MiceTimer** 是一个专为 Android 环境设计的轻量级、高性能定时器守护进程（Daemon），由 Rust 编写。

它旨在解决 Android 平台上传统 Shell 脚本使用 `sleep` 循环时，因系统进入深度睡眠（Doze Mode）导致计时不准或任务被挂起的问题。

## 🌟 核心特性

- **精准计时**：基于 Linux `timerfd` 原生系统调用，使用 `CLOCK_BOOTTIME` 时钟，确保在手机休眠期间依然能够精准倒计时。
- **唤醒保证**：内置 Android WakeLock 持久化支持。在任务触发时自动申请唤醒锁，确保 CPU 在任务执行期间保持活跃，执行完毕后自动释放。
- **Systemd 体验**：采用类似 Systemd Timer 的扁平化 TOML 配置语法，清晰易读。
- **动态加载**：自动扫描配置目录（默认 `/data/adb/micetimer/timers.d/`），无需重启程序即可通过添加文件增加任务。
- **极低开销**：Rust 零成本抽象，内存占用极低，适合作为长期后台进程运行。

## 🛠️ 配置说明

配置文件采用 `.toml` 格式，放置在 `timers.d/` 目录下。文件名即为任务名。

示例：`/data/adb/micetimer/timers.d/fcm-hosts.toml`

```toml
Description = "每隔 6 小时同步一次 FCM Hosts"

# 要执行的命令（建议使用绝对路径）
Exec = "/system/bin/fcm-update"

# 开机后等待多久进行第一次执行（例如 5m, 10s, 1h）
OnBootSec = "5m"

# 上次执行完成后，间隔多久再次执行
OnUnitActiveSec = "6h"

# 运行期间是否持有唤醒锁 (默认为 true)
WakeLock = true
```

## 📦 安装方式

本项目目前主要作为 **KernelSU (KSU)** 模块分发：

1. 从 [Releases](https://github.com/Mice-Tailor-Infra/micetimer/releases/tag/nightly) 下载最新的 `micetimer-ksu-nightly.zip`。
2. 在 KernelSU 管理管理器中安装。
3. 模块会自动创建 `/data/adb/micetimer/` 目录结构。
4. 将你的定时器配置文件放入 `/data/adb/micetimer/timers.d/` 即可。

## 🏗️ 项目架构

本仓库属于 [Mice-Tailor-Infra](https://github.com/Mice-Tailor-Infra) 基础设施的一部分，与其他项目配合实现网络自动化优化。

- **源码**：`src/main.rs` (核心调度逻辑)
- **模板**：`ksu-template/` (KSU 模块结构)
- **CI**：GitHub Actions 自动交叉编译 `aarch64-linux-android` 产物。

## 📄 开源协议

MIT License
