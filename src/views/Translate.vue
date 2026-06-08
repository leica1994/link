<template>
  <div class="page translate-page">
    <header class="translate-header">
      <div>
        <h1 class="page-title">翻译</h1>
      </div>

      <div class="translate-tabs" role="tablist" aria-label="翻译工作台">
        <button
          v-for="tab in translateTabs"
          :id="`${tab.value}-tab`"
          :key="tab.value"
          class="translate-tab"
          :class="{ active: activeTab === tab.value }"
          type="button"
          role="tab"
          :aria-selected="activeTab === tab.value"
          :aria-controls="`${tab.value}-panel`"
          @click="selectTab(tab.value)"
        >
          <component :is="tab.icon" :stroke-width="2.1" aria-hidden="true" />
          <span>{{ tab.label }}</span>
        </button>
      </div>
    </header>

    <main class="translate-workspace">
      <section
        v-if="activeTab === TranslateTab.Transcription"
        :id="`${TranslateTab.Transcription}-panel`"
        class="translate-panel"
        role="tabpanel"
        :aria-labelledby="`${TranslateTab.Transcription}-tab`"
      >
        <div class="translate-grid">
          <section class="settings-section" aria-labelledby="transcription-input-title">
            <div id="transcription-input-title" class="section-heading">
              <Video aria-hidden="true" />
              <span>视频输入</span>
            </div>

            <div class="settings-panel translate-drop-panel">
              <div
                ref="videoDropZoneRef"
                class="translate-drop-zone"
                :class="{ 'drag-active': dragTarget === FileInputTarget.Video }"
                @dragenter.prevent="dragTarget = FileInputTarget.Video"
                @dragover.prevent
                @dragleave.prevent="clearNativeDragTarget(FileInputTarget.Video)"
                @drop.prevent="handleBrowserDrop(FileInputTarget.Video, $event)"
              >
                <UploadCloud class="translate-drop-icon" :stroke-width="2.1" aria-hidden="true" />
                <div class="translate-drop-copy">
                  <span class="translate-drop-title">选择或拖入需要转录的视频</span>
                  <span class="translate-drop-subtitle">支持本地视频和音频文件，转录完成后会自动保存字幕</span>
                </div>
                <button class="settings-action" type="button" :disabled="isTranscribing" @click="selectVideoFile">
                  选择视频
                </button>
              </div>

              <div class="translate-file-strip" aria-label="当前视频">
                <FileVideo :stroke-width="2.1" aria-hidden="true" />
                <span>{{ selectedVideoName }}</span>
              </div>
            </div>
          </section>

          <section class="settings-section" aria-labelledby="transcription-options-title">
            <div id="transcription-options-title" class="section-heading">
              <SlidersHorizontal aria-hidden="true" />
              <span>转录参数</span>
            </div>

            <div class="settings-panel">
              <button class="setting-row setting-row-button" type="button" @click="openDialog(TranslateDialog.TranscriptionModel)">
                <Bot class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="setting-copy">
                  <span class="setting-title">转录模型</span>
                  <span class="setting-subtitle">选择用于语音识别的模型</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ transcriptionModelLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>

              <button class="setting-row setting-row-button" type="button" @click="openDialog(TranslateDialog.SourceLanguage)">
                <Languages class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="setting-copy">
                  <span class="setting-title">源语言</span>
                  <span class="setting-subtitle">视频语音语言</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ sourceLanguageLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>

              <button class="setting-row setting-row-button" type="button" @click="openDialog(TranslateDialog.TranscriptionFormat)">
                <Captions class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="setting-copy">
                  <span class="setting-title">输出格式</span>
                  <span class="setting-subtitle">转录后生成的字幕格式</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ transcriptionFormatLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>

              <div class="setting-row">
                <Scissors class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <div class="setting-copy">
                  <div class="setting-title">智能断句</div>
                  <div class="setting-subtitle">开启后转录完成会用 AI 优化字幕断句</div>
                </div>
                <button
                  class="setting-toggle"
                  :class="{ active: isSmartSegmentationEnabled }"
                  type="button"
                  :aria-pressed="isSmartSegmentationEnabled"
                  @click="isSmartSegmentationEnabled = !isSmartSegmentationEnabled"
                >
                  <span class="setting-toggle-label">{{ isSmartSegmentationEnabled ? '开' : '关' }}</span>
                  <span class="setting-toggle-track" aria-hidden="true">
                    <span class="setting-toggle-thumb" />
                  </span>
                </button>
              </div>
            </div>
          </section>
        </div>

        <section class="settings-section translate-output-section" aria-labelledby="transcription-output-title">
          <div id="transcription-output-title" class="section-heading">
            <ListVideo aria-hidden="true" />
            <span>转录结果</span>
          </div>

          <div class="settings-panel translate-result-panel">
            <div class="translate-status-bar">
              <div class="translate-status">
                <span class="translate-status-dot" :class="transcriptionStatusClass" aria-hidden="true" />
                <span>{{ transcriptionStatusText }}</span>
              </div>
              <div class="translate-actions">
                <button
                  class="settings-action"
                  type="button"
                  :disabled="!canStartTranscription"
                  @click="startTranscription"
                >
                  {{ isTranscribing ? '转录中' : '开始转录' }}
                </button>
                <button
                  class="settings-action"
                  type="button"
                  :disabled="!canExportTranscription"
                  @click="exportTranscription"
                >
                  导出字幕
                </button>
              </div>
            </div>

            <div v-if="currentTranscriptionStage" class="translate-progress" aria-label="处理进度">
              <div class="translate-progress-track">
                <span
                  class="translate-progress-bar"
                  :style="{ width: `${currentTranscriptionStage.progress}%` }"
                />
              </div>
              <span class="translate-progress-value">{{ currentTranscriptionStage.progress }}%</span>
            </div>

            <div v-if="transcriptionError" class="translate-alert" role="alert">
              <CircleAlert :stroke-width="2.1" aria-hidden="true" />
              <span>{{ transcriptionError }}</span>
            </div>

            <div v-if="transcriptionSegments.length > 0" class="translate-preview translate-subtitle-list">
              <article
                v-for="(segment, index) in transcriptionSegments"
                :key="segment.uid || `${segment.startTime}-${index}`"
                class="translate-subtitle-row"
              >
                <span class="translate-subtitle-index">{{ index + 1 }}</span>
                <span
                  class="translate-subtitle-status"
                  :class="segment.status ? `status-${segment.status}` : 'status-raw'"
                >
                  {{ transcriptionSegmentStatusLabel(segment.status) }}
                </span>
                <span class="translate-subtitle-time translate-subtitle-start">{{ formatSegmentTime(segment.startTime) }}</span>
                <span class="translate-subtitle-time translate-subtitle-end">{{ formatSegmentTime(segment.endTime) }}</span>
                <p>{{ segment.text }}</p>
              </article>
            </div>

            <div v-else class="translate-preview translate-preview-empty">
              <Captions class="translate-empty-icon" :stroke-width="2.1" aria-hidden="true" />
              <span class="translate-empty-title">暂无转录内容</span>
              <span class="translate-empty-subtitle">选择视频并开始转录后，字幕内容会显示在这里</span>
            </div>
          </div>
        </section>
      </section>

      <section
        v-else
        :id="`${TranslateTab.TranslationOptimization}-panel`"
        class="translate-panel"
        role="tabpanel"
        :aria-labelledby="`${TranslateTab.TranslationOptimization}-tab`"
      >
        <div class="translate-grid">
          <section class="settings-section" aria-labelledby="subtitle-input-title">
            <div id="subtitle-input-title" class="section-heading">
              <FileText aria-hidden="true" />
              <span>字幕输入</span>
            </div>

            <div class="settings-panel translate-drop-panel">
              <div
                ref="subtitleDropZoneRef"
                class="translate-drop-zone"
                :class="{ 'drag-active': dragTarget === FileInputTarget.Subtitle }"
                @dragenter.prevent="dragTarget = FileInputTarget.Subtitle"
                @dragover.prevent
                @dragleave.prevent="clearNativeDragTarget(FileInputTarget.Subtitle)"
                @drop.prevent="handleBrowserDrop(FileInputTarget.Subtitle, $event)"
              >
                <UploadCloud class="translate-drop-icon" :stroke-width="2.1" aria-hidden="true" />
                <div class="translate-drop-copy">
                  <span class="translate-drop-title">选择或拖入转录后的字幕</span>
                  <span class="translate-drop-subtitle">支持 SRT、VTT、ASS，可直接导入已有字幕继续处理</span>
                </div>
                <button class="settings-action" type="button" @click="selectSubtitleFile">选择字幕</button>
              </div>

              <div class="translate-file-strip" aria-label="当前字幕">
                <FileText :stroke-width="2.1" aria-hidden="true" />
                <span>{{ selectedSubtitleName }}</span>
              </div>
            </div>
          </section>

          <section class="settings-section" aria-labelledby="translation-options-title">
            <div id="translation-options-title" class="section-heading">
              <WandSparkles aria-hidden="true" />
              <span>翻译与优化参数</span>
            </div>

            <div class="settings-panel">
              <button class="setting-row setting-row-button" type="button" @click="openDialog(TranslateDialog.VideoContentType)">
                <Film class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="setting-copy">
                  <span class="setting-title">视频类型</span>
                  <span class="setting-subtitle">选择视频内容类型</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ videoContentTypeLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>

              <button class="setting-row setting-row-button" type="button" @click="openDialog(TranslateDialog.TargetLanguage)">
                <Languages class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="setting-copy">
                  <span class="setting-title">目标语言</span>
                  <span class="setting-subtitle">翻译字幕的目标语言</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ targetLanguageLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>

              <button class="setting-row setting-row-button" type="button" @click="openDialog(TranslateDialog.OutputMode)">
                <PanelTop class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="setting-copy">
                  <span class="setting-title">输出模式</span>
                  <span class="setting-subtitle">选择最终字幕的呈现方式</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ outputModeLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>

              <button class="setting-row setting-row-button" type="button" @click="openDialog(TranslateDialog.TranslationFormat)">
                <Captions class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="setting-copy">
                  <span class="setting-title">输出格式</span>
                  <span class="setting-subtitle">处理后导出的字幕格式</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ translationFormatLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>
            </div>
          </section>
        </div>

        <section class="settings-section translate-output-section" aria-labelledby="translation-output-title">
          <div id="translation-output-title" class="section-heading">
            <Columns2 aria-hidden="true" />
            <span>处理结果</span>
          </div>

          <div class="settings-panel translate-result-panel">
            <div class="translate-status-bar">
              <div class="translate-status">
                <span class="translate-status-dot" :class="translationStatusClass" aria-hidden="true" />
                <span>{{ translationStatusText }}</span>
              </div>
              <div class="translate-actions">
                <button
                  class="settings-action"
                  type="button"
                  :disabled="!canStartTranslationProcessing"
                  @click="startTranslationProcessing"
                >
                  开始处理
                </button>
                <button class="settings-action" type="button" disabled>导出结果</button>
              </div>
            </div>

            <div v-if="subtitleInputError" class="translate-alert" role="alert">
              <CircleAlert :stroke-width="2.1" aria-hidden="true" />
              <span>{{ subtitleInputError }}</span>
            </div>

            <div class="translate-compare">
              <div class="translate-preview translate-preview-empty">
                <FileText class="translate-empty-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="translate-empty-title">原文字幕</span>
                <span class="translate-empty-subtitle">导入字幕后显示原文内容</span>
              </div>
              <div class="translate-preview translate-preview-empty">
                <Languages class="translate-empty-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="translate-empty-title">译文字幕</span>
                <span class="translate-empty-subtitle">完成翻译与优化后显示结果</span>
              </div>
            </div>
          </div>
        </section>
      </section>
    </main>

    <Teleport to="body">
      <div v-if="activeDialog" class="dialog-backdrop" role="presentation" @click.self="closeDialog">
        <section
          class="settings-dialog"
          :class="{ 'language-dialog': isLanguageDialog }"
          role="dialog"
          aria-modal="true"
          :aria-labelledby="`${activeDialog}-dialog-title`"
        >
          <h2 :id="`${activeDialog}-dialog-title`" class="dialog-title">{{ activeDialogTitle }}</h2>
          <label v-if="isLanguageDialog" class="language-search-field">
            <Search class="language-search-icon" :stroke-width="2.1" aria-hidden="true" />
            <input
              v-model="languageSearch"
              class="settings-input language-search-input"
              type="search"
              placeholder="搜索语言"
              :aria-label="`搜索${activeDialogTitle}`"
            />
          </label>
          <div
            class="dialog-options"
            :class="{ 'language-options': isLanguageDialog }"
            role="radiogroup"
            :aria-label="activeDialogTitle"
          >
            <button
              v-for="option in filteredActiveDialogOptions"
              :key="option.value"
              class="dialog-option"
              :class="[{ active: activeDialogValue === option.value }, { 'language-option': isLanguageDialog }]"
              type="button"
              role="radio"
              :aria-checked="activeDialogValue === option.value"
              @click="selectDialogValue(option.value)"
            >
              <span class="dialog-radio" aria-hidden="true" />
              <span :class="{ 'language-option-label': isLanguageDialog }">{{ option.label }}</span>
            </button>
            <span v-if="filteredActiveDialogOptions.length === 0" class="language-empty">未找到语言</span>
          </div>
        </section>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { DragDropEvent } from '@tauri-apps/api/webview'
