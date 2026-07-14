<template>
  <div class="page translate-page subtitle-review-page">
    <header class="translate-header subtitle-review-header">
      <div class="subtitle-review-title-group">
        <h1 class="page-title">字幕审核</h1>
        <span v-if="sessionId" class="subtitle-review-session-state" :class="{ dirty: hasUnexportedChanges }">
          {{ hasUnexportedChanges ? '有未导出修改' : '本次会话' }}
        </span>
      </div>

      <div v-if="sessionId" class="translate-actions subtitle-review-header-actions">
        <button class="settings-action icon-only" type="button" :disabled="!canUndo || isExporting" aria-label="撤销" @click="undo">
          <Undo2 :stroke-width="2.1" aria-hidden="true" />
        </button>
        <button class="settings-action icon-only" type="button" :disabled="!canRedo || isExporting" aria-label="重做" @click="redo">
          <Redo2 :stroke-width="2.1" aria-hidden="true" />
        </button>
        <button class="settings-action" type="button" :disabled="isExporting" @click="requestResetWorkspace">
          <RefreshCw :stroke-width="2.1" aria-hidden="true" />
          <span>更换文件</span>
        </button>
        <button
          class="settings-action subtitle-review-primary-action"
          type="button"
          :disabled="!canExport"
          @click="exportReviewedVideo"
        >
          <Save :stroke-width="2.1" aria-hidden="true" />
          <span>{{ isExporting ? '导出中' : '导出视频' }}</span>
        </button>
      </div>
    </header>

    <main class="translate-workspace subtitle-review-workspace">
      <div v-if="reviewError" class="translate-alert" role="alert">
        <CircleAlert :stroke-width="2.1" aria-hidden="true" />
        <span>{{ reviewError }}</span>
        <button class="subtitle-review-alert-close" type="button" aria-label="关闭错误提示" @click="reviewError = ''">
          <X :stroke-width="2.1" aria-hidden="true" />
        </button>
      </div>

      <template v-if="!sessionId">
        <div class="subtitle-review-import-grid">
          <section class="settings-section" aria-labelledby="review-video-input-title">
            <div id="review-video-input-title" class="section-heading">
              <Video aria-hidden="true" />
              <span>视频输入</span>
            </div>
            <div class="settings-panel translate-drop-panel subtitle-review-drop-panel">
              <div
                class="translate-drop-zone subtitle-review-drop-zone"
                :class="{ 'drag-active': nativeDragKind === FileKind.Video }"
                @dragenter.prevent="nativeDragKind = FileKind.Video"
                @dragover.prevent
                @dragleave.prevent="nativeDragKind = null"
                @drop.prevent="handleBrowserDrop($event)"
              >
                <UploadCloud class="translate-drop-icon" :stroke-width="2.1" aria-hidden="true" />
                <div class="translate-drop-copy">
                  <span class="translate-drop-title">选择或拖入视频</span>
                  <span class="translate-drop-subtitle">支持 MP4、MOV、MKV、AVI、FLV、WMV、WEBM、M4V</span>
                </div>
                <button class="settings-action" type="button" :disabled="isPreparing" @click="selectVideoFile">
                  选择视频
                </button>
              </div>
              <div class="translate-file-strip" aria-label="当前视频">
                <FileVideo :stroke-width="2.1" aria-hidden="true" />
                <span>{{ selectedVideoName }}</span>
              </div>
            </div>
          </section>

          <section class="settings-section" aria-labelledby="review-subtitle-input-title">
            <div id="review-subtitle-input-title" class="section-heading">
              <Captions aria-hidden="true" />
              <span>字幕输入</span>
            </div>
            <div class="settings-panel translate-drop-panel subtitle-review-drop-panel">
              <div
                class="translate-drop-zone subtitle-review-drop-zone"
                :class="{ 'drag-active': nativeDragKind === FileKind.Subtitle }"
                @dragenter.prevent="nativeDragKind = FileKind.Subtitle"
                @dragover.prevent
                @dragleave.prevent="nativeDragKind = null"
                @drop.prevent="handleBrowserDrop($event)"
              >
                <UploadCloud class="translate-drop-icon" :stroke-width="2.1" aria-hidden="true" />
                <div class="translate-drop-copy">
                  <span class="translate-drop-title">选择或拖入字幕</span>
                  <span class="translate-drop-subtitle">支持 SRT、VTT、ASS，ASS 保留文件内样式</span>
                </div>
                <button class="settings-action" type="button" :disabled="isPreparing" @click="selectSubtitleFile">
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

        <section class="settings-section subtitle-review-ready-section" aria-labelledby="review-ready-title">
          <div id="review-ready-title" class="section-heading">
            <BadgeCheck aria-hidden="true" />
            <span>审核工作台</span>
          </div>
          <div class="settings-panel subtitle-review-ready-panel">
            <div class="translate-status">
              <span class="translate-status-dot" :class="{ active: isPreparing, warning: filesReady && !isPreparing }" aria-hidden="true" />
              <span>{{ preparationStatus }}</span>
            </div>
            <button
              class="settings-action subtitle-review-primary-action"
              type="button"
              :disabled="!filesReady || isPreparing"
              @click="prepareWorkspace"
            >
              <LoaderCircle v-if="isPreparing" class="spin" :stroke-width="2.1" aria-hidden="true" />
              <BadgeCheck v-else :stroke-width="2.1" aria-hidden="true" />
              <span>{{ isPreparing ? '正在准备' : '开始审核' }}</span>
            </button>
          </div>
        </section>
      </template>

      <template v-else>
        <section class="settings-section subtitle-review-editor-section" aria-labelledby="review-editor-title">
          <div id="review-editor-title" class="section-heading subtitle-review-editor-heading">
            <BadgeCheck aria-hidden="true" />
            <span>审核工作台</span>
            <span class="subtitle-review-heading-meta">{{ subtitleFormatLabel }} · {{ styleName }} · {{ cueCountLabel }}</span>
          </div>

          <div class="settings-panel subtitle-review-editor-panel">
            <div class="subtitle-review-filebar">
              <span class="subtitle-review-filebar-item">
                <FileVideo :stroke-width="2.1" aria-hidden="true" />
                <b>{{ videoMetadata?.fileName }}</b>
                <small>{{ videoDetail }}</small>
              </span>
              <span class="subtitle-review-filebar-item">
                <FileText :stroke-width="2.1" aria-hidden="true" />
                <b>{{ selectedSubtitleName }}</b>
                <small>{{ styleName }}</small>
              </span>
              <span class="subtitle-review-filebar-spacer" />
              <span v-if="validationSummary" class="subtitle-review-validation-summary" :class="validationSummaryClass">
                <CircleAlert v-if="validationErrorCount > 0" :stroke-width="2.1" aria-hidden="true" />
                <CheckCircle2 v-else :stroke-width="2.1" aria-hidden="true" />
                {{ validationSummary }}
              </span>
            </div>

            <div v-if="warnings.length > 0" class="subtitle-review-warning-strip" role="status">
              <CircleAlert :stroke-width="2.1" aria-hidden="true" />
              <span>{{ warnings.join('；') }}</span>
            </div>

            <div v-if="isProxyPreparing || proxyError" class="subtitle-review-proxy-strip" :class="{ error: proxyError }">
              <span class="translate-status-dot" :class="{ active: isProxyPreparing, error: proxyError }" aria-hidden="true" />
              <span>{{ proxyError || proxyMessage }}</span>
              <div v-if="isProxyPreparing" class="subtitle-review-inline-progress">
                <span :style="{ width: `${proxyProgress}%` }" />
              </div>
              <button v-if="isProxyPreparing" class="settings-action compact" type="button" @click="cancelProxy">
                取消
              </button>
            </div>

            <div class="subtitle-review-main-grid">
              <div class="subtitle-review-video-column">
                <div ref="videoFrameRef" class="subtitle-review-video-frame">
                  <div ref="videoStageRef" class="subtitle-review-video-stage" :style="videoStageStyle" @click="toggleVideoPlayback">
                    <video
                      ref="videoRef"
                      class="subtitle-review-video"
                      :src="previewVideoUrl"
                      preload="metadata"
                      @loadedmetadata="handleVideoLoaded"
                      @loadeddata="drawCurrentVideoFrame"
                      @play="startFrameUpdates"
                      @pause="stopFrameUpdates"
                      @seeked="updateCurrentTime"
                      @timeupdate="updateCurrentTime"
                      @volumechange="syncVideoVolumeState"
                      @error="handleVideoError"
                    />
                    <canvas ref="videoCanvasRef" class="subtitle-review-frame-canvas" aria-hidden="true" />
                    <div
                      ref="subtitleLayerRef"
                      class="subtitle-review-subtitle-layer"
                      :class="{ ready: isRendererReady, obscured: activePreviewCues.length > 0 }"
                      aria-hidden="true"
                    />
                    <div
                      v-if="activePreviewCues.length > 0"
                      class="subtitle-review-caption-fallback"
                      aria-hidden="true"
                    >
                      <span
                        v-for="cue in activePreviewCues"
                        :key="cue.id"
                        :style="captionFallbackStyle(cue)"
                      >{{ captionFallbackText(cue) }}</span>
                    </div>
                  </div>
                  <div v-if="isRendererLoading" class="subtitle-review-video-loading">
                    <LoaderCircle class="spin" :stroke-width="2.1" aria-hidden="true" />
                    <span>正在加载字幕渲染器</span>
                  </div>
                  <div v-if="videoLoadMessage" class="subtitle-review-video-notice" role="status">
                    {{ videoLoadMessage }}
                  </div>
                </div>

                <div class="subtitle-review-transport">
                  <button
                    class="settings-action compact icon-only"
                    type="button"
                    :aria-label="isVideoPlaying ? '暂停视频' : '播放视频'"
                    @click="toggleVideoPlayback"
                  >
                    <Pause v-if="isVideoPlaying" :stroke-width="2.1" aria-hidden="true" />
                    <Play v-else :stroke-width="2.1" aria-hidden="true" />
                  </button>
                  <button
                    class="settings-action compact icon-only"
                    type="button"
                    :aria-label="isVideoMuted ? '恢复声音' : '静音'"
                    @click="toggleVideoMute"
                  >
                    <VolumeX v-if="isVideoMuted" :stroke-width="2.1" aria-hidden="true" />
                    <Volume2 v-else :stroke-width="2.1" aria-hidden="true" />
                  </button>
                  <span class="subtitle-review-time-readout">
                    {{ formatReviewTime(currentTimeMs) }}
                    <i>/</i>
                    {{ formatReviewTime(videoDurationMs) }}
                  </span>
                  <span class="subtitle-review-transport-spacer" />
                  <button class="settings-action compact" type="button" :disabled="isExporting" @click="setSelectedBoundary('start')">
                    <CornerDownRight :stroke-width="2.1" aria-hidden="true" />
                    <span>设为开始</span>
                  </button>
                  <button class="settings-action compact" type="button" :disabled="isExporting" @click="setSelectedBoundary('end')">
                    <CornerDownLeft :stroke-width="2.1" aria-hidden="true" />
                    <span>设为结束</span>
                  </button>
                  <button class="settings-action compact" type="button" :disabled="isExporting" @click="addCueAtPlayhead">
                    <Plus :stroke-width="2.1" aria-hidden="true" />
                    <span>新增字幕</span>
                  </button>
                </div>

                <SubtitleTimeline
                  class="subtitle-review-timeline compact"
                  :cues="timelineCues"
                  :duration-ms="videoDurationMs"
                  :current-time-ms="currentTimeMs"
                  :selected-cue-id="selectedCueId"
                  :disabled="isExporting"
                  @select="selectCue($event, false)"
                  @seek="seekVideo"
                  @preview-change="previewTimelineChange"
                  @commit-change="commitTimelineChange"
                />
              </div>

              <aside class="subtitle-review-cue-pane" aria-label="字幕编辑列表">
                <div class="subtitle-review-cue-toolbar">
                  <span>
                    <b>字幕条目</b>
                    <small>{{ activeCueLabel }}</small>
                  </span>
                  <button class="settings-action compact" type="button" :disabled="isExporting" @click="addCueAtPlayhead">
                    <Plus :stroke-width="2.1" aria-hidden="true" />
                    <span>新增</span>
                  </button>
                </div>

                <div ref="cueListRef" class="subtitle-review-cue-list">
                  <article
                    v-for="(group, index) in sortedCueGroups"
                    :key="group.id"
                    class="subtitle-review-cue-row"
                    :class="{
                      selected: group.id === selectedCueId,
                      active: activeCueIds.has(group.id),
                      invalid: groupHasError(group),
                      bilingual: group.isBilingual,
                    }"
                    :data-cue-id="group.id"
                    @click="selectCue(group.id, false)"
                  >
                    <div class="subtitle-review-cue-row-head">
                      <button class="subtitle-review-cue-index" type="button" @click.stop="selectCue(group.id, true)">
                        {{ index + 1 }}
                      </button>
                      <span v-if="group.isBilingual" class="subtitle-review-bilingual-tag">双语</span>
                      <span v-if="subtitleFormat === 'ass'" class="subtitle-review-cue-style">
                        {{ groupStyleLabel(group) }}
                      </span>
                      <span class="subtitle-review-cue-head-spacer" />
                      <button
                        class="subtitle-review-cue-delete"
                        type="button"
                        :disabled="isExporting"
                        :aria-label="`删除第 ${index + 1} 组字幕`"
                        @click.stop="deleteCueGroup(group.id)"
                      >
                        <Trash2 :stroke-width="2.1" aria-hidden="true" />
                      </button>
                    </div>

                    <div class="subtitle-review-time-fields">
                      <label>
                        <span>开始</span>
                        <input
                          :value="formatReviewTime(group.startTime)"
                          :disabled="isExporting"
                          spellcheck="false"
                          @focus="beginCueEdit"
                          @blur="commitCueEdit"
                          @change="changeCueGroupTime(group.id, 'start', $event)"
                          @keydown.enter.prevent="blurEventTarget"
                        />
                      </label>
                      <label>
                        <span>结束</span>
                        <input
                          :value="formatReviewTime(group.endTime)"
                          :disabled="isExporting"
                          spellcheck="false"
                          @focus="beginCueEdit"
                          @blur="commitCueEdit"
                          @change="changeCueGroupTime(group.id, 'end', $event)"
                          @keydown.enter.prevent="blurEventTarget"
                        />
                      </label>
                    </div>

                    <div
                      v-for="(cue, languageIndex) in group.cues"
                      :key="cue.id"
                      class="subtitle-review-language-block"
                    >
                      <div class="subtitle-review-language-head">
                        <span class="subtitle-review-language-role">{{ cueRoleLabel(cue, languageIndex, group) }}</span>
                        <span v-if="cue.hasInlineTags" class="subtitle-review-cue-tag">
                          <Braces :stroke-width="2.1" aria-hidden="true" />
                          ASS 特效
                        </span>
                        <span class="subtitle-review-language-spacer" />
                        <div
                          v-if="subtitleFormat === 'ass' && cue.hasInlineTags"
                          class="subtitle-review-text-mode"
                          role="group"
                          :aria-label="`${cueRoleLabel(cue, languageIndex, group)} ASS 编辑模式`"
                        >
                          <button
                            type="button"
                            :class="{ active: cue.textMode === 'plain' }"
                            :disabled="isExporting"
                            @click.stop="requestPlainMode(cue.id)"
                          >
                            普通文字
                          </button>
                          <button
                            type="button"
                            :class="{ active: cue.textMode === 'raw' }"
                            :disabled="isExporting"
                            @click.stop="switchCueToRaw(cue.id)"
                          >
                            ASS 原文
                          </button>
                        </div>
                      </div>

                      <textarea
                        class="subtitle-review-cue-text"
                        :class="{ raw: cue.textMode === 'raw' }"
                        :value="cue.textMode === 'raw' ? cue.rawText : cue.text"
                        :disabled="isExporting"
                        :aria-label="`第 ${index + 1} 组${cueRoleLabel(cue, languageIndex, group)}文字`"
                        spellcheck="false"
                        @focus="beginCueEdit"
                        @input="changeCueText(cue.id, $event)"
                        @blur="commitCueEdit"
                      />

                      <div v-if="cueIssues(cue.id).length > 0" class="subtitle-review-cue-issues">
                        <span v-for="issue in cueIssues(cue.id)" :key="issue.code" :class="issue.level">
                          {{ issue.message }}
                        </span>
                      </div>
                    </div>
                  </article>
                </div>
              </aside>
            </div>

            <div v-if="isExporting || exportProgress > 0 || exportOutputPath" class="subtitle-review-export-bar">
              <div class="translate-status">
                <span
                  class="translate-status-dot"
                  :class="{ active: isExporting, success: Boolean(exportOutputPath) && !exportError, error: Boolean(exportError) }"
                  aria-hidden="true"
                />
                <span>{{ exportError || exportMessage }}</span>
              </div>
              <div v-if="isExporting || exportProgress > 0" class="translate-progress subtitle-review-export-progress">
                <div class="translate-progress-track">
                  <span class="translate-progress-bar" :style="{ width: `${exportProgress}%` }" />
                </div>
                <span class="translate-progress-value">{{ exportProgress }}%</span>
              </div>
              <span class="subtitle-review-export-spacer" />
              <button v-if="isExporting" class="settings-action compact" type="button" @click="cancelExport">取消导出</button>
              <button v-else class="settings-action compact" type="button" :disabled="!exportOutputPath" @click="openExportOutput">
                <FolderOpen :stroke-width="2.1" aria-hidden="true" />
                <span>打开位置</span>
              </button>
            </div>
          </div>
        </section>
      </template>
    </main>

    <div v-if="showPlainModeDialog" class="dialog-backdrop" @click.self="closePlainModeDialog">
      <section
        class="settings-dialog subtitle-review-confirm-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="review-plain-mode-title"
        @click.stop
      >
        <h2 id="review-plain-mode-title" class="dialog-title">切换为普通文字</h2>
        <p>这条字幕包含 ASS 行内特效。切换后会移除该条的定位、卡拉 OK 等行内标签，但保留事件级字幕样式。</p>
        <div class="subtitle-review-dialog-actions">
          <button class="settings-action" type="button" @click="closePlainModeDialog">取消</button>
          <button class="settings-action subtitle-review-primary-action" type="button" @click="confirmPlainMode">继续切换</button>
        </div>
      </section>
    </div>

    <div v-if="showResetDialog" class="dialog-backdrop" @click.self="showResetDialog = false">
      <section
        class="settings-dialog subtitle-review-confirm-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="review-reset-title"
        @click.stop
      >
        <h2 id="review-reset-title" class="dialog-title">更换审核文件</h2>
        <p>当前修改仅保存在本次会话中。更换文件后，这些未导出的修改将无法恢复。</p>
        <div class="subtitle-review-dialog-actions">
          <button class="settings-action" type="button" @click="showResetDialog = false">继续审核</button>
          <button class="settings-action subtitle-review-danger-action" type="button" @click="resetWorkspace">放弃并更换</button>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import JASSUB from 'jassub'
