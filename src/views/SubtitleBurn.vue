<template>
  <div class="page translate-page subtitle-burn-page">
    <header class="translate-header">
      <h1 class="page-title">字幕烧录</h1>
    </header>

    <main class="translate-workspace subtitle-burn-workspace">
      <div class="subtitle-burn-grid">
        <section class="settings-section" aria-labelledby="burn-video-title">
          <div id="burn-video-title" class="section-heading">
            <Video aria-hidden="true" />
            <span>视频输入</span>
          </div>

          <div class="settings-panel translate-drop-panel subtitle-burn-drop-panel">
            <div
              ref="videoDropZoneRef"
              class="translate-drop-zone subtitle-burn-drop-zone"
              :class="{ 'drag-active': dragTarget === FileInputTarget.Video }"
              @dragenter.prevent="dragTarget = FileInputTarget.Video"
              @dragover.prevent
              @dragleave.prevent="clearNativeDragTarget(FileInputTarget.Video)"
              @drop.prevent="handleBrowserDrop(FileInputTarget.Video, $event)"
            >
              <UploadCloud class="translate-drop-icon" :stroke-width="2.1" aria-hidden="true" />
              <div class="translate-drop-copy">
                <span class="translate-drop-title">选择或拖入视频</span>
                <span class="translate-drop-subtitle">支持 MP4、MOV、MKV、AVI、FLV、WMV、WEBM、M4V</span>
              </div>
              <button class="settings-action" type="button" :disabled="isBurning" @click="selectVideoFile">
                选择视频
              </button>
            </div>

            <div class="translate-file-strip" aria-label="当前视频">
              <FileVideo :stroke-width="2.1" aria-hidden="true" />
              <span>{{ selectedVideoName }}</span>
            </div>
          </div>
        </section>

        <section class="settings-section" aria-labelledby="burn-subtitle-title">
          <div id="burn-subtitle-title" class="section-heading">
            <Captions aria-hidden="true" />
            <span>字幕输入</span>
          </div>

          <div class="settings-panel translate-drop-panel subtitle-burn-drop-panel">
            <div
              ref="subtitleDropZoneRef"
              class="translate-drop-zone subtitle-burn-drop-zone"
              :class="{ 'drag-active': dragTarget === FileInputTarget.Subtitle }"
              @dragenter.prevent="dragTarget = FileInputTarget.Subtitle"
              @dragover.prevent
              @dragleave.prevent="clearNativeDragTarget(FileInputTarget.Subtitle)"
              @drop.prevent="handleBrowserDrop(FileInputTarget.Subtitle, $event)"
            >
              <UploadCloud class="translate-drop-icon" :stroke-width="2.1" aria-hidden="true" />
              <div class="translate-drop-copy">
                <span class="translate-drop-title">选择或拖入字幕</span>
                <span class="translate-drop-subtitle">支持 SRT、VTT、ASS</span>
              </div>
              <button class="settings-action" type="button" :disabled="isBurning" @click="selectSubtitleFile">
                选择字幕
              </button>
            </div>

            <div class="translate-file-strip" aria-label="当前字幕">
              <FileText :stroke-width="2.1" aria-hidden="true" />
              <span>{{ selectedSubtitleName }}</span>
            </div>
          </div>
        </section>
      </div>

      <section class="settings-section subtitle-burn-output-section" aria-labelledby="burn-output-title">
        <div id="burn-output-title" class="section-heading">
          <Flame aria-hidden="true" />
          <span>烧录输出</span>
        </div>

        <div class="settings-panel subtitle-burn-output-panel">
          <div class="translate-status-bar">
            <div class="translate-status">
              <span class="translate-status-dot" :class="statusDotClass" aria-hidden="true" />
              <span>{{ statusText }}</span>
            </div>

            <div class="translate-actions subtitle-burn-actions">
              <button class="settings-action" type="button" :disabled="isBurning || !selectedVideoPath" @click="selectOutputFile">
                <Save :stroke-width="2.1" aria-hidden="true" />
                <span>选择输出</span>
              </button>
              <button
                class="settings-action subtitle-burn-primary-action"
                type="button"
                :disabled="!canStartBurn"
                @click="startBurn"
              >
                <Play :stroke-width="2.1" aria-hidden="true" />
                <span>{{ isBurning ? '烧录中' : '开始烧录' }}</span>
              </button>
              <button class="settings-action" type="button" :disabled="!canOpenOutput" @click="openOutput">
                <FolderOpen :stroke-width="2.1" aria-hidden="true" />
                <span>打开位置</span>
              </button>
            </div>
          </div>

          <div v-if="isBurning || burnProgress > 0" class="translate-progress" aria-label="处理进度">
            <div class="translate-progress-track">
              <span class="translate-progress-bar" :style="{ width: `${burnProgress}%` }" />
            </div>
            <span class="translate-progress-value">{{ burnProgress }}%</span>
          </div>

          <div v-if="burnError" class="translate-alert" role="alert">
            <CircleAlert :stroke-width="2.1" aria-hidden="true" />
            <span>{{ burnError }}</span>
          </div>

          <div class="subtitle-burn-summary" aria-label="烧录任务">
            <div class="subtitle-burn-summary-item">
              <FileVideo :stroke-width="2.1" aria-hidden="true" />
              <span class="subtitle-burn-summary-copy">
                <span class="subtitle-burn-summary-label">视频</span>
                <span class="subtitle-burn-summary-value">{{ selectedVideoName }}</span>
              </span>
            </div>
            <div class="subtitle-burn-summary-item">
              <Captions :stroke-width="2.1" aria-hidden="true" />
              <span class="subtitle-burn-summary-copy">
                <span class="subtitle-burn-summary-label">字幕</span>
                <span class="subtitle-burn-summary-value">{{ selectedSubtitleName }}</span>
              </span>
            </div>
            <div class="subtitle-burn-summary-item">
              <FolderOpen :stroke-width="2.1" aria-hidden="true" />
              <span class="subtitle-burn-summary-copy">
                <span class="subtitle-burn-summary-label">输出</span>
                <span class="subtitle-burn-summary-value">{{ outputPathLabel }}</span>
              </span>
            </div>
          </div>

          <div v-if="burnOutputPath && !burnError" class="subtitle-burn-result">
            <CheckCircle2 :stroke-width="2.1" aria-hidden="true" />
            <span>{{ burnOutputPath }}</span>
          </div>
          <div v-else class="translate-preview translate-preview-empty subtitle-burn-empty">
            <Flame class="translate-empty-icon" :stroke-width="2.1" aria-hidden="true" />
            <span class="translate-empty-title">等待烧录</span>
            <span class="translate-empty-subtitle">选择视频和字幕后，输出文件会显示在这里</span>
          </div>
        </div>
      </section>
    </main>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { DragDropEvent } from '@tauri-apps/api/webview'