import { open, save } from '@tauri-apps/plugin-dialog'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  Bot,
  Captions,
  ChevronRight,
  CircleAlert,
  Columns2,
  Film,
  FileText,
  FileVideo,
  Languages,
  ListVideo,
  PanelTop,
  Scissors,
  Search,
  SlidersHorizontal,
  UploadCloud,
  Video,
  WandSparkles,
} from 'lucide-vue-next'
import {
  type AppSettings,
  OutputMode,
  SubtitleFormat,
  TranscriptionModel,
  VideoContentType,
  getOptionLabel,
  normalizeSettings,
  outputModeOptions,
  sourceLanguageOptions,
  subtitleFormatOptions,
  targetLanguageOptions,
  transcriptionModelOptions,
  videoContentTypeOptions,
} from '../settingsModel'

enum TranslateTab {
  Transcription = 'transcription',
  TranslationOptimization = 'translation-optimization',
}

enum TranslateDialog {
  TranscriptionModel = 'transcription-model',
  SourceLanguage = 'source-language',
  TranscriptionFormat = 'transcription-format',
  VideoContentType = 'video-content-type',
  TargetLanguage = 'target-language',
  OutputMode = 'output-mode',
  TranslationFormat = 'translation-format',
}

enum FileInputTarget {
  Video = 'video',
  Subtitle = 'subtitle',
}