import workerUrl from 'jassub/dist/worker/worker.js?worker&url'
import wasmUrl from 'jassub/dist/wasm/jassub-worker.wasm?url'
import modernWasmUrl from 'jassub/dist/wasm/jassub-worker-modern.wasm?url'
import {
  computed,
  nextTick,
  onActivated,
  onBeforeUnmount,
  onDeactivated,
  onMounted,
  ref,
  shallowRef,
  type CSSProperties,
  watch,
} from 'vue'
import {
  BadgeCheck,
  Braces,
  Captions,
  CheckCircle2,
  CircleAlert,
  CornerDownLeft,
  CornerDownRight,
  FileText,
  FileVideo,
  FolderOpen,
  LoaderCircle,
  Plus,
  Play,
  Pause,
  Redo2,
  RefreshCw,
  Save,
  Trash2,
  Undo2,
  UploadCloud,
  Video,
  Volume2,
  VolumeX,
  X,
} from 'lucide-vue-next'
import SubtitleTimeline from '../components/SubtitleTimeline.vue'
import {
  DEFAULT_CUE_DURATION_MS,
  MIN_CUE_DURATION_MS,
  clampNumber,
  cloneReviewCues,
  findActiveCueIds,
  findPlaybackCueId,
  formatReviewTime,
  parseReviewTime,
  reviewCuesEqual,
  sortReviewCues,
  type PrepareSubtitleReviewResult,
  type ReviewCue,
  type ReviewSubtitleFormat,
  type ReviewValidation,
  type ReviewValidationIssue,
  type ReviewVideoMetadata,
  type UpdateSubtitleReviewResult,
} from '../subtitleReviewModel'

defineOptions({ name: 'SubtitleReview' })

enum FileKind {
  Video = 'video',
  Subtitle = 'subtitle',
}

type ProxyProgress = {
  sessionId: string
  progress: number
  message: string
  status: 'running' | 'done' | 'failed' | 'cancelled'
  previewPath?: string
}

type ProxyResult = {
  sessionId: string
  previewPath: string
}

type ExportProgress = {
  sessionId: string
  jobId: string
  progress: number
  message: string
  status: 'running' | 'done' | 'failed' | 'cancelled'
  outputPath?: string
}

type ExportResult = {
  sessionId: string
  jobId: string
  outputPath: string
  durationMs: number
}

type ReviewCueGroup = {
  id: string
  cues: ReviewCue[]
  startTime: number
  endTime: number
  isBilingual: boolean
}

type AssPreviewStyle = {
  fontFamily: string
  fontSize: number
  primaryColor: string
  outlineColor: string
  backgroundColor: string
  bold: boolean
  italic: boolean
  underline: boolean
  strikeOut: boolean
  scaleX: number
  scaleY: number
  spacing: number
  angle: number
  borderStyle: number
  outline: number
  shadow: number
  alignment: number
  marginL: number
  marginR: number
  marginV: number
}

type AssPreviewConfig = {
  playResX: number
  playResY: number
  styles: Record<string, AssPreviewStyle>
}

type AssInlineOverrides = {
  alignment?: number
  position?: { x: number; y: number }
  fontFamily?: string
  fontSize?: number
  primaryColor?: string
  outlineColor?: string
  bold?: boolean
  italic?: boolean
  underline?: boolean
  outline?: number
  shadow?: number
}

const videoExtensions = ['mp4', 'mov', 'mkv', 'avi', 'flv', 'wmv', 'webm', 'm4v'] as const
const subtitleExtensions = ['srt', 'vtt', 'ass'] as const
const VIDEO_STREAM_CHUNK_BYTES = 2 * 1024 * 1024
const MAX_DIRECT_PREVIEW_BYTES = 256 * 1024 * 1024
const videoRef = ref<HTMLVideoElement | null>(null)
const videoFrameRef = ref<HTMLElement | null>(null)
const videoStageRef = ref<HTMLElement | null>(null)
const videoCanvasRef = ref<HTMLCanvasElement | null>(null)
const subtitleLayerRef = ref<HTMLElement | null>(null)
const cueListRef = ref<HTMLElement | null>(null)
const videoFrameSize = ref({ width: 0, height: 0 })
const naturalVideoSize = ref({ width: 0, height: 0 })
const selectedVideoPath = ref('')
const selectedSubtitlePath = ref('')
const sessionId = ref('')
const videoMetadata = ref<ReviewVideoMetadata | null>(null)
const previewPath = ref('')
const previewVideoUrl = ref('')
const subtitleFormat = ref<ReviewSubtitleFormat>('srt')
const styleName = ref('')
const cues = ref<ReviewCue[]>([])
const assPreviewConfig = ref<AssPreviewConfig>(createDefaultAssPreviewConfig())
const validation = ref<ReviewValidation>({ canExport: false, issues: [] })
const warnings = ref<string[]>([])
const selectedCueId = ref('')
const currentTimeMs = ref(0)
const videoDurationMs = ref(0)
const revision = ref(0)
const lastExportedRevision = ref(0)
const undoStack = ref<ReviewCue[][]>([])
const redoStack = ref<ReviewCue[][]>([])
const reviewError = ref('')
const isPreparing = ref(false)
const nativeDragKind = ref<FileKind | null>(null)
const isRendererLoading = ref(false)
const isRendererReady = ref(false)
const isVideoPlaying = ref(false)
const isVideoMuted = ref(false)
const videoLoadMessage = ref('')
const isProxyPreparing = ref(false)
const proxyProgress = ref(0)
const proxyMessage = ref('正在准备兼容预览')
const proxyError = ref('')
const proxyAttempted = ref(false)
const isExporting = ref(false)
const exportJobId = ref('')
const exportProgress = ref(0)
const exportMessage = ref('等待导出')
const exportError = ref('')
const exportOutputPath = ref('')
const showPlainModeDialog = ref(false)
const pendingPlainCueId = ref('')
const showResetDialog = ref(false)
const renderer = shallowRef<JASSUB | null>(null)
const isCueEditing = ref(false)

let updateTimer: ReturnType<typeof window.setTimeout> | undefined
let fieldEditSnapshot: ReviewCue[] | null = null
let unlistenProxyProgress: UnlistenFn | undefined
let unlistenExportProgress: UnlistenFn | undefined
let unlistenDragDrop: UnlistenFn | undefined
let videoFrameCallbackId: number | undefined
let videoLoadTimer: ReturnType<typeof window.setTimeout> | undefined
let renderQueue: Promise<void> = Promise.resolve()
let isViewActive = true
let videoFrameResizeObserver: ResizeObserver | null = null
let videoCanvasDrawFailed = false
let videoSourceGeneration = 0
let videoSourceAbortController: AbortController | null = null
let previewObjectUrl = ''
let isReplacingVideoSource = false
let subtitleCanvasElement: HTMLCanvasElement | null = null