import { open, save } from '@tauri-apps/plugin-dialog'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import {
  Captions,
  CheckCircle2,
  CircleAlert,
  FileText,
  FileVideo,
  Flame,
  FolderOpen,
  Play,
  Save,
  UploadCloud,
  Video,
} from 'lucide-vue-next'

defineOptions({ name: 'SubtitleBurn' })

enum FileInputTarget {
  Video = 'video',
  Subtitle = 'subtitle',
}

type SubtitleBurnProgress = {
  progress: number
  message: string
  outputPath?: string
}

type SubtitleBurnResult = {
  outputPath: string
  durationMs?: number
}

const videoDropZoneRef = ref<HTMLElement | null>(null)
const subtitleDropZoneRef = ref<HTMLElement | null>(null)
const dragTarget = ref<FileInputTarget | null>(null)
const selectedVideoPath = ref('')
const selectedSubtitlePath = ref('')
const selectedOutputPath = ref('')
const burnOutputPath = ref('')
const burnProgress = ref(0)
const burnMessage = ref('等待选择文件')
const burnError = ref('')
const isBurning = ref(false)
let unlistenBurnProgress: UnlistenFn | undefined
let unlistenDragDrop: UnlistenFn | undefined

const videoExtensions = ['mp4', 'mov', 'mkv', 'avi', 'flv', 'wmv', 'webm', 'm4v']
const subtitleExtensions = ['srt', 'vtt', 'ass']
const isTauriRuntime = () => '__TAURI_INTERNALS__' in window

const selectedVideoName = computed(() => {
  return selectedVideoPath.value ? fileNameFromPath(selectedVideoPath.value) : '尚未选择视频'
})

const selectedSubtitleName = computed(() => {
  return selectedSubtitlePath.value ? fileNameFromPath(selectedSubtitlePath.value) : '尚未选择字幕'
})