type DialogOption = {
  value: string
  label: string
}

type TranscriptionSegmentStatus = 'raw' | 'segmenting' | 'segmented' | 'correcting' | 'corrected' | 'kept' | 'done'

type TranscriptionSegment = {
  uid?: string
  text: string
  startTime: number
  endTime: number
  status?: TranscriptionSegmentStatus
}

type TranscriptionStageStatus = 'pending' | 'active' | 'done' | 'failed'

type TranscriptionProgressStage = {
  progress: number
  message: string
  status: TranscriptionStageStatus
}

type TranscriptionStageProgress = {
  transcription?: TranscriptionProgressStage
  smartSegmentation?: TranscriptionProgressStage
  subtitleCorrection?: TranscriptionProgressStage
}

type TranscriptionResult = {
  segments: TranscriptionSegment[]
  subtitleText: string
  outputPath: string
  outputFormat: string
  logPath: string
  warnings: string[]
}

type TranscriptionProgress = {
  progress: number
  message: string
  stageProgress?: TranscriptionStageProgress
  revision?: number
  segments?: TranscriptionSegment[]
  warnings?: string[]
}

const transcriptionSegmentStatusLabels: Record<TranscriptionSegmentStatus, string> = {
  raw: '原始',
  segmenting: '断句中',
  segmented: '已断句',
  correcting: '校正中',
  corrected: '已校正',
  kept: '保留原文',
  done: '完成',
}