const isTauriRuntime = () => '__TAURI_INTERNALS__' in window
const filesReady = computed(() => Boolean(selectedVideoPath.value && selectedSubtitlePath.value))
const selectedVideoName = computed(() => fileNameFromPath(selectedVideoPath.value) || '尚未选择视频')
const selectedSubtitleName = computed(() => fileNameFromPath(selectedSubtitlePath.value) || '尚未选择字幕')
const sortedCueGroups = computed(() => buildCueGroups(cues.value))
const timelineCues = computed(() => sortedCueGroups.value.map(groupToTimelineCue))
const activeRawCueIds = computed(() => new Set(findActiveCueIds(cues.value, currentTimeMs.value)))
const activeCueIds = computed(
  () =>
    new Set(
      sortedCueGroups.value
        .filter((group) => group.cues.some((cue) => activeRawCueIds.value.has(cue.id)))
        .map((group) => group.id),
    ),
)
const activePreviewCues = computed(() =>
  sortReviewCues(cues.value).filter(
    (cue) => cue.startTime <= currentTimeMs.value && currentTimeMs.value < cue.endTime,
  ),
)
const selectedCueGroup = computed(
  () => sortedCueGroups.value.find((group) => group.id === selectedCueId.value) ?? null,
)
const selectedCue = computed(() => selectedCueGroup.value?.cues[0] ?? null)
const cueCountLabel = computed(() => {
  if (subtitleFormat.value === 'ass' && sortedCueGroups.value.some((group) => group.isBilingual)) {
    return `${sortedCueGroups.value.length} 组 · ${cues.value.length} 条事件`
  }
  return `${sortedCueGroups.value.length} 条`
})
const canUndo = computed(() => undoStack.value.length > 0)
const canRedo = computed(() => redoStack.value.length > 0)
const hasUnexportedChanges = computed(() => revision.value > lastExportedRevision.value)
const validationErrorCount = computed(() => validation.value.issues.filter((issue) => issue.level === 'error').length)
const validationWarningCount = computed(() => validation.value.issues.filter((issue) => issue.level === 'warning').length)
const validationSummary = computed(() => {
  if (validationErrorCount.value > 0) return `${validationErrorCount.value} 项错误`
  if (validationWarningCount.value > 0) return `${validationWarningCount.value} 项提醒`
  return '字幕检查通过'
})
const validationSummaryClass = computed(() => ({
  error: validationErrorCount.value > 0,
  warning: validationErrorCount.value === 0 && validationWarningCount.value > 0,
  success: validationErrorCount.value === 0 && validationWarningCount.value === 0,
}))
const preparationStatus = computed(() => {
  if (isPreparing.value) return '正在读取视频信息和字幕样式'
  if (filesReady.value) return '文件已齐全，可以进入审核工作台'
  return '选择一个视频和一份字幕后开始人工调整'
})
const subtitleFormatLabel = computed(() => subtitleFormat.value.toUpperCase())
const activeCueLabel = computed(() => {
  if (activeCueIds.value.size > 0) return `当前播放 ${activeCueIds.value.size} 组`
  if (selectedCueGroup.value) {
    return `已选第 ${sortedCueGroups.value.findIndex((group) => group.id === selectedCueId.value) + 1} 组`
  }
  return '选择字幕后编辑'
})
const videoDetail = computed(() => {
  if (!videoMetadata.value) return ''
  const codec = videoMetadata.value.videoCodec ? videoMetadata.value.videoCodec.toUpperCase() : '视频'
  return `${videoMetadata.value.width}×${videoMetadata.value.height} · ${codec} · ${formatReviewTime(videoDurationMs.value)}`
})
const videoStageSize = computed(() => {
  const frameWidth = videoFrameSize.value.width
  const frameHeight = videoFrameSize.value.height
  const sourceWidth = naturalVideoSize.value.width || videoMetadata.value?.width || 0
  const sourceHeight = naturalVideoSize.value.height || videoMetadata.value?.height || 0
  if (frameWidth <= 0 || frameHeight <= 0) {
    return { width: 0, height: 0 }
  }
  if (sourceWidth <= 0 || sourceHeight <= 0) {
    return { width: frameWidth, height: frameHeight }
  }
  const videoRatio = sourceWidth / sourceHeight
  const frameRatio = frameWidth / frameHeight
  const width = frameRatio > videoRatio ? frameHeight * videoRatio : frameWidth
  const height = frameRatio > videoRatio ? frameHeight : frameWidth / videoRatio
  return {
    width: Math.max(1, Math.round(width)),
    height: Math.max(1, Math.round(height)),
  }
})
const videoStageStyle = computed(() => {
  const { width, height } = videoStageSize.value
  if (width <= 0 || height <= 0) {
    return { width: '100%', height: '100%' }
  }
  return {
    width: `${width}px`,
    height: `${height}px`,
  }
})
const canExport = computed(
  () => Boolean(sessionId.value && cues.value.length > 0 && validation.value.canExport && !isPreparing.value && !isExporting.value),
)

const selectVideoFile = async () => {
  if (!isTauriRuntime()) {
    reviewError.value = '请在桌面应用中选择视频文件'
    return
  }
  try {
    const selected = await open({
      title: '选择需要审核的视频',
      multiple: false,
      filters: [{ name: '视频文件', extensions: [...videoExtensions] }],
    })
    if (typeof selected === 'string') applyInputFile(selected)
  } catch (error) {
    reviewError.value = stringifyError(error, '选择视频失败')
  }
}

const selectSubtitleFile = async () => {
  if (!isTauriRuntime()) {
    reviewError.value = '请在桌面应用中选择字幕文件'
    return
  }
  try {
    const selected = await open({
      title: '选择需要审核的字幕',
      multiple: false,
      filters: [{ name: '字幕文件', extensions: [...subtitleExtensions] }],
    })
    if (typeof selected === 'string') applyInputFile(selected)
  } catch (error) {
    reviewError.value = stringifyError(error, '选择字幕失败')
  }
}

const applyInputFile = (path: string) => {
  const extension = fileExtension(path)
  if (videoExtensions.includes(extension as (typeof videoExtensions)[number])) {
    selectedVideoPath.value = path
    reviewError.value = ''
    return
  }
  if (subtitleExtensions.includes(extension as (typeof subtitleExtensions)[number])) {
    selectedSubtitlePath.value = path
    reviewError.value = ''
    return
  }
  reviewError.value = '请选择支持的视频或 SRT、VTT、ASS 字幕文件'
}

const handleBrowserDrop = (event: DragEvent) => {
  nativeDragKind.value = null
  const file = Array.from(event.dataTransfer?.files ?? [])[0] as (File & { path?: string }) | undefined
  if (file?.path) applyInputFile(file.path)
}

const releasePlaybackVideoSource = () => {
  videoSourceGeneration += 1
  isReplacingVideoSource = true
  videoSourceAbortController?.abort()
  videoSourceAbortController = null
  previewVideoUrl.value = ''
  if (previewObjectUrl) {
    URL.revokeObjectURL(previewObjectUrl)
    previewObjectUrl = ''
  }
}

const loadPlaybackVideoSource = async (): Promise<boolean> => {
  if (!previewPath.value || !sessionId.value) return false
  const generation = ++videoSourceGeneration
  isReplacingVideoSource = true
  videoSourceAbortController?.abort()
  const abortController = new AbortController()
  videoSourceAbortController = abortController
  if (previewObjectUrl) {
    URL.revokeObjectURL(previewObjectUrl)
    previewObjectUrl = ''
  }
  previewVideoUrl.value = ''
  videoLoadMessage.value = '正在载入视频预览 0%'

  try {
    if (!isTauriRuntime()) {
      previewVideoUrl.value = previewPath.value
      isReplacingVideoSource = false
      return true
    }

    const streamUrl = convertFileSrc(sessionId.value, 'review-video')
    const chunks: ArrayBuffer[] = []
    let contentType = 'video/mp4'
    let start = 0
    let total = 0

    while (total === 0 || start < total) {
      const response = await fetch(streamUrl, {
        cache: 'no-store',
        headers: { Range: `bytes=${start}-${start + VIDEO_STREAM_CHUNK_BYTES - 1}` },
        signal: abortController.signal,
      })
      if (!response.ok) throw new Error(`视频分块读取失败（${response.status}）`)
      const contentRange = response.headers.get('content-range')
      const rangeMatch = contentRange?.match(/^bytes\s+(\d+)-(\d+)\/(\d+)$/i)
      const buffer = await response.arrayBuffer()
      if (buffer.byteLength === 0) throw new Error('视频分块读取结果为空')
      chunks.push(buffer)
      contentType = response.headers.get('content-type') || contentType

      if (rangeMatch) {
        const end = Number(rangeMatch[2])
        total = Number(rangeMatch[3])
        start = end + 1
      } else {
        total = buffer.byteLength
        start = total
      }
      if (!Number.isFinite(total) || total <= 0 || start > total) throw new Error('视频分块范围无效')
      videoLoadMessage.value = `正在载入视频预览 ${Math.min(100, Math.round((start / total) * 100))}%`
      if (generation !== videoSourceGeneration) return false
    }

    const objectUrl = URL.createObjectURL(new Blob(chunks, { type: contentType }))
    if (generation !== videoSourceGeneration) {
      URL.revokeObjectURL(objectUrl)
      return false
    }
    previewObjectUrl = objectUrl
    previewVideoUrl.value = objectUrl
    await nextTick()
    isReplacingVideoSource = false
    armVideoLoadTimeout()
    videoRef.value?.load()
    return true
  } catch (error) {
    if (generation === videoSourceGeneration) isReplacingVideoSource = false
    if (abortController.signal.aborted || generation !== videoSourceGeneration) return false
    videoLoadMessage.value = stringifyError(error, '载入视频预览失败')
    return false
  } finally {
    if (videoSourceAbortController === abortController) videoSourceAbortController = null
  }
}

const prepareWorkspace = async () => {
  if (!filesReady.value || isPreparing.value) return
  isPreparing.value = true
  reviewError.value = ''
  try {
    const result = await invoke<PrepareSubtitleReviewResult>('prepare_subtitle_review', {
      request: {
        videoPath: selectedVideoPath.value,
        subtitlePath: selectedSubtitlePath.value,
      },
    })
    applyPreparedWorkspace(result)
    await nextTick()
    observeVideoFrame()
    let canLoadVideo = true
    if (result.video.fileSize > MAX_DIRECT_PREVIEW_BYTES) {
      proxyAttempted.value = true
      canLoadVideo = await prepareProxy(false)
    }
    await Promise.all([
      canLoadVideo ? loadPlaybackVideoSource() : Promise.resolve(false),
      initializeRenderer(result.assContent),
    ])
  } catch (error) {
    reviewError.value = stringifyError(error, '准备字幕审核工作台失败')
  } finally {
    isPreparing.value = false
  }
}

const applyPreparedWorkspace = (result: PrepareSubtitleReviewResult) => {
  sessionId.value = result.sessionId
  videoMetadata.value = result.video
  naturalVideoSize.value = { width: result.video.width, height: result.video.height }
  previewPath.value = result.video.previewPath
  videoDurationMs.value = result.video.durationMs
  subtitleFormat.value = result.subtitleFormat
  styleName.value = result.styleName
  cues.value = cloneReviewCues(result.cues)
  assPreviewConfig.value = parseAssPreviewConfig(result.assContent)
  validation.value = result.validation
  warnings.value = [...result.warnings]
  revision.value = result.revision
  lastExportedRevision.value = result.revision
  selectedCueId.value = buildCueGroups(result.cues)[0]?.id ?? ''
  undoStack.value = []
  redoStack.value = []
  proxyAttempted.value = false
  proxyError.value = ''
  exportProgress.value = 0
  exportOutputPath.value = ''
  exportError.value = ''
}

const observeVideoFrame = () => {
  videoFrameResizeObserver?.disconnect()
  videoFrameResizeObserver = null
  const frame = videoFrameRef.value
  if (!frame) {
    videoFrameSize.value = { width: 0, height: 0 }
    return
  }

  const updateSize = (width: number, height: number) => {
    const nextSize = { width: Math.max(0, width), height: Math.max(0, height) }
    if (
      Math.abs(videoFrameSize.value.width - nextSize.width) < 0.5 &&
      Math.abs(videoFrameSize.value.height - nextSize.height) < 0.5
    ) {
      return
    }
    videoFrameSize.value = nextSize
    if (isRendererReady.value) {
      void nextTick(() => safeResizeRenderer(true).then(() => renderCurrentSubtitleFrame(true)))
    }
  }

  updateSize(frame.clientWidth, frame.clientHeight)
  videoFrameResizeObserver = new ResizeObserver(([entry]) => {
    if (entry) updateSize(entry.contentRect.width, entry.contentRect.height)
  })
  videoFrameResizeObserver.observe(frame)
}

const initializeRenderer = async (assContent: string) => {
  await destroyRenderer()
  const video = videoRef.value
  const layer = subtitleLayerRef.value
  if (!video || !layer) return
  const canvas = document.createElement('canvas')
  canvas.className = 'subtitle-review-subtitle-canvas'
  layer.replaceChildren(canvas)
  subtitleCanvasElement = canvas
  isRendererLoading.value = true
  isRendererReady.value = false
  try {
    const instance = new JASSUB({
      video,
      canvas,
      subContent: assContent,
      workerUrl,
      wasmUrl,
      modernWasmUrl,
      queryFonts: 'local',
    })
    renderer.value = instance
    await instance.ready
    isRendererReady.value = true
    await safeResizeRenderer(true)
    renderCurrentSubtitleFrame(true)
  } catch (error) {
    isRendererReady.value = false
    reviewError.value = stringifyError(error, '字幕实时渲染器加载失败')
    await destroyRenderer()
  } finally {
    isRendererLoading.value = false
  }
}

