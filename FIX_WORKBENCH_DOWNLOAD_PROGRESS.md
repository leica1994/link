# 修复：工作台下载视频进度显示不同步

## 问题描述

**症状**：
- 视频文件在主页已经下载完成（显示"已下载"状态）
- 点击工作台"开始执行"按钮后
- 工作台第一步"下载视频"的进度停留在 2%，但实际上视频已经就绪
- 上方视频文件区域显示"已下载"，下方工作台步骤显示 2%，状态不一致

## 根本原因

1. **视频已存在的快速路径没有发送进度事件**
   - 当工作台执行到"下载视频"阶段时，会调用 `ensure_video_downloaded()` 函数
   - 该函数检测到视频已下载并存在于磁盘时，直接返回视频对象（第 664 行）
   - **关键问题**：没有触发 `home-video-download-progress` 事件通知前端

2. **前端依赖进度事件同步工作台UI**
   - 前端的 `syncWorkbenchDownloadProgress()` 函数（Home.vue:2350）监听 `home-video-download-progress` 事件
   - 只有收到该事件，才会更新工作台下载视频阶段的进度
   - 由于后端没有发送事件，前端工作台进度停留在初始值 2%

## 修复方案

在 `src-tauri/src/home_workbench.rs` 的 `ensure_video_downloaded()` 函数中：

**修改位置**：第 656-677 行

**修改内容**：
当检测到视频已存在时，主动发送一个进度完成事件：

```rust
if let Some(video) = task
    .downloaded_video
    .filter(|video| Path::new(&video.file_path).is_file())
{
    // 视频已存在，发送进度完成事件以同步工作台UI
    let _ = app.emit(
        "home-video-download-progress",
        serde_json::json!({
            "taskId": task.id,
            "kind": "video",
            "key": "video",
            "progress": 100,
            "status": "done",
            "message": "视频文件已就绪",
            "downloadedBytes": null as Option<u64>,
            "totalBytes": null as Option<u64>,
            "language": null as Option<String>,
            "sourceKind": null as Option<String>,
        }),
    );
    return Ok(video);
}
```

## 修复原理

1. **事件结构对齐**：发送的事件结构与 `HomeVideoDownloadProgress` 类型完全一致（home_tasks.rs:177-190）
2. **前端自动同步**：前端的 `syncWorkbenchDownloadProgress()` 函数接收到该事件后：
   - 检查条件满足（kind='video', taskId 匹配, workbench status='running', stage status='active'）
   - 将工作台"下载视频"阶段的进度更新为 100%
   - 重新计算工作台总体进度

3. **用户体验改善**：
   - 工作台启动后，下载视频阶段立即显示为 100%（或跳过显示"已就绪"）
   - 上下两处的状态保持一致

## 测试步骤

1. 在主页视频详情中先下载视频文件（等待下载完成）
2. 确认视频文件区域显示"已下载"状态
3. 点击工作台"开始执行"按钮
4. **预期结果**：工作台第一步"下载视频"的进度应该快速从 2% 跳到 100%，然后进入下一步
5. **修复前**：进度停留在 2%

## 相关文件

- `src-tauri/src/home_workbench.rs` - 工作台执行逻辑（修改点）
- `src-tauri/src/home_tasks.rs` - 视频下载逻辑和进度事件定义
- `src/views/Home.vue` - 前端工作台UI和进度同步逻辑

## 备注

- 该修复不影响正常的视频下载流程（视频未下载时的执行路径）
- 只是在视频已存在的情况下补充发送进度事件，确保前后端状态一致
- 事件发送失败不会影响主流程（使用 `let _ =` 忽略结果）