const translateTabs = [
  { value: TranslateTab.Transcription, label: '转录', icon: Captions },
  { value: TranslateTab.TranslationOptimization, label: '翻译与优化', icon: WandSparkles },
] as const

const activeTab = ref<TranslateTab>(TranslateTab.Transcription)
const activeDialog = ref<TranslateDialog | null>(null)
const currentSettings = ref<AppSettings>(normalizeSettings({}))
const selectedTranscriptionModel = ref<TranscriptionModel>(TranscriptionModel.Bilibili)
const selectedSourceLanguage = ref('auto')
const selectedTranscriptionFormat = ref<SubtitleFormat>(SubtitleFormat.Srt)
const selectedTranslationFormat = ref<SubtitleFormat>(SubtitleFormat.Srt)
const selectedVideoContentType = ref<VideoContentType>(VideoContentType.General)
const selectedTargetLanguage = ref('zh-Hans')
const selectedOutputMode = ref<OutputMode>(OutputMode.Bilingual)
const isSmartSegmentationEnabled = ref(true)
const isSettingsLoaded = ref(false)
const languageSearch = ref('')
const videoDropZoneRef = ref<HTMLElement | null>(null)
const subtitleDropZoneRef = ref<HTMLElement | null>(null)
const dragTarget = ref<FileInputTarget | null>(null)
const selectedVideoPath = ref('')
const selectedSubtitlePath = ref('')
const isTranscribing = ref(false)
const transcriptionProgress = ref(0)
const transcriptionStageProgress = ref<TranscriptionStageProgress>({})
const transcriptionMessage = ref('等待选择视频')
const transcriptionError = ref('')
const transcriptionWarnings = ref<string[]>([])
const transcriptionSegments = ref<TranscriptionSegment[]>([])
const transcriptionText = ref('')
const lastTranscriptionRevision = ref(0)
const lastOutputPath = ref('')
const subtitleInputError = ref('')
const translationMessage = ref('等待选择字幕')
let isApplyingStoredSettings = false
let hasLoadedOnce = false
let saveSettingsTimer: ReturnType<typeof window.setTimeout> | undefined
let unlistenTranscriptionProgress: UnlistenFn | undefined
let unlistenDragDrop: UnlistenFn | undefined

const isTauriRuntime = () => '__TAURI_INTERNALS__' in window
const mediaExtensions = ['mp4', 'mov', 'mkv', 'avi', 'flv', 'wmv', 'webm', 'm4v', 'mp3', 'wav', 'm4a', 'flac', 'aac', 'ogg']
const subtitleExtensions = ['srt', 'vtt', 'ass']

