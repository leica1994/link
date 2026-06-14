# Link

<div align="center">

**一款基于 Tauri 2 的全能视频处理桌面应用**

提供视频转录、字幕翻译、AI 配音和 YouTube 频道监控功能

[功能特性](#功能特性) • [快速开始](#快速开始) • [开发指南](#开发指南) • [项目架构](#项目架构)

</div>

---

## 功能特性

### 🎙️ 视频转录
- 音视频文件转文字，自动生成字幕文件
- 支持音频分离（人声/背景音乐）
- 多格式支持（mp4, mp3, wav 等）

### 🌐 字幕翻译
- AI 驱动的智能字幕翻译
- 支持多种语言对
- 保持时间轴同步

### 🎬 AI 配音
- 完整的配音工作流：
  - 音频分离（人声/背景音）
  - TTS 语音合成（批量生成、可预览）
  - 时间对齐（智能调整语速）
  - 音频合成（配音 + 背景音乐）
- 支持多种 TTS 模型
- 配音模型管理

### 📺 YouTube 监控
- YouTube 频道订阅和监控
- 自动检测新视频更新
- 视频信息获取和下载（基于 yt-dlp）
- 实时推送更新通知

### ⚙️ 其他特性
- 任务管理（添加、删除、进度追踪）
- 工作台（批量处理多个输入文件）
- 自定义窗口标题栏（无系统装饰）
- 单实例运行保护
- 应用日志管理

---

## 技术栈

### 前端
- **框架**: Vue 3 (Composition API)
- **语言**: TypeScript
- **构建工具**: Vite
- **路由**: Vue Router

### 后端
- **框架**: Tauri 2
- **语言**: Rust
- **数据库**: SQLite (rusqlite)
- **音频处理**: PyTorch (tch)

### 外部依赖
- **yt-dlp**: YouTube 视频下载
- **FFmpeg**: 音视频处理
- **AI 服务**: OpenAI / Anthropic

---

## 系统要求

### 必需工具
- **Node.js**: 18.x 或更高版本
- **Rust**: 1.70 或更高版本
- **yt-dlp**: 最新版本（必须在 PATH 中可访问）
  ```bash
  # 检查是否安装
  yt-dlp --version
  ```

### 可选工具
- **FFmpeg**: 用于音视频处理（yt-dlp 依赖）
- **PyTorch LibTorch**: 用于音频处理模型

### 操作系统
- Windows 10/11（当前主要支持平台）
- macOS / Linux（理论支持，需测试）

---

## 快速开始

### 1. 克隆项目
```bash
git clone https://github.com/leica1994/link.git
cd link
```

### 2. 安装依赖
```bash
# 安装前端依赖
npm install
```

### 3. 运行开发模式
```bash
# 启动完整应用（前端 + Rust 后端）
npm run tauri dev
```

### 4. 构建应用
```bash
# 构建桌面应用（生成 NSIS 安装包）
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`

---

## 开发指南

### 项目结构
```
link/
├── src/                    # Vue 前端源码
│   ├── components/         # 组件
│   ├── views/              # 页面视图
│   ├── router/             # 路由配置
│   └── assets/             # 静态资源
├── src-tauri/              # Rust 后端源码
│   ├── src/
│   │   ├── lib.rs          # 入口文件（命令注册）
│   │   ├── ai.rs           # AI 服务（LLM 集成）
│   │   ├── transcription.rs # 转录服务
│   │   ├── subtitle_translation.rs # 字幕翻译
│   │   ├── dubbing.rs      # 配音核心逻辑
│   │   ├── youtube_monitor.rs # YouTube 监控
│   │   └── ...             # 其他功能模块
│   ├── Cargo.toml          # Rust 依赖
│   └── tauri.conf.json     # Tauri 配置
├── package.json            # 前端依赖
├── CLAUDE.md               # 项目开发指南
└── README.md               # 本文件
```

### 开发命令

#### 前端开发
```bash
# 启动前端开发服务器（单独运行，用于调试前端）
npm run dev

# 前端构建（生成 dist 目录）
npm run build

# 类型检查
npm run preview
```

#### Rust 后端开发
```bash
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

### 添加新的 Tauri 命令

1. **定义命令函数**（在对应的 Rust 模块中）
   ```rust
   #[tauri::command]
   pub fn my_command(param: String) -> Result<String, String> {
       // 实现逻辑
       Ok("success".to_string())
   }
   ```

2. **在 lib.rs 中导入和注册**
   ```rust
   // 导入
   use crate::my_module::my_command;

   // 注册到 invoke_handler
   tauri::Builder::default()
       .invoke_handler(tauri::generate_handler![
           // ... 其他命令
           my_command,
       ])
   ```

3. **前端调用**
   ```typescript
   import { invoke } from '@tauri-apps/api/core'
   
   const result = await invoke('my_command', { param: 'value' })
   ```

### 调试技巧

#### 后端调试
- **日志位置**: 通过设置页面的"打开日志目录"按钮访问
- **控制台输出**: 在命令行中查看 `cargo run` 或 `npm run tauri dev` 的输出
- **断点调试**: 使用 VS Code + rust-analyzer 的调试功能

#### 前端调试
- **Chrome DevTools**: 在应用中按 `F12` 打开开发者工具
- **Vue DevTools**: 安装浏览器扩展进行 Vue 组件调试
- **网络请求**: 查看 Tauri 命令调用和响应

---

## 项目架构

### 核心服务（Rust）

| 服务模块 | 文件 | 职责 |
|---------|------|------|
| `SettingsStore` | `settings.rs` | 应用配置持久化 |
| `AiService` | `ai.rs` | LLM 集成（OpenAI/Anthropic），并发控制和速率限制 |
| `AppLogger` | `app_log.rs` | 日志管理 |
| `DubbingTtsScheduler` | `dubbing.rs` | 配音 TTS 任务调度 |
| `YoutubeMonitorService` | `youtube_monitor.rs` | YouTube 频道监控 |

### 路由结构（Vue）

| 路由 | 组件 | 功能 |
|-----|------|------|
| `/` | `Home.vue` | 首页（任务列表 + 工作台） |
| `/tasks/:taskId` | `Home.vue` | 任务详情页 |
| `/translate` | `Translate.vue` | 字幕翻译页面 |
| `/dubbing` | `Dubbing.vue` | 配音页面 |
| `/youtube-monitor` | `YoutubeMonitor.vue` | YouTube 监控列表 |
| `/youtube-monitor/:channelId` | `YoutubeMonitor.vue` | 频道详情 |
| `/settings` | `Settings.vue` | 设置页面 |

### 数据流

```
前端 (Vue 3)
    ↓ invoke('command', params)
Tauri IPC
    ↓
后端 (Rust)
    ↓ 调用服务
核心服务 (AiService, YoutubeMonitorService, etc.)
    ↓ 持久化
SQLite / JSON 配置文件
```

### AI 服务特性
- **并发控制**: 基于信号量的并发限制（1-100 并发）
- **速率限制**: 自动退避（15-90 秒）
- **容错机制**: 请求失败自动重试（最多 3 次）
- **流式支持**: 支持流式和非流式响应

---

## 常见问题

### Q: 运行时提示找不到 yt-dlp
**A**: 确保 yt-dlp 已安装并在系统 PATH 中：
```bash
# Windows (使用 Scoop)
scoop install yt-dlp

# 或下载可执行文件放到 PATH 目录
# https://github.com/yt-dlp/yt-dlp
```

### Q: 构建失败，提示缺少 LibTorch
**A**: 音频处理需要 PyTorch LibTorch。可以：
1. 安装完整的 PyTorch（会包含 LibTorch）
2. 或者禁用相关功能（修改 Cargo.toml）

### Q: 前端热更新不生效
**A**: 
```bash
# 重启开发服务器
npm run tauri dev
```

### Q: 如何查看应用日志
**A**: 在设置页面点击"打开日志目录"按钮，或手动访问应用数据目录。

### Q: YouTube 监控不工作
**A**: 
1. 确认 yt-dlp 已正确安装
2. 检查网络连接
3. 查看日志文件中的错误信息

---

## 推荐 IDE 配置

- **VS Code** + 扩展：
  - [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar)
  - [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
  - [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

---

## 许可证

本项目采用 [MIT 许可证](./LICENSE)。

---

## 贡献

欢迎提交 Issue 和 Pull Request！

更多开发细节请参考 [CLAUDE.md](./CLAUDE.md)。