const queueAssTrack = (assContent: string) => {
  assPreviewConfig.value = parseAssPreviewConfig(assContent)
  const instance = renderer.value
  if (!instance) return
  renderQueue = renderQueue
    .catch(() => undefined)
    .then(async () => {
      if (renderer.value !== instance) return
      await instance.ready
      await instance.renderer.setTrack(assContent)
      await safeResizeRenderer(true)
      renderCurrentSubtitleFrame(true)
    })
    .catch((error: unknown) => {
      reviewError.value = stringifyError(error, '刷新字幕预览失败')
    })
}

const scheduleReviewUpdate = () => {
  revision.value += 1
  if (updateTimer !== undefined) window.clearTimeout(updateTimer)
  updateTimer = window.setTimeout(() => {
    updateTimer = undefined
    void syncReviewState()
  }, 120)
}

const syncReviewState = async (): Promise<boolean> => {
  if (!sessionId.value) return false
  if (updateTimer !== undefined) {
    window.clearTimeout(updateTimer)
    updateTimer = undefined
  }
  const requestRevision = revision.value
  const requestCues = cloneReviewCues(cues.value)
  try {
    const result = await invoke<UpdateSubtitleReviewResult>('update_subtitle_review', {
      request: {
        sessionId: sessionId.value,
        revision: requestRevision,
        cues: requestCues,
      },
    })
    if (result.revision === revision.value) {
      validation.value = result.validation
      queueAssTrack(result.assContent)
    }
    return true
  } catch (error) {
    reviewError.value = stringifyError(error, '保存本次审核修改失败')
    return false
  }
}

const selectCue = (groupId: string, shouldSeek: boolean) => {
  selectedCueId.value = groupId
  if (shouldSeek) {
    const group = sortedCueGroups.value.find((item) => item.id === groupId)
    if (group) seekVideo(group.startTime)
  }
}

const beginCueEdit = () => {
  isCueEditing.value = true
  if (!fieldEditSnapshot) fieldEditSnapshot = cloneReviewCues(cues.value)
}

const commitCueEdit = () => {
  const before = fieldEditSnapshot
  fieldEditSnapshot = null
  isCueEditing.value = false
  if (before && !reviewCuesEqual(before, cues.value)) pushUndoSnapshot(before)
}

const changeCueText = (cueId: string, event: Event) => {
  const cue = cues.value.find((item) => item.id === cueId)
  const target = event.target as HTMLTextAreaElement
  if (!cue) return
  if (cue.textMode === 'raw') {
    cue.rawText = target.value
    cue.text = plainTextFromAss(target.value)
  } else {
    cue.text = target.value
  }
  scheduleReviewUpdate()
}

const changeCueGroupTime = (groupId: string, field: 'start' | 'end', event: Event) => {
  const group = sortedCueGroups.value.find((item) => item.id === groupId)
  const target = event.target as HTMLInputElement
  if (!group) return
  const parsed = parseReviewTime(target.value)
  if (parsed === null) {
    target.value = formatReviewTime(field === 'start' ? group.startTime : group.endTime)
    reviewError.value = '时间格式应为 HH:MM:SS.mmm'
    fieldEditSnapshot = null
    return
  }
  for (const cue of group.cues) {
    if (field === 'start') cue.startTime = parsed
    else cue.endTime = parsed
  }
  target.value = formatReviewTime(parsed)
  scheduleReviewUpdate()
  commitCueEdit()
}

const requestPlainMode = (cueId: string) => {
  const cue = cues.value.find((item) => item.id === cueId)
  if (!cue || cue.textMode === 'plain') return
  pendingPlainCueId.value = cueId
  showPlainModeDialog.value = true
}

const closePlainModeDialog = () => {
  showPlainModeDialog.value = false
  pendingPlainCueId.value = ''
}

const confirmPlainMode = () => {
  const cue = cues.value.find((item) => item.id === pendingPlainCueId.value)
  if (cue) {
    const before = cloneReviewCues(cues.value)
    cue.textMode = 'plain'
    cue.text = plainTextFromAss(cue.rawText)
    pushUndoSnapshot(before)
    scheduleReviewUpdate()
  }
  closePlainModeDialog()
}

const switchCueToRaw = (cueId: string) => {
  const cue = cues.value.find((item) => item.id === cueId)
  if (!cue || cue.textMode === 'raw') return
  const before = cloneReviewCues(cues.value)
  cue.textMode = 'raw'
  pushUndoSnapshot(before)
  scheduleReviewUpdate()
}

const addCueAtPlayhead = () => {
  if (!sessionId.value || isExporting.value) return
  const before = cloneReviewCues(cues.value)
  const previousCues = sortReviewCues(cues.value).filter((cue) => cue.startTime <= currentTimeMs.value)
  const neighbor = selectedCue.value ?? previousCues[previousCues.length - 1]
  const startTime = Math.min(Math.round(currentTimeMs.value), Math.max(0, videoDurationMs.value - MIN_CUE_DURATION_MS))
  const endTime = Math.min(videoDurationMs.value, startTime + DEFAULT_CUE_DURATION_MS)
  const cue: ReviewCue = {
    id: `new-${crypto.randomUUID()}`,
    startTime,
    endTime: Math.max(startTime + MIN_CUE_DURATION_MS, endTime),
    text: '新字幕',
    rawText: '新字幕',
    textMode: 'plain',
    styleName: neighbor?.styleName || (subtitleFormat.value === 'ass' ? 'Default' : 'Primary'),
    layer: neighbor?.layer || '0',
    actor: '',
    marginL: neighbor?.marginL || '0000',
    marginR: neighbor?.marginR || '0000',
    marginV: neighbor?.marginV || '0000',
    effect: '',
    hasInlineTags: false,
    sourceOrder: Math.max(-1, ...cues.value.map((item) => item.sourceOrder)) + 1,
    isNew: true,
  }
  cues.value.push(cue)
  selectedCueId.value = cue.id
  pushUndoSnapshot(before)
  scheduleReviewUpdate()
  nextTick(() => focusSelectedCueText())
}

const deleteCueGroup = (groupId: string) => {
  const groups = sortedCueGroups.value
  const groupIndex = groups.findIndex((group) => group.id === groupId)
  const group = groups[groupIndex]
  if (!group) return
  const before = cloneReviewCues(cues.value)
  const cueIds = new Set(group.cues.map((cue) => cue.id))
  cues.value = cues.value.filter((cue) => !cueIds.has(cue.id))
  if (selectedCueId.value === groupId) {
    const nextGroups = buildCueGroups(cues.value)
    selectedCueId.value = nextGroups[Math.min(groupIndex, nextGroups.length - 1)]?.id ?? ''
  }
  pushUndoSnapshot(before)
  scheduleReviewUpdate()
}

const previewTimelineChange = (cue: ReviewCue) => {
  const group = sortedCueGroups.value.find((item) => item.id === cue.id)
  if (!group) return
  for (const item of group.cues) {
    item.startTime = cue.startTime
    item.endTime = cue.endTime
  }
  scheduleReviewUpdate()
}

const commitTimelineChange = ({ before, after }: { before: ReviewCue; after: ReviewCue }) => {
  const beforeSnapshot = cloneReviewCues(cues.value)
  const group = sortedCueGroups.value.find((item) => item.id === after.id)
  if (!group) return
  const groupCueIds = new Set(group.cues.map((cue) => cue.id))
  for (const cue of beforeSnapshot) {
    if (groupCueIds.has(cue.id)) {
      cue.startTime = before.startTime
      cue.endTime = before.endTime
    }
  }
  for (const cue of group.cues) {
    cue.startTime = after.startTime
    cue.endTime = after.endTime
  }
  pushUndoSnapshot(beforeSnapshot)
  scheduleReviewUpdate()
}

const setSelectedBoundary = (boundary: 'start' | 'end') => {
  const group = selectedCueGroup.value
  if (!group) return
  const before = cloneReviewCues(cues.value)
  if (boundary === 'start') {
    const startTime = Math.min(
      Math.round(currentTimeMs.value),
      Math.max(0, group.endTime - MIN_CUE_DURATION_MS),
    )
    for (const cue of group.cues) cue.startTime = startTime
  } else {
    const endTime = Math.max(
      Math.round(currentTimeMs.value),
      group.startTime + MIN_CUE_DURATION_MS,
    )
    for (const cue of group.cues) cue.endTime = endTime
  }
  pushUndoSnapshot(before)
  scheduleReviewUpdate()
}

const pushUndoSnapshot = (snapshot: ReviewCue[]) => {
  undoStack.value.push(snapshot)
  if (undoStack.value.length > 100) undoStack.value.shift()
  redoStack.value = []
}

const undo = () => {
  const snapshot = undoStack.value.pop()
  if (!snapshot) return
  redoStack.value.push(cloneReviewCues(cues.value))
  cues.value = cloneReviewCues(snapshot)
  ensureSelectedCueExists()
  scheduleReviewUpdate()
}

const redo = () => {
  const snapshot = redoStack.value.pop()
  if (!snapshot) return
  undoStack.value.push(cloneReviewCues(cues.value))
  cues.value = cloneReviewCues(snapshot)
  ensureSelectedCueExists()
  scheduleReviewUpdate()
}

const ensureSelectedCueExists = () => {
  if (!sortedCueGroups.value.some((group) => group.id === selectedCueId.value)) {
    selectedCueId.value = sortedCueGroups.value[0]?.id ?? ''
  }
}

const cueIssues = (cueId: string): ReviewValidationIssue[] =>
  validation.value.issues.filter((issue) => issue.cueId === cueId)

const groupHasError = (group: ReviewCueGroup) =>
  group.cues.some((cue) => cueIssues(cue.id).some((issue) => issue.level === 'error'))

const seekVideo = (timeMs: number) => {
  const video = videoRef.value
  currentTimeMs.value = Math.max(0, Math.min(timeMs, videoDurationMs.value))
  if (video) video.currentTime = currentTimeMs.value / 1_000
}

const updateCurrentTime = () => {
  if (videoRef.value) {
    currentTimeMs.value = Math.round(videoRef.value.currentTime * 1_000)
    drawCurrentVideoFrame()
    if (videoRef.value.paused) renderCurrentSubtitleFrame(true)
  }
}

const startFrameUpdates = () => {
  stopFrameUpdates()
  const video = videoRef.value
  isVideoPlaying.value = Boolean(video && !video.paused)
  if (!video || typeof video.requestVideoFrameCallback !== 'function') return
  const update = (_now: number, metadata: VideoFrameCallbackMetadata) => {
    currentTimeMs.value = Math.round(metadata.mediaTime * 1_000)
    drawCurrentVideoFrame()
    if (!video.paused && !video.ended) videoFrameCallbackId = video.requestVideoFrameCallback(update)
  }
  videoFrameCallbackId = video.requestVideoFrameCallback(update)
}

const stopFrameUpdates = () => {
  const video = videoRef.value
  isVideoPlaying.value = false
  if (video && videoFrameCallbackId !== undefined && typeof video.cancelVideoFrameCallback === 'function') {
    video.cancelVideoFrameCallback(videoFrameCallbackId)
  }
  videoFrameCallbackId = undefined
  updateCurrentTime()
}

const drawCurrentVideoFrame = () => {
  const video = videoRef.value
  const canvas = videoCanvasRef.value
  if (!video || !canvas || video.videoWidth <= 0 || video.videoHeight <= 0) return
  const width = video.videoWidth
  const height = video.videoHeight
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width
    canvas.height = height
  }
  try {
    canvas.getContext('2d')?.drawImage(video, 0, 0, width, height)
    videoCanvasDrawFailed = false
  } catch {
    if (!videoCanvasDrawFailed) {
      videoCanvasDrawFailed = true
      videoLoadMessage.value = '视频帧预览失败，请尝试生成兼容预览'
    }
  }
}

const toggleVideoPlayback = () => {
  const video = videoRef.value
  if (!video) return
  if (video.paused) void video.play()
  else video.pause()
}

const toggleVideoMute = () => {
  const video = videoRef.value
  if (!video) return
  video.muted = !video.muted
  isVideoMuted.value = video.muted
}

const syncVideoVolumeState = () => {
  isVideoPlaying.value = Boolean(videoRef.value && !videoRef.value.paused)
  isVideoMuted.value = Boolean(videoRef.value?.muted)
}

const safeResizeRenderer = async (repaint = true) => {
  const instance = renderer.value
  if (!instance) return
  try {
    await instance.ready
    if (renderer.value === instance) await instance.resize(repaint)
  } catch (error) {
    if (renderer.value === instance) {
      reviewError.value = stringifyError(error, '调整字幕预览尺寸失败')
    }
  }
}

const renderCurrentSubtitleFrame = (repaint = false) => {
  const video = videoRef.value
  const instance = renderer.value
  if (!video || !instance || !isRendererReady.value || video.videoWidth <= 0 || video.videoHeight <= 0) return
  void instance
    .manualRender(
      {
        expectedDisplayTime: performance.now(),
        width: video.videoWidth,
        height: video.videoHeight,
        mediaTime: video.currentTime,
      },
      repaint,
    )
    .catch((error: unknown) => {
      reviewError.value = stringifyError(error, '字幕画面渲染失败')
    })
}