const transcriptionModelLabel = computed(() =>
  getOptionLabel(transcriptionModelOptions, selectedTranscriptionModel.value),
)
const sourceLanguageLabel = computed(() => getOptionLabel(sourceLanguageOptions, selectedSourceLanguage.value))
const transcriptionFormatLabel = computed(() =>
  getOptionLabel(subtitleFormatOptions, selectedTranscriptionFormat.value),
)
const translationFormatLabel = computed(() =>
  getOptionLabel(subtitleFormatOptions, selectedTranslationFormat.value),
)
const videoContentTypeLabel = computed(() => getOptionLabel(videoContentTypeOptions, selectedVideoContentType.value))
const targetLanguageLabel = computed(() => getOptionLabel(targetLanguageOptions, selectedTargetLanguage.value))
const outputModeLabel = computed(() => getOptionLabel(outputModeOptions, selectedOutputMode.value))
const selectedVideoName = computed(() => {
  return selectedVideoPath.value ? fileNameFromPath(selectedVideoPath.value) : '尚未选择视频'
})
const activeSubtitlePath = computed(() => selectedSubtitlePath.value || lastOutputPath.value)
const selectedSubtitleName = computed(() => {
  return activeSubtitlePath.value ? fileNameFromPath(activeSubtitlePath.value) : '尚未选择字幕'
})
const canStartTranscription = computed(() => {
  return Boolean(selectedVideoPath.value) && !isTranscribing.value
})
const canExportTranscription = computed(() => {
  return Boolean(transcriptionText.value) && !isTranscribing.value
})
const canStartTranslationProcessing = computed(() => Boolean(activeSubtitlePath.value) && !subtitleInputError.value)
const transcriptionStatusText = computed(() => {
  if (transcriptionError.value) {
    return '转录失败'
  }

  if (isTranscribing.value && currentTranscriptionStage.value) {
    return currentTranscriptionStage.value.message
  }

  return transcriptionMessage.value
})
const transcriptionStatusClass = computed(() => ({
  active: isTranscribing.value,
  success: !isTranscribing.value && transcriptionSegments.value.length > 0 && !transcriptionError.value,
  error: Boolean(transcriptionError.value),
}))
const visibleTranscriptionStages = computed(() => {
  const stages = transcriptionStageProgress.value
  return [
    { key: 'transcription', label: '转录', stage: stages.transcription },
    { key: 'smart-segmentation', label: '智能断句', stage: stages.smartSegmentation },
    { key: 'subtitle-correction', label: '字幕校正', stage: stages.subtitleCorrection },
  ]
    .filter((item): item is { key: string; label: string; stage: TranscriptionProgressStage } => Boolean(item.stage))
    .map((item) => ({
      key: item.key,
      label: item.label,
      progress: clampProgress(item.stage.progress),
      status: item.stage.status,
      message: item.stage.message,
    }))
})
const currentTranscriptionStage = computed(() => {
  const stages = visibleTranscriptionStages.value
  return (
    [...stages].reverse().find((stage) => stage.status === 'active') ??
    [...stages].reverse().find((stage) => stage.status === 'pending') ??
    [...stages].reverse().find((stage) => stage.status === 'failed') ??
    [...stages].reverse().find((stage) => stage.status === 'done') ??
    null
  )
})
const translationStatusText = computed(() => {
  if (subtitleInputError.value) {
    return '字幕导入失败'
  }

  if (activeSubtitlePath.value && translationMessage.value === '等待选择字幕') {
    return '已选择字幕'
  }

  return translationMessage.value
})
const translationStatusClass = computed(() => ({
  success: Boolean(activeSubtitlePath.value) && !subtitleInputError.value,
  error: Boolean(subtitleInputError.value),
}))
const isLanguageDialog = computed(() => {
  return activeDialog.value === TranslateDialog.SourceLanguage || activeDialog.value === TranslateDialog.TargetLanguage
})

const activeDialogTitle = computed(() => {
  switch (activeDialog.value) {
    case TranslateDialog.TranscriptionModel:
      return '转录模型'
    case TranslateDialog.SourceLanguage:
      return '源语言'
    case TranslateDialog.TranscriptionFormat:
      return '输出格式'
    case TranslateDialog.VideoContentType:
      return '视频类型'
    case TranslateDialog.TargetLanguage:
      return '目标语言'
    case TranslateDialog.OutputMode:
      return '输出模式'
    case TranslateDialog.TranslationFormat:
      return '输出格式'
    default:
      return ''
  }
})

const activeDialogOptions = computed<readonly DialogOption[]>(() => {
  switch (activeDialog.value) {
    case TranslateDialog.TranscriptionModel:
      return transcriptionModelOptions
    case TranslateDialog.SourceLanguage:
      return sourceLanguageOptions
    case TranslateDialog.TranscriptionFormat:
      return subtitleFormatOptions
    case TranslateDialog.VideoContentType:
      return videoContentTypeOptions
    case TranslateDialog.TargetLanguage:
      return targetLanguageOptions
    case TranslateDialog.OutputMode:
      return outputModeOptions
    case TranslateDialog.TranslationFormat:
      return subtitleFormatOptions
    default:
      return []
  }
})

const filteredActiveDialogOptions = computed<readonly DialogOption[]>(() => {
  const options = activeDialogOptions.value
  const query = languageSearch.value.trim().toLowerCase()

  if (!isLanguageDialog.value || !query) {
    return options
  }

  return options.filter((option) => {
    return option.label.toLowerCase().includes(query) || option.value.toLowerCase().includes(query)
  })
})

const activeDialogValue = computed<string>(() => {
  switch (activeDialog.value) {
    case TranslateDialog.TranscriptionModel:
      return selectedTranscriptionModel.value
    case TranslateDialog.SourceLanguage:
      return selectedSourceLanguage.value
    case TranslateDialog.TranscriptionFormat:
      return selectedTranscriptionFormat.value
    case TranslateDialog.VideoContentType:
      return selectedVideoContentType.value
    case TranslateDialog.TargetLanguage:
      return selectedTargetLanguage.value
    case TranslateDialog.OutputMode:
      return selectedOutputMode.value
    case TranslateDialog.TranslationFormat:
      return selectedTranslationFormat.value
    default:
      return ''
  }
})