const outputPathLabel = computed(() => {
  if (burnOutputPath.value) {
    return fileNameFromPath(burnOutputPath.value)
  }

  if (selectedOutputPath.value) {
    return fileNameFromPath(selectedOutputPath.value)
  }

  return selectedVideoPath.value ? '自动保存到源视频目录' : '尚未生成输出'
})

const canStartBurn = computed(() => {
  return Boolean(selectedVideoPath.value && selectedSubtitlePath.value) && !isBurning.value
})

const canOpenOutput = computed(() => Boolean(burnOutputPath.value) && !burnError.value && !isBurning.value && isTauriRuntime())

const statusText = computed(() => {
  if (isBurning.value) {
    return burnMessage.value
  }

  if (burnError.value) {
    return '烧录失败'
  }

  if (burnOutputPath.value) {
    return '烧录完成'
  }

  if (selectedVideoPath.value && selectedSubtitlePath.value) {
    return '准备开始'
  }

  return '等待选择文件'
})

const statusDotClass = computed(() => ({
  active: isBurning.value,
  success: Boolean(burnOutputPath.value) && !burnError.value && !isBurning.value,
  warning: Boolean(selectedVideoPath.value && selectedSubtitlePath.value) && !burnOutputPath.value && !burnError.value && !isBurning.value,
  error: Boolean(burnError.value),
}))

const selectVideoFile = async () => {
  if (!isTauriRuntime()) {
    burnError.value = '请在桌面应用中选择视频文件'
    return
  }

  try {
    const selected = await open({
      title: '选择需要烧录的视频',
      multiple: false,
      filters: [
        {
          name: '视频文件',
          extensions: videoExtensions,
        },
      ],
    })

    if (typeof selected === 'string') {
      applyVideoFile(selected)
    }
  } catch (error) {
    burnError.value = stringifyError(error, '选择视频失败')
  }
}

const selectSubtitleFile = async () => {
  if (!isTauriRuntime()) {
    burnError.value = '请在桌面应用中选择字幕文件'
    return
  }

  try {
    const selected = await open({
      title: '选择需要烧录的字幕',
      multiple: false,
      filters: [
        {
          name: '字幕文件',
          extensions: subtitleExtensions,
        },
      ],
    })

    if (typeof selected === 'string') {
      applySubtitleFile(selected)
    }
  } catch (error) {
    burnError.value = stringifyError(error, '选择字幕失败')
  }
}

const selectOutputFile = async () => {
  if (!selectedVideoPath.value || !isTauriRuntime()) {
    return
  }

  try {
    const selected = await save({
      title: '保存烧录视频',
      defaultPath: buildSuggestedOutputPath(selectedVideoPath.value),
      filters: [
        {
          name: 'MP4 视频',
          extensions: ['mp4'],
        },
      ],
    })

    if (selected) {
      selectedOutputPath.value = ensureMp4Extension(selected)
      burnOutputPath.value = ''
      burnError.value = ''
      burnProgress.value = 0
    }
  } catch (error) {
    burnError.value = stringifyError(error, '选择输出位置失败')
  }
}

const applyVideoFile = (path: string) => {
  if (isBurning.value) {
    return
  }

  if (!videoExtensions.includes(fileExtension(path))) {
    burnError.value = '请选择支持的视频文件'
    return
  }

  selectedVideoPath.value = path
  selectedOutputPath.value = ''
  resetResultState()
}

const applySubtitleFile = (path: string) => {
  if (isBurning.value) {
    return
  }

  if (!subtitleExtensions.includes(fileExtension(path))) {
    burnError.value = '请选择 SRT、VTT 或 ASS 字幕文件'
    return
  }

  selectedSubtitlePath.value = path
  resetResultState()
}

const resetResultState = () => {
  burnOutputPath.value = ''
  burnProgress.value = 0
  burnMessage.value = selectedVideoPath.value && selectedSubtitlePath.value ? '准备开始' : '等待选择文件'
  burnError.value = ''
}