const handleVideoLoaded = () => {
  clearVideoLoadTimeout()
  videoLoadMessage.value = ''
  if (videoRef.value && videoRef.value.duration > 0) {
    naturalVideoSize.value = {
      width: videoRef.value.videoWidth || videoMetadata.value?.width || 0,
      height: videoRef.value.videoHeight || videoMetadata.value?.height || 0,
    }
    const loadedDuration = Math.round(videoRef.value.duration * 1_000)
    if (
      previewPath.value !== videoMetadata.value?.path &&
      videoMetadata.value &&
      Math.abs(loadedDuration - videoMetadata.value.durationMs) > 500 &&
      !warnings.value.includes('预览代理与源视频时长存在差异，请以源视频导出结果为准')
    ) {
      warnings.value.push('预览代理与源视频时长存在差异，请以源视频导出结果为准')
    }
    videoDurationMs.value = loadedDuration
  }
  syncVideoVolumeState()
  drawCurrentVideoFrame()
  void nextTick(() => safeResizeRenderer(true).then(() => renderCurrentSubtitleFrame(true)))
}

const handleVideoError = () => {
  if (isReplacingVideoSource) return
  clearVideoLoadTimeout()
  if (!sessionId.value || proxyAttempted.value || isProxyPreparing.value || previewPath.value !== videoMetadata.value?.path) {
    if (!isProxyPreparing.value) videoLoadMessage.value = '当前视频无法在预览窗口中播放'
    return
  }
  proxyAttempted.value = true
  void prepareProxy()
}

const prepareProxy = async (reloadAfter = true): Promise<boolean> => {
  if (!sessionId.value) return false
  isProxyPreparing.value = true
  proxyProgress.value = 0
  proxyError.value = ''
  proxyMessage.value = '正在生成兼容预览代理'
  videoLoadMessage.value = '视频编码不兼容，正在准备预览代理'
  try {
    const result = await invoke<ProxyResult>('prepare_subtitle_review_proxy', { sessionId: sessionId.value })
    if (result.sessionId === sessionId.value) {
      previewPath.value = result.previewPath
      videoLoadMessage.value = ''
      if (reloadAfter) await loadPlaybackVideoSource()
      return true
    }
  } catch (error) {
    const message = stringifyError(error, '预览代理生成失败')
    if (message.includes('取消')) {
      proxyMessage.value = '预览代理已取消'
      proxyError.value = ''
      videoLoadMessage.value = '预览代理已取消，仍可编辑字幕并尝试导出'
    } else {
      proxyError.value = message
      videoLoadMessage.value = '无法生成兼容预览，但仍可编辑字幕并尝试导出'
    }
    return false
  } finally {
    isProxyPreparing.value = false
  }
  return false
}

const cancelProxy = async () => {
  if (!sessionId.value) return
  await invoke('cancel_subtitle_review_proxy', { sessionId: sessionId.value })
}

const armVideoLoadTimeout = () => {
  clearVideoLoadTimeout()
  videoLoadTimer = window.setTimeout(() => {
    videoLoadTimer = undefined
    if (previewPath.value === videoMetadata.value?.path && !proxyAttempted.value && !isProxyPreparing.value) {
      handleVideoError()
    }
  }, 6_000)
}

const clearVideoLoadTimeout = () => {
  if (videoLoadTimer !== undefined) {
    window.clearTimeout(videoLoadTimer)
    videoLoadTimer = undefined
  }
}

const exportReviewedVideo = async () => {
  if (!canExport.value || !videoMetadata.value) return
  const synced = await syncReviewState()
  if (!synced || !validation.value.canExport) return
  try {
    const selected = await save({
      title: '导出审核完成的视频',
      defaultPath: buildSuggestedOutputPath(videoMetadata.value.path),
      filters: [{ name: 'MP4 视频', extensions: ['mp4'] }],
    })
    if (!selected) return
    const outputPath = ensureMp4Extension(selected)
    exportJobId.value = crypto.randomUUID()
    exportProgress.value = 0
    exportError.value = ''
    exportOutputPath.value = ''
    exportMessage.value = '准备导出视频'
    isExporting.value = true
    const result = await invoke<ExportResult>('start_subtitle_review_export', {
      request: {
        sessionId: sessionId.value,
        jobId: exportJobId.value,
        revision: revision.value,
        cues: cloneReviewCues(cues.value),
        outputPath,
      },
    })
    exportOutputPath.value = result.outputPath
    exportProgress.value = 100
    exportMessage.value = '视频导出完成'
    lastExportedRevision.value = revision.value
  } catch (error) {
    const message = stringifyError(error, '审核视频导出失败')
    if (!message.includes('任务已取消')) exportError.value = message
  } finally {
    isExporting.value = false
    exportJobId.value = ''
  }
}

const cancelExport = async () => {
  if (!exportJobId.value) return
  await invoke('cancel_subtitle_review_export', { jobId: exportJobId.value })
}

const openExportOutput = async () => {
  if (!exportOutputPath.value) return
  try {
    await revealItemInDir(exportOutputPath.value)
  } catch (error) {
    reviewError.value = stringifyError(error, '打开导出位置失败')
  }
}

const requestResetWorkspace = () => {
  if (hasUnexportedChanges.value) showResetDialog.value = true
  else void resetWorkspace()
}

const resetWorkspace = async () => {
  showResetDialog.value = false
  clearVideoLoadTimeout()
  if (sessionId.value) {
    await invoke('release_subtitle_review_session', { sessionId: sessionId.value }).catch(() => undefined)
  }
  await destroyRenderer()
  releasePlaybackVideoSource()
  sessionId.value = ''
  videoMetadata.value = null
  naturalVideoSize.value = { width: 0, height: 0 }
  videoFrameSize.value = { width: 0, height: 0 }
  previewPath.value = ''
  cues.value = []
  assPreviewConfig.value = createDefaultAssPreviewConfig()
  validation.value = { canExport: false, issues: [] }
  warnings.value = []
  selectedCueId.value = ''
  selectedVideoPath.value = ''
  selectedSubtitlePath.value = ''
  currentTimeMs.value = 0
  videoDurationMs.value = 0
  revision.value = 0
  lastExportedRevision.value = 0
  undoStack.value = []
  redoStack.value = []
  reviewError.value = ''
  exportOutputPath.value = ''
  exportProgress.value = 0
  proxyAttempted.value = false
}

const destroyRenderer = async () => {
  const instance = renderer.value
  renderer.value = null
  isRendererReady.value = false
  if (instance) await instance.destroy().catch(() => undefined)
  subtitleCanvasElement?.remove()
  subtitleCanvasElement = null
  subtitleLayerRef.value?.replaceChildren()
}

const registerProgressListeners = async () => {
  if (!isTauriRuntime()) return
  unlistenProxyProgress = await listen<ProxyProgress>('subtitle-review-proxy-progress', ({ payload }) => {
    if (payload.sessionId !== sessionId.value) return
    proxyProgress.value = clampProgress(payload.progress)
    proxyMessage.value = payload.message
    isProxyPreparing.value = payload.status === 'running'
    if (payload.status === 'failed') proxyError.value = payload.message
    if (payload.previewPath) previewPath.value = payload.previewPath
  })
  unlistenExportProgress = await listen<ExportProgress>('subtitle-review-export-progress', ({ payload }) => {
    if (payload.sessionId !== sessionId.value || payload.jobId !== exportJobId.value) return
    exportProgress.value = clampProgress(payload.progress)
    exportMessage.value = payload.message
    if (payload.status === 'failed') exportError.value = payload.message
    if (payload.status === 'cancelled') {
      exportMessage.value = '视频导出已取消'
      exportProgress.value = 0
    }
    if (payload.outputPath && payload.status === 'done') exportOutputPath.value = payload.outputPath
  })
}

const registerNativeDragDrop = async () => {
  if (!isTauriRuntime()) return
  const { getCurrentWebview } = await import('@tauri-apps/api/webview')
  unlistenDragDrop = await getCurrentWebview().onDragDropEvent((event) => {
    const payload = event.payload
    if (payload.type === 'leave') {
      nativeDragKind.value = null
      return
    }
    if (payload.type === 'enter') {
      nativeDragKind.value = detectFileKind(payload.paths[0] ?? '')
      return
    }
    if (payload.type === 'drop') {
      nativeDragKind.value = null
      const path = payload.paths[0]
      if (path && !sessionId.value) applyInputFile(path)
    }
  })
}

const handleKeyboardShortcut = (event: KeyboardEvent) => {
  if (!isViewActive) return
  if (event.key === 'Escape' && showPlainModeDialog.value) {
    event.preventDefault()
    closePlainModeDialog()
    return
  }
  if (event.key === 'Escape' && showResetDialog.value) {
    event.preventDefault()
    showResetDialog.value = false
    return
  }
  if (!sessionId.value || showPlainModeDialog.value || showResetDialog.value) return
  const target = event.target as HTMLElement | null
  const isEditing = target?.matches('input, textarea, [contenteditable="true"]') ?? false
  if ((event.ctrlKey || event.metaKey) && !event.shiftKey && event.key.toLowerCase() === 'z' && !isEditing) {
    event.preventDefault()
    undo()
    return
  }
  if ((event.ctrlKey || event.metaKey) && (event.key.toLowerCase() === 'y' || (event.shiftKey && event.key.toLowerCase() === 'z')) && !isEditing) {
    event.preventDefault()
    redo()
    return
  }
  if (isEditing || isExporting.value) return
  if (event.code === 'Space') {
    event.preventDefault()
    const video = videoRef.value
    if (!video) return
    if (video.paused) void video.play()
    else video.pause()
  } else if (event.key === 'Delete') {
    event.preventDefault()
    if (selectedCueId.value) deleteCueGroup(selectedCueId.value)
  } else if (event.key === '[') {
    event.preventDefault()
    setSelectedBoundary('start')
  } else if (event.key === ']') {
    event.preventDefault()
    setSelectedBoundary('end')
  } else if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
    event.preventDefault()
    moveCueSelection(event.key === 'ArrowUp' ? -1 : 1)
  }
}

const moveCueSelection = (offset: -1 | 1) => {
  const groups = sortedCueGroups.value
  if (groups.length === 0) return
  const currentIndex = groups.findIndex((group) => group.id === selectedCueId.value)
  const nextIndex = Math.max(0, Math.min(groups.length - 1, currentIndex + offset))
  selectCue(groups[nextIndex]?.id ?? groups[0]!.id, true)
  nextTick(() => scrollSelectedCueIntoView())
}

const focusSelectedCueText = () => {
  scrollCueIntoListView(selectedCueId.value, 'auto')
  const row = cueListRef.value?.querySelector<HTMLElement>(`[data-cue-id="${cssEscape(selectedCueId.value)}"]`)
  row?.querySelector<HTMLTextAreaElement>('textarea')?.focus()
}

const scrollSelectedCueIntoView = () => {
  scrollCueIntoListView(selectedCueId.value, 'smooth')
}

const scrollCueIntoListView = (cueId: string, behavior: ScrollBehavior) => {
  const list = cueListRef.value
  const row = list?.querySelector<HTMLElement>(`[data-cue-id="${cssEscape(cueId)}"]`)
  if (!list || !row) return
  const listRect = list.getBoundingClientRect()
  const rowRect = row.getBoundingClientRect()
  const padding = 8
  const visibleTop = listRect.top + padding
  const visibleBottom = listRect.bottom - padding
  let top: number | null = null
  if (rowRect.top < visibleTop) {
    top = list.scrollTop + rowRect.top - visibleTop
  } else if (rowRect.bottom > visibleBottom) {
    top = list.scrollTop + rowRect.bottom - visibleBottom
  }
  if (top !== null) list.scrollTo({ top: Math.max(0, top), behavior })
}

const syncSelectedCueToPlayback = () => {
  if (isCueEditing.value) return
  const playbackCueId = findPlaybackCueId(timelineCues.value, currentTimeMs.value, selectedCueId.value)
  if (!playbackCueId || playbackCueId === selectedCueId.value) return
  selectedCueId.value = playbackCueId
  void nextTick(() => scrollCueIntoListView(playbackCueId, 'auto'))
}