const createSettingsSnapshot = (): AppSettings => ({
  ...currentSettings.value,
  transcriptionModel: selectedTranscriptionModel.value,
  sourceLanguage: selectedSourceLanguage.value,
  transcriptionFormat: selectedTranscriptionFormat.value,
  translationFormat: selectedTranslationFormat.value,
  isSmartSegmentationEnabled: isSmartSegmentationEnabled.value,
  videoContentType: selectedVideoContentType.value,
  outputMode: selectedOutputMode.value,
  targetLanguage: selectedTargetLanguage.value,
})

const applySettings = (settings: AppSettings) => {
  isApplyingStoredSettings = true

  currentSettings.value = settings
  selectedTranscriptionModel.value = settings.transcriptionModel
  selectedSourceLanguage.value = settings.sourceLanguage
  selectedTranscriptionFormat.value = settings.transcriptionFormat
  selectedTranslationFormat.value = settings.translationFormat
  isSmartSegmentationEnabled.value = settings.isSmartSegmentationEnabled
  selectedVideoContentType.value = settings.videoContentType
  selectedOutputMode.value = settings.outputMode
  selectedTargetLanguage.value = settings.targetLanguage

  nextTick(() => {
    isApplyingStoredSettings = false
  })
}

const saveSettingsNow = async () => {
  if (!isSettingsLoaded.value || isApplyingStoredSettings) {
    return
  }

  const settings = createSettingsSnapshot()

  if (!isTauriRuntime()) {
    return
  }

  try {
    await invoke('save_settings', { settings })
  } catch (error) {
    console.error('保存翻译参数失败', error)
  }
}

const scheduleSaveSettings = () => {
  if (!isSettingsLoaded.value || isApplyingStoredSettings) {
    return
  }

  if (saveSettingsTimer !== undefined) {
    window.clearTimeout(saveSettingsTimer)
  }

  saveSettingsTimer = window.setTimeout(() => {
    saveSettingsTimer = undefined
    void saveSettingsNow()
  }, 260)
}

const flushPendingSave = async () => {
  if (saveSettingsTimer !== undefined) {
    window.clearTimeout(saveSettingsTimer)
    saveSettingsTimer = undefined
    await saveSettingsNow()
  }
}

const loadStoredSettings = async () => {
  const shouldPersistDefaults = !hasLoadedOnce

  if (!isTauriRuntime()) {
    applySettings(normalizeSettings({}))
    await nextTick()
    isSettingsLoaded.value = true
    hasLoadedOnce = true
    return
  }

  try {
    const storedSettings = await invoke<Partial<AppSettings>>('load_settings')
    applySettings(normalizeSettings(storedSettings))
  } catch (error) {
    console.error('加载翻译参数失败', error)
  } finally {
    await nextTick()
    isSettingsLoaded.value = true

    if (shouldPersistDefaults) {
      hasLoadedOnce = true
      void saveSettingsNow()
    }
  }
}

const selectTab = async (tab: TranslateTab) => {
  if (activeTab.value === tab) {
    return
  }

  await flushPendingSave()
  activeTab.value = tab
  void loadStoredSettings()
}

const selectVideoFile = async () => {
  if (!isTauriRuntime()) {
    transcriptionError.value = '请在桌面应用中选择视频文件'
    return
  }

  try {
    const selected = await open({
      title: '选择需要转录的视频',
      multiple: false,
      filters: [
        {
          name: '媒体文件',
          extensions: ['mp4', 'mov', 'mkv', 'avi', 'flv', 'wmv', 'webm', 'm4v', 'mp3', 'wav', 'm4a', 'flac', 'aac', 'ogg'],
        },
        {
          name: '视频文件',
          extensions: ['mp4', 'mov', 'mkv', 'avi', 'flv', 'wmv', 'webm', 'm4v'],
        },
        {
          name: '音频文件',
          extensions: ['mp3', 'wav', 'm4a', 'flac', 'aac', 'ogg'],
        },
      ],
    })

    if (typeof selected !== 'string') {
      return
    }

    applyVideoFile(selected)
  } catch (error) {
    transcriptionError.value = stringifyError(error)
  }
}

const selectSubtitleFile = async () => {
  if (!isTauriRuntime()) {
    subtitleInputError.value = '请在桌面应用中选择字幕文件'
    return
  }

  try {
    const selected = await open({
      title: '选择需要处理的字幕',
      multiple: false,
      filters: [
        {
          name: '字幕文件',
          extensions: subtitleExtensions,
        },
      ],
    })

    if (typeof selected !== 'string') {
      return
    }

    applySubtitleFile(selected)
  } catch (error) {
    subtitleInputError.value = stringifyError(error)
  }
}

const applyVideoFile = (path: string) => {
  const extension = fileExtension(path)
  if (!mediaExtensions.includes(extension)) {
    transcriptionError.value = '请选择支持的视频或音频文件'
    return
  }

  selectedVideoPath.value = path
  transcriptionError.value = ''
  transcriptionProgress.value = 0
  transcriptionStageProgress.value = {}
  transcriptionMessage.value = '已选择视频'
  transcriptionWarnings.value = []
  transcriptionSegments.value = []
  transcriptionText.value = ''
  lastTranscriptionRevision.value = Number.MAX_SAFE_INTEGER
  lastOutputPath.value = ''
}