const startBurn = async () => {
  if (!canStartBurn.value) {
    return
  }

  if (!isTauriRuntime()) {
    burnError.value = '请在桌面应用中开始烧录'
    return
  }

  isBurning.value = true
  burnError.value = ''
  burnOutputPath.value = selectedOutputPath.value
  burnProgress.value = 0
  burnMessage.value = '准备烧录'

  try {
    const result = await invoke<SubtitleBurnResult>('start_subtitle_burn', {
      request: {
        videoPath: selectedVideoPath.value,
        subtitlePath: selectedSubtitlePath.value,
        outputPath: selectedOutputPath.value || null,
      },
    })
    burnOutputPath.value = result.outputPath
    burnProgress.value = 100
    burnMessage.value = '烧录完成'
  } catch (error) {
    burnError.value = stringifyError(error, '字幕烧录失败')
    burnMessage.value = '烧录失败'
  } finally {
    isBurning.value = false
  }
}

const openOutput = async () => {
  if (!burnOutputPath.value || !isTauriRuntime()) {
    return
  }

  try {
    await revealItemInDir(burnOutputPath.value)
  } catch (error) {
    burnError.value = stringifyError(error, '打开输出位置失败')
  }
}

const handleBrowserDrop = (target: FileInputTarget, event: DragEvent) => {
  dragTarget.value = null

  const file = Array.from(event.dataTransfer?.files ?? [])[0] as (File & { path?: string }) | undefined
  if (!file?.path) {
    return
  }

  applyDroppedPath(target, file.path)
}

const applyDroppedPath = (target: FileInputTarget, path: string) => {
  if (target === FileInputTarget.Video) {
    applyVideoFile(path)
  } else {
    applySubtitleFile(path)
  }
}

const clearNativeDragTarget = (target: FileInputTarget) => {
  if (dragTarget.value === target) {
    dragTarget.value = null
  }
}

const registerProgressListener = async () => {
  if (!isTauriRuntime()) {
    return
  }

  unlistenBurnProgress = await listen<SubtitleBurnProgress>('subtitle-burn-progress', (event) => {
    const payload = event.payload
    burnProgress.value = clampProgress(payload.progress)
    burnMessage.value = payload.message
    if (payload.outputPath) {
      burnOutputPath.value = payload.outputPath
    }
  })
}

const registerDragDropListener = async () => {
  if (!isTauriRuntime()) {
    return
  }

  const [{ getCurrentWebview }, { getCurrentWindow }] = await Promise.all([
    import('@tauri-apps/api/webview'),
    import('@tauri-apps/api/window'),
  ])
  const webview = getCurrentWebview()
  const currentWindow = getCurrentWindow()

  unlistenDragDrop = await webview.onDragDropEvent(async (event) => {
    const payload = event.payload

    if (payload.type === 'leave') {
      dragTarget.value = null
      return
    }

    if (payload.type === 'over') {
      dragTarget.value = await resolveDropTarget(payload, await currentWindow.scaleFactor())
      return
    }

    if (payload.type !== 'enter' && payload.type !== 'drop') {
      return
    }

    const target = await resolveDropTarget(payload, await currentWindow.scaleFactor())
    dragTarget.value = target

    if (payload.type !== 'drop') {
      return
    }

    dragTarget.value = null
    const path = payload.paths[0]
    if (target && path) {
      applyDroppedPath(target, path)
    }
  })
}

const resolveDropTarget = (payload: Extract<DragDropEvent, { type: 'enter' | 'over' | 'drop' }>, scaleFactor: number) => {
  const logicalPosition = payload.position.toLogical(scaleFactor)
  const point = {
    x: logicalPosition.x,
    y: logicalPosition.y,
  }

  if (isPointInsideElement(point, videoDropZoneRef.value)) {
    return FileInputTarget.Video
  }

  if (isPointInsideElement(point, subtitleDropZoneRef.value)) {
    return FileInputTarget.Subtitle
  }

  return null
}

const isPointInsideElement = (point: { x: number; y: number }, element: HTMLElement | null) => {
  if (!element) {
    return false
  }

  const rect = element.getBoundingClientRect()
  return point.x >= rect.left && point.x <= rect.right && point.y >= rect.top && point.y <= rect.bottom
}

const buildSuggestedOutputPath = (path: string) => {
  const withoutExtension = path.replace(/\.[^/.\\]+$/, '')
  return `${withoutExtension}_burned.mp4`
}

const ensureMp4Extension = (path: string) => {
  return path.toLowerCase().endsWith('.mp4') ? path : `${path}.mp4`
}

const fileNameFromPath = (path: string) => {
  const normalizedPath = path.replace(/\\/g, '/')
  return normalizedPath.split('/').filter(Boolean).pop() ?? path
}