const blurEventTarget = (event: Event) => (event.target as HTMLElement).blur()
const detectFileKind = (path: string): FileKind | null => {
  const extension = fileExtension(path)
  if (videoExtensions.includes(extension as (typeof videoExtensions)[number])) return FileKind.Video
  if (subtitleExtensions.includes(extension as (typeof subtitleExtensions)[number])) return FileKind.Subtitle
  return null
}
const fileNameFromPath = (path: string) => path.replace(/\\/g, '/').split('/').filter(Boolean).pop() ?? ''
const fileExtension = (path: string) => fileNameFromPath(path).split('.').pop()?.toLowerCase() ?? ''
const buildSuggestedOutputPath = (path: string) => `${path.replace(/\.[^/.\\]+$/, '')}_reviewed.mp4`
const ensureMp4Extension = (path: string) => (path.toLowerCase().endsWith('.mp4') ? path : `${path}.mp4`)
const clampProgress = (value: number) => Math.min(100, Math.max(0, Math.round(value)))
const cssEscape = (value: string) => (typeof CSS !== 'undefined' && CSS.escape ? CSS.escape(value) : value.replace(/["\\]/g, '\\$&'))
const plainTextFromAss = (value: string) =>
  value
    .replace(/\{[^}]*\}/g, '')
    .replace(/\\[Nn]/g, '\n')
    .replace(/\\h/g, ' ')
    .trim()
const buildCueGroups = (items: readonly ReviewCue[]): ReviewCueGroup[] => {
  const sorted = sortReviewCues(items)
  const consumed = new Set<string>()
  const groups: ReviewCueGroup[] = []
  for (const cue of sorted) {
    if (consumed.has(cue.id)) continue
    const partner = sorted.find(
      (candidate) =>
        candidate.id !== cue.id &&
        !consumed.has(candidate.id) &&
        candidate.startTime === cue.startTime &&
        candidate.endTime === cue.endTime &&
        isBilingualCuePair(cue, candidate),
    )
    const groupCues = partner
      ? [cue, partner].sort((left, right) => left.sourceOrder - right.sourceOrder)
      : [cue]
    for (const item of groupCues) consumed.add(item.id)
    groups.push({
      id: groupCues[0]!.id,
      cues: groupCues,
      startTime: cue.startTime,
      endTime: cue.endTime,
      isBilingual: groupCues.length > 1,
    })
  }
  return groups
}
const groupToTimelineCue = (group: ReviewCueGroup): ReviewCue => {
  const first = group.cues[0]!
  return {
    ...first,
    id: group.id,
    startTime: group.startTime,
    endTime: group.endTime,
    text: group.cues.map((cue) => cue.text.trim()).filter(Boolean).join(' / '),
    rawText: group.cues.map((cue) => cue.rawText.trim()).filter(Boolean).join(' / '),
    hasInlineTags: group.cues.some((cue) => cue.hasInlineTags),
    sourceOrder: Math.min(...group.cues.map((cue) => cue.sourceOrder)),
    isNew: group.cues.some((cue) => cue.isNew),
  }
}
const isBilingualCuePair = (left: ReviewCue, right: ReviewCue) => {
  if (left.styleName.trim().toLowerCase() === right.styleName.trim().toLowerCase()) return false
  const leftHasCjk = hasCjkText(left.text)
  const rightHasCjk = hasCjkText(right.text)
  if (leftHasCjk !== rightHasCjk) return true
  return isBilingualStyleName(left.styleName) || isBilingualStyleName(right.styleName)
}
const isBilingualStyleName = (value: string) =>
  ['default', 'primary', 'secondary', 'source', 'target', 'original', 'translation'].includes(
    value.trim().toLowerCase(),
  )
const hasCjkText = (value: string) => /[\u3400-\u4DBF\u4E00-\u9FFF\uF900-\uFAFF\u3040-\u30FF\uAC00-\uD7AF]/u.test(value)
const groupStyleLabel = (group: ReviewCueGroup) =>
  [...new Set(group.cues.map((cue) => cue.styleName).filter(Boolean))].join(' · ')
const cueRoleLabel = (cue: ReviewCue, index: number, group: ReviewCueGroup) => {
  if (!group.isBilingual) return '字幕内容'
  const style = cue.styleName.trim().toLowerCase()
  if (['secondary', 'source', 'original'].includes(style)) return '原文'
  if (['target', 'translation'].includes(style)) return '译文'
  const other = group.cues.find((item) => item.id !== cue.id)
  if (other && hasCjkText(cue.text) !== hasCjkText(other.text)) {
    return hasCjkText(cue.text) ? '中文' : '外语'
  }
  return `字幕 ${index + 1}`
}

function createDefaultAssPreviewConfig(): AssPreviewConfig {
  return {
    playResX: 1280,
    playResY: 720,
    styles: {
      default: {
        fontFamily: 'Microsoft YaHei, sans-serif',
        fontSize: 32,
        primaryColor: 'rgba(255, 255, 255, 1)',
        outlineColor: 'rgba(0, 0, 0, 1)',
        backgroundColor: 'rgba(0, 0, 0, 0.78)',
        bold: true,
        italic: false,
        underline: false,
        strikeOut: false,
        scaleX: 100,
        scaleY: 100,
        spacing: 0,
        angle: 0,
        borderStyle: 1,
        outline: 2,
        shadow: 0,
        alignment: 2,
        marginL: 10,
        marginR: 10,
        marginV: 24,
      },
    },
  }
}

function parseAssPreviewConfig(content: string): AssPreviewConfig {
  const fallback = createDefaultAssPreviewConfig()
  const styles: Record<string, AssPreviewStyle> = {}
  let playResX = fallback.playResX
  let playResY = fallback.playResY
  let section = ''
  let styleFormat: string[] = []

  for (const rawLine of content.replace(/^\uFEFF/, '').split(/\r?\n/)) {
    const line = rawLine.trim()
    if (!line || line.startsWith(';')) continue
    if (line.startsWith('[') && line.endsWith(']')) {
      section = line.toLowerCase()
      continue
    }
    if (section === '[script info]') {
      const [key, value] = line.split(':', 2)
      const normalizedKey = key?.trim().toLowerCase()
      if (normalizedKey === 'playresx') playResX = Math.max(1, Number(value) || playResX)
      if (normalizedKey === 'playresy') playResY = Math.max(1, Number(value) || playResY)
      continue
    }
    if (section !== '[v4+ styles]' && section !== '[v4 styles]') continue
    if (/^format\s*:/i.test(line)) {
      styleFormat = line.slice(line.indexOf(':') + 1).split(',').map((field) => field.trim().toLowerCase())
      continue
    }
    if (!/^style\s*:/i.test(line) || styleFormat.length === 0) continue
    const values = line.slice(line.indexOf(':') + 1).split(',')
    const fields = Object.fromEntries(styleFormat.map((field, index) => [field, values[index]?.trim() ?? '']))
    const name = fields.name || 'Default'
    const fallbackStyle = fallback.styles.default!
    styles[name.toLowerCase()] = {
      fontFamily: fields.fontname ? `${quoteCssFontFamily(fields.fontname)}, Microsoft YaHei, sans-serif` : fallbackStyle.fontFamily,
      fontSize: positiveNumber(fields.fontsize, fallbackStyle.fontSize),
      primaryColor: assColorToCss(fields.primarycolour, fallbackStyle.primaryColor),
      outlineColor: assColorToCss(fields.outlinecolour, fallbackStyle.outlineColor),
      backgroundColor: assColorToCss(fields.backcolour, fallbackStyle.backgroundColor),
      bold: assBoolean(fields.bold, fallbackStyle.bold),
      italic: assBoolean(fields.italic, fallbackStyle.italic),
      underline: assBoolean(fields.underline, fallbackStyle.underline),
      strikeOut: assBoolean(fields.strikeout, fallbackStyle.strikeOut),
      scaleX: positiveNumber(fields.scalex, fallbackStyle.scaleX),
      scaleY: positiveNumber(fields.scaley, fallbackStyle.scaleY),
      spacing: finiteNumber(fields.spacing, fallbackStyle.spacing),
      angle: finiteNumber(fields.angle, fallbackStyle.angle),
      borderStyle: Math.round(clampNumber(finiteNumber(fields.borderstyle, fallbackStyle.borderStyle), 1, 4)),
      outline: Math.max(0, finiteNumber(fields.outline, fallbackStyle.outline)),
      shadow: Math.max(0, finiteNumber(fields.shadow, fallbackStyle.shadow)),
      alignment: Math.round(clampNumber(finiteNumber(fields.alignment, fallbackStyle.alignment), 1, 9)),
      marginL: Math.max(0, finiteNumber(fields.marginl, fallbackStyle.marginL)),
      marginR: Math.max(0, finiteNumber(fields.marginr, fallbackStyle.marginR)),
      marginV: Math.max(0, finiteNumber(fields.marginv, fallbackStyle.marginV)),
    }
  }

  return { playResX, playResY, styles: Object.keys(styles).length > 0 ? styles : fallback.styles }
}

function finiteNumber(value: string | undefined, fallback: number): number {
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : fallback
}

function positiveNumber(value: string | undefined, fallback: number): number {
  const parsed = finiteNumber(value, fallback)
  return parsed > 0 ? parsed : fallback
}

function assBoolean(value: string | undefined, fallback: boolean): boolean {
  if (value === undefined || value.trim() === '') return fallback
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed !== 0 : fallback
}

function quoteCssFontFamily(value: string): string {
  const sanitized = value.trim().replace(/['\\]/g, '\\$&')
  return sanitized ? `'${sanitized}'` : 'Microsoft YaHei'
}

function assColorToCss(value: string | undefined, fallback: string): string {
  const hex = value?.replace(/^&H/i, '').replace(/&$/, '').padStart(8, '0').slice(-8)
  if (!hex || !/^[0-9a-f]{8}$/i.test(hex)) return fallback
  const alpha = 1 - Number.parseInt(hex.slice(0, 2), 16) / 255
  const blue = Number.parseInt(hex.slice(2, 4), 16)
  const green = Number.parseInt(hex.slice(4, 6), 16)
  const red = Number.parseInt(hex.slice(6, 8), 16)
  return `rgba(${red}, ${green}, ${blue}, ${alpha.toFixed(3)})`
}

function previewStyleForCue(cue?: ReviewCue): AssPreviewStyle {
  const styles = assPreviewConfig.value.styles
  return styles[cue?.styleName.trim().toLowerCase() || 'default'] ?? styles.default ?? createDefaultAssPreviewConfig().styles.default!
}

function captionFallbackText(cue: ReviewCue): string {
  return plainTextFromAss(cue.textMode === 'raw' ? cue.rawText : cue.text)
}

function captionFallbackStyle(cue: ReviewCue): CSSProperties {
  const style = previewStyleForCue(cue)
  const overrides = parseAssInlineOverrides(cue)
  const stage = getCaptionStageSize()
  const scaleX = stage.width / Math.max(1, assPreviewConfig.value.playResX)
  const scaleY = stage.height / Math.max(1, assPreviewConfig.value.playResY)
  const alignment = Math.round(clampNumber(overrides.alignment ?? style.alignment, 1, 9))
  const horizontal = assHorizontalAlignment(alignment)
  const vertical = assVerticalAlignment(alignment)
  const positioned = overrides.position
  const marginL = cueMarginValue(cue.marginL, style.marginL) * scaleX
  const marginR = cueMarginValue(cue.marginR, style.marginR) * scaleX
  const marginV = cueMarginValue(cue.marginV, style.marginV) * scaleY
  const fontSize = Math.max(9, (overrides.fontSize ?? style.fontSize) * scaleY * (style.scaleY / 100))
  const outline = Math.max(0, (overrides.outline ?? style.outline) * scaleY)
  const shadow = Math.max(0, (overrides.shadow ?? style.shadow) * scaleY)
  const lineHeight = fontSize * 1.18
  const maxWidth = Math.max(40, stage.width - marginL - marginR)
  const lineCount = estimateCaptionLineCount(cue, fontSize, maxWidth)
  const paddingY = style.borderStyle === 3 ? Math.max(2, outline) * 2 : 0
  const boxHeight = lineHeight * lineCount + paddingY
  const stackGap = Math.max(2, 5 * scaleY)
  const stackOffset = positioned ? 0 : captionFallbackStackOffset(cue, alignment, vertical, boxHeight, stackGap)
  const textColor = overrides.primaryColor ?? style.primaryColor
  const outlineColor = overrides.outlineColor ?? style.outlineColor
  const textShadow = [
    outline > 0 ? `${outline}px 0 ${outlineColor}` : '',
    outline > 0 ? `${-outline}px 0 ${outlineColor}` : '',
    outline > 0 ? `0 ${outline}px ${outlineColor}` : '',
    outline > 0 ? `0 ${-outline}px ${outlineColor}` : '',
    outline > 0 ? `${outline}px ${outline}px ${outlineColor}` : '',
    outline > 0 ? `${-outline}px ${outline}px ${outlineColor}` : '',
    outline > 0 ? `${outline}px ${-outline}px ${outlineColor}` : '',
    outline > 0 ? `${-outline}px ${-outline}px ${outlineColor}` : '',
    shadow > 0 ? `${shadow}px ${shadow}px ${outlineColor}` : '',
  ].filter(Boolean)
  const transform: string[] = []
  const base: CSSProperties = {
    position: 'absolute',
    color: textColor,
    fontFamily: overrides.fontFamily
      ? `${quoteCssFontFamily(overrides.fontFamily)}, ${style.fontFamily}`
      : style.fontFamily,
    fontSize: `${fontSize}px`,
    fontWeight: (overrides.bold ?? style.bold) ? 700 : 400,
    fontStyle: (overrides.italic ?? style.italic) ? 'italic' : 'normal',
    textDecoration: [
      (overrides.underline ?? style.underline) ? 'underline' : '',
      style.strikeOut ? 'line-through' : '',
    ]
      .filter(Boolean)
      .join(' ') || 'none',
    letterSpacing: style.spacing ? `${style.spacing * scaleX}px` : '0',
    lineHeight: `${lineHeight}px`,
    maxWidth: `${maxWidth}px`,
    overflowWrap: 'anywhere',
    textAlign: horizontal,
    textShadow: style.borderStyle === 3 ? undefined : textShadow.join(', '),
    whiteSpace: 'pre-wrap',
  }

  if (style.borderStyle === 3) {
    base.backgroundColor = style.backgroundColor
    base.borderRadius = '4px'
    base.padding = `${Math.max(2, outline)}px ${Math.max(5, outline * 1.8)}px`
  }

  if (positioned) {
    base.left = `${positioned.x * scaleX}px`
    base.top = `${positioned.y * scaleY}px`
    const translateX = horizontal === 'center' ? '-50%' : horizontal === 'right' ? '-100%' : '0'
    const translateY = vertical === 'middle' ? '-50%' : vertical === 'bottom' ? '-100%' : '0'
    if (translateX !== '0' || translateY !== '0') transform.push(`translate(${translateX}, ${translateY})`)
  } else {
    base.left = `${marginL}px`
    base.right = `${marginR}px`
    if (vertical === 'top') {
      base.top = `${marginV + stackOffset}px`
    } else if (vertical === 'middle') {
      base.top = `calc(50% + ${stackOffset}px)`
      transform.push('translateY(-50%)')
    } else {
      base.bottom = `${marginV + stackOffset}px`
    }
  }

  if (style.angle) transform.push(`rotate(${style.angle}deg)`)
  if (transform.length > 0) base.transform = transform.join(' ')
  return base
}

function captionFallbackStackOffset(
  cue: ReviewCue,
  alignment: number,
  vertical: 'top' | 'middle' | 'bottom',
  boxHeight: number,
  gap: number,
): number {
  const stackItems = activePreviewCues.value.filter((item) => {
    const itemStyle = previewStyleForCue(item)
    const itemOverrides = parseAssInlineOverrides(item)
    if (itemOverrides.position) return false
    const itemAlignment = Math.round(clampNumber(itemOverrides.alignment ?? itemStyle.alignment, 1, 9))
    return itemAlignment === alignment
  })
  if (stackItems.length <= 1) return 0

  const itemHeights = stackItems.map((item) => {
    if (item.id === cue.id) return boxHeight
    return captionFallbackBoxHeight(item)
  })
  const index = stackItems.findIndex((item) => item.id === cue.id)
  if (index < 0) return 0

  if (vertical === 'bottom') {
    return itemHeights.slice(0, index).reduce((sum, height) => sum + height + gap, 0)
  }
  if (vertical === 'top') {
    return itemHeights.slice(0, index).reduce((sum, height) => sum + height + gap, 0)
  }

  const totalHeight = itemHeights.reduce((sum, height) => sum + height, 0) + gap * (itemHeights.length - 1)
  const beforeHeight = itemHeights.slice(0, index).reduce((sum, height) => sum + height + gap, 0)
  return beforeHeight + itemHeights[index]! / 2 - totalHeight / 2
}

function captionFallbackBoxHeight(cue: ReviewCue): number {
  const style = previewStyleForCue(cue)
  const overrides = parseAssInlineOverrides(cue)
  const stage = getCaptionStageSize()
  const scaleX = stage.width / Math.max(1, assPreviewConfig.value.playResX)
  const scaleY = stage.height / Math.max(1, assPreviewConfig.value.playResY)
  const marginL = cueMarginValue(cue.marginL, style.marginL) * scaleX
  const marginR = cueMarginValue(cue.marginR, style.marginR) * scaleX
  const fontSize = Math.max(9, (overrides.fontSize ?? style.fontSize) * scaleY * (style.scaleY / 100))
  const outline = Math.max(0, (overrides.outline ?? style.outline) * scaleY)
  const paddingY = style.borderStyle === 3 ? Math.max(2, outline) * 2 : 0
  const maxWidth = Math.max(40, stage.width - marginL - marginR)
  const lineCount = estimateCaptionLineCount(cue, fontSize, maxWidth)
  return fontSize * 1.18 * lineCount + paddingY
}

function estimateCaptionLineCount(cue: ReviewCue, fontSize: number, maxWidth: number): number {
  return captionFallbackText(cue)
    .split('\n')
    .reduce((count, line) => {
      const visualWidth = Array.from(line).reduce((sum, character) => {
        return sum + fontSize * (/[\u3400-\u4DBF\u4E00-\u9FFF\uF900-\uFAFF\u3040-\u30FF\uAC00-\uD7AF]/u.test(character) ? 1 : 0.55)
      }, 0)
      return count + Math.max(1, Math.ceil(visualWidth / Math.max(1, maxWidth)))
    }, 0)
}

function getCaptionStageSize() {
  const stage = videoStageSize.value
  return {
    width: stage.width > 0 ? stage.width : assPreviewConfig.value.playResX,
    height: stage.height > 0 ? stage.height : assPreviewConfig.value.playResY,
  }
}

function cueMarginValue(value: string, fallback: number): number {
  const parsed = Number(value)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback
}

function assHorizontalAlignment(alignment: number): 'left' | 'center' | 'right' {
  const column = ((alignment - 1) % 3) + 1
  if (column === 1) return 'left'
  if (column === 3) return 'right'
  return 'center'
}

function assVerticalAlignment(alignment: number): 'top' | 'middle' | 'bottom' {
  if (alignment >= 7) return 'top'
  if (alignment >= 4) return 'middle'
  return 'bottom'
}

function parseAssInlineOverrides(cue: ReviewCue): AssInlineOverrides {
  if (cue.textMode !== 'raw' || !cue.rawText.includes('{')) return {}
  const overrides: AssInlineOverrides = {}
  const tags = cue.rawText.match(/\{[^}]*\}/g) ?? []
  for (const rawTag of tags) {
    const tag = rawTag.slice(1, -1)
    const alignment = tag.match(/\\an([1-9])/i)
    if (alignment) overrides.alignment = Number(alignment[1])

    const position = tag.match(/\\pos\s*\(\s*(-?\d+(?:\.\d+)?)\s*,\s*(-?\d+(?:\.\d+)?)\s*\)/i)
    if (position) {
      overrides.position = { x: Number(position[1]), y: Number(position[2]) }
    }

    const move = tag.match(
      /\\move\s*\(\s*(-?\d+(?:\.\d+)?)\s*,\s*(-?\d+(?:\.\d+)?)\s*,\s*(-?\d+(?:\.\d+)?)\s*,\s*(-?\d+(?:\.\d+)?)(?:\s*,\s*(\d+)\s*,\s*(\d+))?\s*\)/i,
    )
    if (move) {
      const start = { x: Number(move[1]), y: Number(move[2]) }
      const end = { x: Number(move[3]), y: Number(move[4]) }
      const moveStart = Number(move[5] ?? 0)
      const moveEnd = Number(move[6] ?? Math.max(1, cue.endTime - cue.startTime))
      const elapsed = clampNumber(currentTimeMs.value - cue.startTime, moveStart, moveEnd)
      const ratio = moveEnd > moveStart ? (elapsed - moveStart) / (moveEnd - moveStart) : 1
      overrides.position = {
        x: start.x + (end.x - start.x) * ratio,
        y: start.y + (end.y - start.y) * ratio,
      }
    }

    applyNumericAssOverride(tag, /\\fs(\d+(?:\.\d+)?)/i, (value) => {
      overrides.fontSize = value
    })
    applyNumericAssOverride(tag, /\\bord(\d+(?:\.\d+)?)/i, (value) => {
      overrides.outline = value
    })
    applyNumericAssOverride(tag, /\\shad(\d+(?:\.\d+)?)/i, (value) => {
      overrides.shadow = value
    })

    const primaryColor = tag.match(/\\(?:1c|c)&H([0-9a-f]{6,8})&?/i)
    if (primaryColor) overrides.primaryColor = assColorToCss(`&H${primaryColor[1]}`, overrides.primaryColor ?? '#fff')
    const outlineColor = tag.match(/\\3c&H([0-9a-f]{6,8})&?/i)
    if (outlineColor) overrides.outlineColor = assColorToCss(`&H${outlineColor[1]}`, overrides.outlineColor ?? '#000')

    const fontFamily = tag.match(/\\fn([^\\}]+)/i)
    if (fontFamily) overrides.fontFamily = fontFamily[1]?.trim()

    const bold = tag.match(/\\b(-?1|0)?/i)
    if (bold && bold[1] !== undefined) overrides.bold = Number(bold[1]) !== 0
    const italic = tag.match(/\\i(-?1|0)?/i)
    if (italic && italic[1] !== undefined) overrides.italic = Number(italic[1]) !== 0
    const underline = tag.match(/\\u(-?1|0)?/i)
    if (underline && underline[1] !== undefined) overrides.underline = Number(underline[1]) !== 0
  }
  return overrides
}

function applyNumericAssOverride(tag: string, pattern: RegExp, apply: (value: number) => void) {
  const match = tag.match(pattern)
  if (!match) return
  const value = Number(match[1])
  if (Number.isFinite(value)) apply(value)
}

const stringifyError = (error: unknown, fallback: string) => {
  if (typeof error === 'string') return error
  if (error instanceof Error) return error.message
  return fallback
}

watch(currentTimeMs, syncSelectedCueToPlayback)
watch(timelineCues, syncSelectedCueToPlayback)
watch(isCueEditing, (editing) => {
  if (!editing) void nextTick(syncSelectedCueToPlayback)
})

onMounted(() => {
  isViewActive = true
  window.addEventListener('keydown', handleKeyboardShortcut)
  void registerProgressListeners()
  void registerNativeDragDrop()
  observeVideoFrame()
})

onActivated(() => {
  isViewActive = true
  nextTick(() => {
    observeVideoFrame()
    void safeResizeRenderer(true).then(() => renderCurrentSubtitleFrame(true))
  })
})

onDeactivated(() => {
  isViewActive = false
  videoFrameResizeObserver?.disconnect()
  videoFrameResizeObserver = null
  videoRef.value?.pause()
  stopFrameUpdates()
})

onBeforeUnmount(() => {
  isViewActive = false
  videoFrameResizeObserver?.disconnect()
  videoFrameResizeObserver = null
  window.removeEventListener('keydown', handleKeyboardShortcut)
  if (updateTimer !== undefined) window.clearTimeout(updateTimer)
  clearVideoLoadTimeout()
  releasePlaybackVideoSource()
  stopFrameUpdates()
  unlistenProxyProgress?.()
  unlistenExportProgress?.()
  unlistenDragDrop?.()
  void destroyRenderer()
  if (sessionId.value) void invoke('release_subtitle_review_session', { sessionId: sessionId.value })
})
</script>

<style scoped>
.subtitle-review-workspace {
  display: flex;
  flex-direction: column;
  gap: 22px;
}

.subtitle-review-workspace > .settings-section + .settings-section { margin-top: 0; }

.subtitle-review-header {
  min-height: 46px;
  gap: 18px;
}

.subtitle-review-title-group,
.subtitle-review-header-actions,
.subtitle-review-filebar,
.subtitle-review-ready-panel,
.subtitle-review-proxy-strip,
.subtitle-review-transport,
.subtitle-review-cue-toolbar,
.subtitle-review-export-bar {
  display: flex;
  align-items: center;
}

.subtitle-review-title-group { gap: 12px; }

.subtitle-review-session-state {
  padding: 4px 8px;
  border: 1px solid var(--border);
  border-radius: 999px;
  color: var(--text-subtle);
  font-size: 10px;
  font-weight: 700;
}

.subtitle-review-session-state.dirty {
  color: var(--accent-strong);
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
  background: var(--accent-soft);
}

.subtitle-review-header-actions {
  justify-content: flex-end;
  flex-wrap: wrap;
  margin-left: auto;
  gap: 8px;
}
.settings-action.icon-only { width: 36px; padding: 0; justify-content: center; }

.subtitle-review-primary-action {
  color: var(--bg);
  border-color: var(--accent);
  background: var(--accent);
}

.subtitle-review-primary-action:hover:not(:disabled) { background: var(--accent-strong); }

.subtitle-review-import-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 26px;
}

.subtitle-review-import-grid > .settings-section + .settings-section { margin-top: 0; }
.subtitle-review-drop-panel { height: 100%; }
.subtitle-review-drop-zone { min-height: 208px; }
.subtitle-review-ready-section { margin-top: 0; }

.subtitle-review-ready-panel {
  min-height: 78px;
  justify-content: space-between;
  gap: 20px;
  padding: 16px 18px;
}

.subtitle-review-editor-section { min-height: 0; margin-top: 0; }
.subtitle-review-editor-heading { display: flex; align-items: center; }
.subtitle-review-heading-meta { margin-left: auto; color: var(--text-subtle); font-size: 11px; font-weight: 600; }

.subtitle-review-editor-panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  padding: 0;
  border-radius: var(--radius-panel);
}