const applySubtitleFile = (path: string) => {
  const extension = fileExtension(path)
  if (!subtitleExtensions.includes(extension)) {
    subtitleInputError.value = '请选择 SRT、VTT 或 ASS 字幕文件'
    return
  }

  selectedSubtitlePath.value = path
  subtitleInputError.value = ''
  translationMessage.value = '已选择字幕'
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
    if (isTranscribing.value) {
      return
    }

    applyVideoFile(path)
    return
  }

  applySubtitleFile(path)
}

const clearNativeDragTarget = (target: FileInputTarget) => {
  if (dragTarget.value === target) {
    dragTarget.value = null
  }
}

const clampProgress = (value: number) => Math.min(Math.max(Math.round(value), 0), 100)

const markStageProgressDone = (stages: TranscriptionStageProgress): TranscriptionStageProgress => ({
  transcription: stages.transcription ? { ...stages.transcription, progress: 100, status: 'done' } : undefined,
  smartSegmentation: stages.smartSegmentation
    ? { ...stages.smartSegmentation, progress: 100, status: 'done' }
    : undefined,
  subtitleCorrection: stages.subtitleCorrection
    ? { ...stages.subtitleCorrection, progress: 100, status: 'done' }
    : undefined,
})

const markActiveStageFailed = (stages: TranscriptionStageProgress): TranscriptionStageProgress => ({
  transcription: stages.transcription
    ? { ...stages.transcription, status: stages.transcription.status === 'active' ? 'failed' : stages.transcription.status }
    : undefined,
  smartSegmentation: stages.smartSegmentation
    ? {
        ...stages.smartSegmentation,
        status: stages.smartSegmentation.status === 'active' ? 'failed' : stages.smartSegmentation.status,
      }
    : undefined,
  subtitleCorrection: stages.subtitleCorrection
    ? {
        ...stages.subtitleCorrection,
        status: stages.subtitleCorrection.status === 'active' ? 'failed' : stages.subtitleCorrection.status,
      }
    : undefined,
})

const startTranscription = async () => {
  if (!selectedVideoPath.value || isTranscribing.value) {
    return
  }

  if (!isTauriRuntime()) {
    transcriptionError.value = '请在桌面应用中开始转录'
    return
  }

  await flushPendingSave()
  isTranscribing.value = true
  transcriptionError.value = ''
  transcriptionProgress.value = 0
  transcriptionStageProgress.value = {
    transcription: { progress: 0, message: '准备转录', status: 'active' },
    ...(isSmartSegmentationEnabled.value
      ? { smartSegmentation: { progress: 0, message: '等待语音转录完成', status: 'pending' as TranscriptionStageStatus } }
      : {}),
    ...(currentSettings.value.isSubtitleCorrectionEnabled
      ? { subtitleCorrection: { progress: 0, message: '等待前置处理完成', status: 'pending' as TranscriptionStageStatus } }
      : {}),
  }
  transcriptionMessage.value = '准备转录'
  transcriptionWarnings.value = []
  transcriptionSegments.value = []
  transcriptionText.value = ''
  lastTranscriptionRevision.value = 0
  lastOutputPath.value = ''

  try {
    const result = await invoke<TranscriptionResult>('start_transcription', {
      request: {
        filePath: selectedVideoPath.value,
        model: selectedTranscriptionModel.value,
        sourceLanguage: selectedSourceLanguage.value,
        outputFormat: selectedTranscriptionFormat.value,
      },
    })

    lastTranscriptionRevision.value = Number.MAX_SAFE_INTEGER
    transcriptionSegments.value = result.segments
    transcriptionText.value = result.subtitleText
    lastOutputPath.value = result.outputPath
    transcriptionWarnings.value = result.warnings ?? []
    transcriptionProgress.value = 100
    transcriptionStageProgress.value = markStageProgressDone(transcriptionStageProgress.value)
    transcriptionMessage.value = `转录成功 · ${result.segments.length} 条字幕`
  } catch (error) {
    transcriptionError.value = stringifyError(error)
    transcriptionMessage.value = '转录失败'
    transcriptionStageProgress.value = markActiveStageFailed(transcriptionStageProgress.value)
  } finally {
    isTranscribing.value = false
  }
}

const exportTranscription = async () => {
  if (!transcriptionText.value || !isTauriRuntime()) {
    return
  }

  const suggestedPath = buildExportPath()

  try {
    const outputPath = await save({
      title: '导出字幕',
      defaultPath: suggestedPath,
      filters: [
        {
          name: `${transcriptionFormatLabel.value} 字幕`,
          extensions: [selectedTranscriptionFormat.value],
        },
      ],
    })

    if (!outputPath) {
      return
    }

    await invoke('save_transcription_file', {
      path: ensureSubtitleExtension(outputPath, selectedTranscriptionFormat.value),
      content: transcriptionText.value,
    })
    lastOutputPath.value = ensureSubtitleExtension(outputPath, selectedTranscriptionFormat.value)
    transcriptionMessage.value = '字幕已导出'
  } catch (error) {
    transcriptionError.value = stringifyError(error)
  }
}