const fileExtension = (path: string) => {
  const fileName = fileNameFromPath(path)
  const extension = fileName.split('.').pop()
  return extension ? extension.toLowerCase() : ''
}

const clampProgress = (value: number) => Math.min(Math.max(Math.round(value), 0), 100)

const stringifyError = (error: unknown, fallback: string) => {
  if (typeof error === 'string') {
    return error
  }

  if (error instanceof Error) {
    return error.message
  }

  return fallback
}

onMounted(() => {
  void registerProgressListener()
  void registerDragDropListener()
})

onBeforeUnmount(() => {
  unlistenBurnProgress?.()
  unlistenDragDrop?.()
})
</script>

<style scoped>
.subtitle-burn-workspace {
  display: flex;
  flex-direction: column;
  gap: 26px;
}

.subtitle-burn-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 26px;
  align-items: stretch;
}

.subtitle-burn-grid > .settings-section {
  min-width: 0;
}

.subtitle-burn-grid > .settings-section + .settings-section {
  margin-top: 0;
}

.subtitle-burn-drop-panel {
  height: 100%;
}

.subtitle-burn-drop-zone {
  min-height: 196px;
}

.subtitle-burn-output-section {
  margin-top: 0;
}

.subtitle-burn-output-panel {
  display: flex;
  flex-direction: column;
  padding: 18px;
}

.subtitle-burn-actions .settings-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
}

.subtitle-burn-actions .settings-action svg {
  width: 16px;
  height: 16px;
}

.subtitle-burn-primary-action:not(:disabled) {
  border-color: color-mix(in srgb, var(--accent) 46%, var(--hairline));
  background: var(--accent);
  color: #fff;
}

html[data-theme='dark'] .subtitle-burn-primary-action:not(:disabled) {
  color: #18140f;
}

.subtitle-burn-primary-action:not(:disabled):hover {
  border-color: var(--accent-strong);
  background: var(--accent-strong);
}

.subtitle-burn-summary {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin-top: 16px;
  overflow: hidden;
  border: 1px solid var(--hairline);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.2);
}

html[data-theme='dark'] .subtitle-burn-summary {
  background: rgba(0, 0, 0, 0.12);
}

.subtitle-burn-summary-item {
  min-width: 0;
  min-height: 76px;
  display: grid;
  grid-template-columns: 24px minmax(0, 1fr);
  align-items: center;
  gap: 11px;
  padding: 14px 16px;
}

.subtitle-burn-summary-item + .subtitle-burn-summary-item {
  border-left: 1px solid var(--hairline);
}

.subtitle-burn-summary-item > svg {
  width: 21px;
  height: 21px;
  color: var(--accent);
}

.subtitle-burn-summary-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.subtitle-burn-summary-label {
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 800;
  line-height: 1;
}

.subtitle-burn-summary-value {
  overflow: hidden;
  color: var(--text);
  font-size: 14px;
  font-weight: 800;
  line-height: 1.25;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.subtitle-burn-result {
  min-height: 48px;
  margin-top: 16px;
  border: 1px solid color-mix(in srgb, #2f8a55 32%, var(--hairline));
  border-radius: 14px;
  background: rgba(47, 138, 85, 0.08);
  color: var(--text);
  display: grid;
  grid-template-columns: 22px minmax(0, 1fr);
  align-items: center;
  gap: 10px;
  padding: 12px 14px;
  font-size: 13px;
  font-weight: 750;
}

html[data-theme='dark'] .subtitle-burn-result {
  background: rgba(47, 138, 85, 0.14);
}

.subtitle-burn-result svg {
  color: #2f8a55;
}

html[data-theme='dark'] .subtitle-burn-result svg {
  color: #74c995;
}

.subtitle-burn-result span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.subtitle-burn-empty {
  min-height: 180px;
}

@media (max-width: 980px) {
  .subtitle-burn-grid,
  .subtitle-burn-summary {
    grid-template-columns: 1fr;
  }

  .subtitle-burn-summary-item + .subtitle-burn-summary-item {
    border-left: 0;
    border-top: 1px solid var(--hairline);
  }
}

@media (max-width: 860px) {
  .subtitle-burn-actions {
    align-items: stretch;
    flex-direction: column;
  }

  .subtitle-burn-actions .settings-action {
    width: 100%;
  }

  .subtitle-burn-drop-zone {
    min-height: 164px;
  }
}
</style>
