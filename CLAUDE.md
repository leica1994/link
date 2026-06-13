# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Link 是一个基于 Tauri 2 的桌面应用，提供视频转录、字幕翻译、AI 配音和 YouTube 频道监控功能。

**技术栈**：
- 前端：Vue 3 (Composition API) + TypeScript + Vite
- 后端：Rust + Tauri 2
- 数据存储：SQLite (rusqlite)
- 外部依赖：yt-dlp（YouTube 下载）、PyTorch (tch) 用于音频处理

## Development Commands

### 前端开发
```bash
# 启动前端开发服务器（单独运行，用于调试前端）
npm run dev

# 前端构建（生成 dist 目录）
npm run build

# 类型检查
npm run preview
```

### Tauri 应用开发
```bash
# 启动完整应用（前端 + Rust 后端）
npm run tauri dev

# 构建桌面应用（生成 NSIS 安装包）
npm run tauri build
```

### Rust 后端开发
```bash
# 在 src-tauri 目录下
cd src-tauri

# 运行 Rust 测试
cargo test

# 检查代码（不构建）
cargo check

# 构建 Rust 库
cargo build

# 格式化代码
cargo fmt
```

## Architecture

### 后端架构（Rust）

**核心服务模块**（通过 Tauri 状态管理注入）：

| 服务模块 | 文件 | 职责 |
|---------|------|------|
| `SettingsStore` | `settings.rs` | 应用配置持久化 |
| `AiService` | `ai.rs` | LLM 集成（OpenAI/Anthropic），支持并发控制和速率限制 |
| `AppLogger` | `app_log.rs` | 日志管理 |
| `DubbingTtsScheduler` | `dubbing.rs` | 配音 TTS 任务调度 |
| `YoutubeMonitorService` | `youtube_monitor.rs` | YouTube 频道监控和更新检查 |

**功能模块**：

- `transcription.rs` - 音视频转文字服务，生成字幕文件
- `subtitle_translation.rs` - 字幕翻译（支持 AI 翻译）
- `dubbing.rs` / `dubbing_alignment.rs` / `dubbing_compose.rs` - 完整的配音工作流（模型管理、语音合成、时间对齐、音频合成）
- `home_tasks.rs` - 首页视频任务管理（添加、删除、下载）
- `home_workbench.rs` - 首页工作台（多输入文件处理）
- `htdemucs.rs` - 音频分离（人声/背景音乐）
- `subtitle_ai.rs` - AI 字幕处理辅助功能
- `app_paths.rs` - 应用路径管理（数据目录、日志目录等）

**数据库**：
- 使用 SQLite 存储：YouTube 频道/视频数据、视频任务、配音模型配置
- 数据库文件位置由 `app_paths` 模块管理

**Tauri 命令处理**：
- 43 个命令注册在 `lib.rs` 的 `invoke_handler` 中
- 命令命名约定：`{动词}_{模块}_{资源}`（如 `list_youtube_channels`）
- 前端通过 `@tauri-apps/api` 的 `invoke()` 调用

**外部工具集成**：
- `yt-dlp` - YouTube 视频信息获取和下载
  - 通过 `Command::new()` 调用
  - 配置：30s socket 超时，zh-CN 语言偏好
  - 进度监控通过 stdout 解析

### 前端架构（Vue 3）

**路由结构**：

| 路由 | 组件 | 功能 |
|-----|------|------|
| `/` | `Home.vue` | 首页（任务列表 + 工作台） |
| `/tasks/:taskId` | `Home.vue` | 任务详情页 |
| `/translate` | `Translate.vue` | 字幕翻译页面（保持状态） |
| `/dubbing` | `Dubbing.vue` | 配音页面（保持状态） |
| `/youtube-monitor` | `YoutubeMonitor.vue` | YouTube 监控列表 |
| `/youtube-monitor/:channelId` | `YoutubeMonitor.vue` | 频道详情 |
| `/settings` | `Settings.vue` | 设置页面 |

**核心组件**：
- `TitleBar.vue` - 自定义窗口标题栏（无系统装饰）
- `Sidebar.vue` - 侧边栏导航

**前后端通信**：
- 通过 Tauri 的 `invoke()` 调用 Rust 命令
- 通过 Tauri 事件系统接收后端推送（如进度更新）

## Key Implementation Details

### AI 服务并发控制
- `AiService` 实现了基于信号量的并发限制（1-100 并发）
- 支持速率限制自动退避（15-90 秒）
- 支持流式和非流式响应
- 请求失败自动重试（最多 3 次）

### YouTube 监控机制
- `YoutubeMonitorService` 使用互斥锁防止同一频道并发刷新
- 使用 `RefreshGuard` RAII 模式管理刷新状态
- 通过 WebSocket 事件向前端推送实时更新

### 转录和配音工作流
1. **转录**：音视频 → 音频分离 → 语音识别 → 字幕文件
2. **翻译**：字幕文件 → AI 翻译 → 翻译字幕文件
3. **配音**：
   - 准备素材（音频分离、字幕对齐）
   - TTS 生成（批量、可预览）
   - 时间对齐（调整语速）
   - 音频合成（配音 + 背景音乐）

### 数据持久化
- **设置**：存储在应用数据目录的 JSON 文件
- **任务数据**：SQLite 数据库
- **日志**：文本日志文件（可通过 `open_log_directory` 命令打开）

## Common Tasks

### 添加新的 Tauri 命令
1. 在对应的 Rust 模块中定义函数（标注 `#[tauri::command]`）
2. 在 `lib.rs` 的 `use` 语句中导入函数
3. 在 `invoke_handler` 的 `generate_handler![]` 中注册
4. 前端通过 `invoke('命令名', { 参数 })` 调用

### 调试前后端通信
- Rust 日志：使用 `AppLogger` 记录（可在设置中打开日志目录）
- 前端控制台：Chrome DevTools（Tauri 应用内按 F12）
- Tauri 命令调试：在 Rust 函数中打印到 stdout/stderr

### 更新依赖
```bash
# 前端依赖
npm update

# Rust 依赖
cd src-tauri
cargo update
```

### 构建优化
- 前端生产构建会进行 TypeScript 类型检查（`vue-tsc --noEmit`）
- Rust Release 构建会进行优化（Tauri 自动处理）
- Windows 安装包使用 NSIS，配置在 `tauri.conf.json`

## External Dependencies

### 必需的系统工具
- **yt-dlp** - YouTube 下载工具，必须在 PATH 中可访问
  - 检查：`yt-dlp --version`
  - 安装：https://github.com/yt-dlp/yt-dlp

### 可选的系统工具
- **FFmpeg** - 音视频处理（yt-dlp 依赖）
- PyTorch LibTorch - 用于 `tch` crate（音频处理模型）

## Constraints

- **单实例应用**：通过 `tauri-plugin-single-instance` 确保只有一个实例运行
- **WebView 数据隔离**：自定义 WebView 数据目录（非系统默认位置）
- **Windows 专用配置**：
  - NSIS 安装器，当前用户安装模式
  - 简体中文界面
  - 自定义安装钩子（`nsis/hooks.nsh`）
- **无窗口装饰**：使用自定义标题栏，禁用系统标题栏
- **右键菜单禁用**：前端全局禁用 contextmenu