const startTranslationProcessing = () => {
  if (!canStartTranslationProcessing.value) {
    return
  }

  translationMessage.value = '已选择字幕，等待接入处理流程'
}

const openDialog = (dialog: TranslateDialog) => {
  languageSearch.value = ''
  activeDialog.value = dialog
}

const closeDialog = () => {
  activeDialog.value = null
}

const selectDialogValue = (value: string) => {
  switch (activeDialog.value) {
    case TranslateDialog.TranscriptionModel:
      selectedTranscriptionModel.value = value as TranscriptionModel
      break
    case TranslateDialog.SourceLanguage:
      selectedSourceLanguage.value = value
      break
    case TranslateDialog.TranscriptionFormat:
      selectedTranscriptionFormat.value = value as SubtitleFormat
      break
    case TranslateDialog.VideoContentType:
      selectedVideoContentType.value = value as VideoContentType
      break
    case TranslateDialog.TargetLanguage:
      selectedTargetLanguage.value = value
      break
    case TranslateDialog.OutputMode:
      selectedOutputMode.value = value as OutputMode
      break
    case TranslateDialog.TranslationFormat:
      selectedTranslationFormat.value = value as SubtitleFormat
      break
  }

  closeDialog()
}

const handleKeydown = (event: KeyboardEvent) => {
  if (event.key === 'Escape') {
    closeDialog()
  }
}

const registerProgressListener = async () => {
  if (!isTauriRuntime()) {
    return
  }

  unlistenTranscriptionProgress = await listen<TranscriptionProgress>('transcription-progress', (event) => {
    const payload = event.payload

    if (typeof payload.revision === 'number') {
      if (payload.revision <= lastTranscriptionRevision.value) {
        return
      }

      lastTranscriptionRevision.value = payload.revision
      transcriptionProgress.value = clampProgress(payload.progress)
      transcriptionMessage.value = payload.message
      if (payload.stageProgress) {
        transcriptionStageProgress.value = payload.stageProgress
      }

      if (payload.segments) {
        transcriptionSegments.value = payload.segments
      }

      if (payload.warnings) {
        transcriptionWarnings.value = payload.warnings
      }
      return
    }

    transcriptionProgress.value = clampProgress(payload.progress)
    transcriptionMessage.value = payload.message
    if (payload.stageProgress) {
      transcriptionStageProgress.value = payload.stageProgress
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

const fileNameFromPath = (path: string) => {
  const normalizedPath = path.replace(/\\/g, '/')
  return normalizedPath.split('/').filter(Boolean).pop() ?? path
}

const fileExtension = (path: string) => {
  const fileName = fileNameFromPath(path)
  const extension = fileName.split('.').pop()
  return extension ? extension.toLowerCase() : ''
}

const buildExportPath = () => {
  if (lastOutputPath.value) {
    return replaceExtension(lastOutputPath.value, selectedTranscriptionFormat.value)
  }

  if (!selectedVideoPath.value) {
    return `字幕.${selectedTranscriptionFormat.value}`
  }

  return replaceExtension(selectedVideoPath.value, selectedTranscriptionFormat.value)
}

const replaceExtension = (path: string, extension: string) => {
  const withoutExtension = path.replace(/\.[^/.\\]+$/, '')
  return `${withoutExtension}.${extension}`
}

const ensureSubtitleExtension = (path: string, extension: string) => {
  return path.toLowerCase().endsWith(`.${extension}`) ? path : `${path}.${extension}`
}

const formatSegmentTime = (ms: number) => {
  const safeMs = Math.max(0, Math.round(ms))
  const milliseconds = safeMs % 1000
  const totalSeconds = Math.floor(safeMs / 1000)
  const seconds = totalSeconds % 60
  const totalMinutes = Math.floor(totalSeconds / 60)
  const minutes = totalMinutes % 60
  const hours = Math.floor(totalMinutes / 60)

  return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}.${String(milliseconds).padStart(3, '0')}`
}

const transcriptionSegmentStatusLabel = (status?: TranscriptionSegmentStatus) => {
  return status ? transcriptionSegmentStatusLabels[status] : transcriptionSegmentStatusLabels.raw
}

const stringifyError = (error: unknown) => {
  if (typeof error === 'string') {
    return error
  }

  if (error instanceof Error) {
    return error.message
  }

  return '操作失败'
}

watch(createSettingsSnapshot, scheduleSaveSettings, { deep: true })

window.addEventListener('keydown', handleKeydown)

onMounted(() => {
  void loadStoredSettings()
  void registerProgressListener()
  void registerDragDropListener()
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown)
  unlistenTranscriptionProgress?.()
  unlistenDragDrop?.()

  if (saveSettingsTimer !== undefined) {
    window.clearTimeout(saveSettingsTimer)
    saveSettingsTimer = undefined
    void saveSettingsNow()
  }
})
</script>