.subtitle-review-filebar {
  min-height: 55px;
  flex-wrap: wrap;
  gap: 10px 18px;
  padding: 9px 14px;
  border-bottom: 1px solid var(--hairline);
  background: color-mix(in srgb, var(--bg-surface) 82%, var(--bg));
}

.subtitle-review-filebar-item {
  min-width: 0;
  flex: 0 1 min(360px, 38%);
  display: grid;
  grid-template-columns: 18px minmax(0, auto);
  column-gap: 8px;
  align-items: center;
}

.subtitle-review-filebar-item svg { grid-row: 1 / span 2; width: 17px; height: 17px; color: var(--text-muted); }
.subtitle-review-filebar-item b { overflow: hidden; max-width: 260px; text-overflow: ellipsis; white-space: nowrap; color: var(--text); font-size: 12px; }
.subtitle-review-filebar-item small { color: var(--text-subtle); font-size: 10px; }
.subtitle-review-filebar-spacer,
.subtitle-review-transport-spacer,
.subtitle-review-cue-head-spacer,
.subtitle-review-export-spacer { flex: 1; }

.subtitle-review-validation-summary {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-height: 28px;
  padding: 0 9px;
  border: 1px solid var(--hairline);
  border-radius: 999px;
  color: var(--text-muted);
  font-size: 11px;
  font-weight: 700;
  background: color-mix(in srgb, var(--bg) 46%, transparent);
}
.subtitle-review-validation-summary svg { width: 15px; height: 15px; }
.subtitle-review-validation-summary.error { color: #b73b31; }
.subtitle-review-validation-summary.warning { color: var(--accent-strong); }
.subtitle-review-validation-summary.success { color: #4f7654; }

.subtitle-review-warning-strip,
.subtitle-review-proxy-strip {
  min-height: 38px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  border-bottom: 1px solid var(--hairline);
  color: var(--text-muted);
  background: color-mix(in srgb, var(--accent-soft) 45%, var(--bg-surface));
  font-size: 11px;
}
.subtitle-review-warning-strip svg { width: 15px; height: 15px; flex: 0 0 auto; }
.subtitle-review-proxy-strip.error { color: #b73b31; }

.subtitle-review-inline-progress {
  width: min(260px, 28vw);
  height: 5px;
  margin-left: auto;
  overflow: hidden;
  border-radius: 99px;
  background: var(--border);
}
.subtitle-review-inline-progress span { display: block; height: 100%; background: var(--accent); transition: width 0.2s ease; }

.subtitle-review-main-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(360px, clamp(380px, 30vw, 470px));
  height: clamp(650px, calc(100dvh - var(--titlebar-h) - 222px), 920px);
  min-height: 650px;
  overflow: hidden;
}

.subtitle-review-video-column {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 16px;
  gap: 10px;
  border-right: 1px solid var(--hairline);
  background: color-mix(in srgb, var(--bg) 52%, var(--bg-surface));
}

.subtitle-review-video-frame {
  position: relative;
  width: 100%;
  min-height: 0;
  flex: 1 1 auto;
  margin: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  border-radius: 10px;
  background: #0d0d0d;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
}

.subtitle-review-video-stage {
  position: relative;
  flex: 0 0 auto;
  max-width: 100%;
  max-height: 100%;
  overflow: hidden;
  background: #0d0d0d;
  cursor: pointer;
}

.subtitle-review-video {
  position: absolute;
  inset: 0;
  z-index: 0;
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
  opacity: 0;
  background: #0d0d0d;
  pointer-events: none;
}

.subtitle-review-frame-canvas {
  position: absolute;
  inset: 0;
  z-index: 1;
  display: block;
  width: 100%;
  height: 100%;
  background: #0d0d0d;
  pointer-events: none;
}

.subtitle-review-subtitle-layer {
  position: absolute !important;
  inset: 0;
  z-index: 2;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.12s ease;
}
.subtitle-review-subtitle-layer.ready { opacity: 1; }
.subtitle-review-subtitle-layer.obscured { opacity: 0; }

.subtitle-review-subtitle-layer :deep(.subtitle-review-subtitle-canvas) {
  position: absolute !important;
  inset: 0;
  width: 100% !important;
  height: 100% !important;
  background: transparent !important;
  pointer-events: none;
}

.subtitle-review-caption-fallback {
  position: absolute;
  inset: 0;
  z-index: 3;
  overflow: hidden;
  pointer-events: none;
}
.subtitle-review-caption-fallback span {
  display: block;
}

.subtitle-review-video-loading,
.subtitle-review-video-notice {
  position: absolute;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 9px 12px;
  border-radius: 9px;
  color: #f7f2e9;
  background: rgba(18, 17, 15, 0.8);
  font-size: 11px;
  pointer-events: none;
  z-index: 4;
}
.subtitle-review-video-loading svg { width: 17px; height: 17px; }

.subtitle-review-transport {
  min-height: 42px;
  flex-wrap: wrap;
  gap: 7px;
}
.subtitle-review-time-readout { color: var(--text-muted); font-family: Consolas, 'SFMono-Regular', monospace; font-size: 11px; }
.subtitle-review-time-readout i { margin: 0 5px; color: var(--text-subtle); font-style: normal; }
.settings-action.compact { min-height: 31px; padding: 5px 9px; font-size: 11px; }

.subtitle-review-timeline {
  flex: 0 0 auto;
  width: 100%;
  overflow: hidden;
  border: 1px solid var(--hairline);
  border-radius: 9px;
}

.subtitle-review-cue-pane {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: color-mix(in srgb, var(--bg-surface) 92%, var(--bg));
}

.subtitle-review-cue-toolbar {
  min-height: 49px;
  gap: 12px;
  padding: 8px 10px 8px 14px;
  border-bottom: 1px solid var(--hairline);
}
.subtitle-review-cue-toolbar > span { display: flex; flex-direction: column; gap: 2px; }
.subtitle-review-cue-toolbar b { color: var(--text); font-size: 12px; }
.subtitle-review-cue-toolbar small { color: var(--text-subtle); font-size: 10px; }
.subtitle-review-cue-toolbar .settings-action { margin-left: auto; }

.subtitle-review-cue-list {
  min-height: 0;
  overflow: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--border) transparent;
}

.subtitle-review-cue-row {
  padding: 10px 11px 11px;
  border-bottom: 1px solid var(--hairline);
  transition: background 0.12s ease, box-shadow 0.12s ease;
}
.subtitle-review-cue-row:hover { background: var(--bg-surface-hover); }
.subtitle-review-cue-row.active { background: color-mix(in srgb, var(--accent-soft) 35%, var(--bg-surface)); }
.subtitle-review-cue-row.selected { box-shadow: inset 3px 0 0 var(--accent); background: color-mix(in srgb, var(--accent-soft) 55%, var(--bg-surface)); }
.subtitle-review-cue-row.invalid { box-shadow: inset 3px 0 0 #b73b31; }
.subtitle-review-cue-row.bilingual { contain-intrinsic-size: 250px; }

.subtitle-review-cue-row-head { min-height: 25px; display: flex; align-items: center; gap: 6px; }
.subtitle-review-cue-index {
  min-width: 25px;
  height: 23px;
  border: 0;
  border-radius: 7px;
  color: var(--text-muted);
  background: color-mix(in srgb, var(--bg) 55%, var(--bg-surface));
  font-size: 10px;
  font-weight: 800;
  cursor: pointer;
}
.subtitle-review-cue-tag,
.subtitle-review-cue-style {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-subtle);
  font-size: 9px;
}
.subtitle-review-cue-tag svg { width: 12px; height: 12px; }
.subtitle-review-bilingual-tag {
  padding: 2px 6px;
  border-radius: 999px;
  color: var(--accent-strong);
  background: var(--accent-soft);
  font-size: 9px;
  font-weight: 750;
}
.subtitle-review-cue-delete {
  width: 25px;
  height: 25px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 7px;
  color: var(--text-subtle);
  background: transparent;
  cursor: pointer;
}
.subtitle-review-cue-delete:hover:not(:disabled) { color: #b73b31; background: rgba(183, 59, 49, 0.1); }
.subtitle-review-cue-delete svg { width: 14px; height: 14px; }

.subtitle-review-time-fields { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 7px; margin-top: 7px; }
.subtitle-review-time-fields label { min-width: 0; display: flex; align-items: center; gap: 5px; }
.subtitle-review-time-fields span { color: var(--text-subtle); font-size: 9px; }
.subtitle-review-time-fields input {
  min-width: 0;
  width: 100%;
  height: 28px;
  padding: 0 7px;
  border: 1px solid var(--border);
  border-radius: 7px;
  outline: none;
  color: var(--text);
  background: color-mix(in srgb, var(--bg) 55%, var(--bg-surface));
  font-family: Consolas, 'SFMono-Regular', monospace;
  font-size: 10px;
}
.subtitle-review-time-fields input:focus { border-color: var(--accent); }

.subtitle-review-language-block { margin-top: 8px; }
.subtitle-review-language-block + .subtitle-review-language-block {
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px dashed var(--hairline);
}
.subtitle-review-language-head { min-height: 23px; display: flex; align-items: center; gap: 6px; }
.subtitle-review-language-role { color: var(--text-muted); font-size: 10px; font-weight: 750; }
.subtitle-review-language-spacer { flex: 1; }
.subtitle-review-language-head .subtitle-review-text-mode { margin-top: 0; }

.subtitle-review-text-mode { display: inline-flex; margin-top: 7px; padding: 2px; border-radius: 8px; background: color-mix(in srgb, var(--bg) 55%, var(--bg-surface)); }
.subtitle-review-text-mode button { min-height: 23px; padding: 3px 8px; border: 0; border-radius: 6px; color: var(--text-subtle); background: transparent; font-size: 9px; cursor: pointer; }
.subtitle-review-text-mode button.active { color: var(--text); background: var(--bg-surface-hover); font-weight: 700; }

.subtitle-review-cue-text {
  width: 100%;
  min-height: 62px;
  margin-top: 7px;
  padding: 8px 9px;
  resize: vertical;
  border: 1px solid var(--border);
  border-radius: 9px;
  outline: none;
  color: var(--text);
  background: color-mix(in srgb, var(--bg) 55%, var(--bg-surface));
  font-size: 12px;
  line-height: 1.5;
}
.subtitle-review-cue-text.raw { font-family: Consolas, 'SFMono-Regular', monospace; font-size: 10px; }
.subtitle-review-cue-text:focus { border-color: var(--accent); box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 15%, transparent); }

.subtitle-review-cue-issues { display: flex; flex-wrap: wrap; gap: 5px; margin-top: 6px; }
.subtitle-review-cue-issues span { padding: 2px 5px; border-radius: 5px; color: var(--accent-strong); background: var(--accent-soft); font-size: 9px; }
.subtitle-review-cue-issues span.error { color: #a7352d; background: rgba(183, 59, 49, 0.1); }

.subtitle-review-export-bar {
  min-height: 54px;
  gap: 12px;
  padding: 8px 12px 8px 14px;
  border-top: 1px solid var(--hairline);
}
.subtitle-review-export-progress { width: min(360px, 35vw); margin: 0; }

.subtitle-review-alert-close { margin-left: auto; width: 26px; height: 26px; display: inline-flex; align-items: center; justify-content: center; border: 0; border-radius: 7px; color: inherit; background: transparent; cursor: pointer; }
.subtitle-review-alert-close svg { width: 14px; height: 14px; }

.subtitle-review-confirm-dialog { max-width: 430px; }
.subtitle-review-confirm-dialog p { margin-top: 12px; color: var(--text-muted); font-size: 13px; line-height: 1.7; }
.subtitle-review-dialog-actions { display: flex; justify-content: flex-end; gap: 9px; margin-top: 22px; }
.subtitle-review-danger-action { color: #fff; border-color: #a7352d; background: #a7352d; }

.spin { animation: subtitle-review-spin 0.9s linear infinite; }
@keyframes subtitle-review-spin { to { transform: rotate(360deg); } }

@media (max-width: 1120px) {
  .subtitle-review-main-grid {
    grid-template-columns: minmax(0, 1fr) minmax(330px, 390px);
    height: clamp(620px, calc(100dvh - var(--titlebar-h) - 210px), 860px);
    min-height: 620px;
  }
  .subtitle-review-transport .settings-action span { display: none; }
}

@media (max-width: 860px) {
  .subtitle-review-header { align-items: flex-start; }
  .subtitle-review-title-group { min-height: 40px; }
  .subtitle-review-header-actions { width: 100%; flex-wrap: wrap; margin-left: 0; }
  .subtitle-review-import-grid { grid-template-columns: 1fr; }
  .subtitle-review-main-grid { display: flex; flex-direction: column; height: auto; min-height: 0; }
  .subtitle-review-video-column { border-right: 0; border-bottom: 1px solid var(--hairline); }
  .subtitle-review-video-frame { min-height: 320px; }
  .subtitle-review-cue-pane { min-height: 430px; max-height: 58vh; }
  .subtitle-review-filebar { align-items: flex-start; flex-wrap: wrap; }
  .subtitle-review-filebar-spacer { display: none; }
  .subtitle-review-validation-summary { width: 100%; }
  .subtitle-review-export-bar { align-items: flex-start; flex-wrap: wrap; }
  .subtitle-review-export-progress { width: 100%; order: 3; }
}
</style>
