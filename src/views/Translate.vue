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
              <div class="translate-drop-zone">
                <UploadCloud class="translate-drop-icon" :stroke-width="2.1" aria-hidden="true" />
                <div class="translate-drop-copy">
                  <span class="translate-drop-title">选择需要转录的视频</span>
                  <span class="translate-drop-subtitle">支持本地视频文件，后续将接入真实文件选择和任务队列</span>
                </div>
                <button class="settings-action" type="button" disabled>选择视频</button>
              </div>

              <div class="translate-file-strip" aria-label="当前视频">
                <FileVideo :stroke-width="2.1" aria-hidden="true" />
                <span>尚未选择视频</span>
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
                  <div class="setting-subtitle">后续将结合 LLM 优化字幕断句</div>
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
                <span class="translate-status-dot" aria-hidden="true" />
                <span>等待选择视频</span>
              </div>
              <div class="translate-actions">
                <button class="settings-action" type="button" disabled>开始转录</button>
                <button class="settings-action" type="button" disabled>导出字幕</button>
              </div>
            </div>

            <div class="translate-preview translate-preview-empty">
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
              <div class="translate-drop-zone">
                <UploadCloud class="translate-drop-icon" :stroke-width="2.1" aria-hidden="true" />
                <div class="translate-drop-copy">
                  <span class="translate-drop-title">选择转录后的字幕</span>
                  <span class="translate-drop-subtitle">支持 SRT、VTT、ASS，后续将接入字幕解析和预览</span>
                </div>
                <button class="settings-action" type="button" disabled>选择字幕</button>
              </div>

              <div class="translate-file-strip" aria-label="当前字幕">
                <FileText :stroke-width="2.1" aria-hidden="true" />
                <span>尚未选择字幕</span>
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
                <span class="translate-status-dot" aria-hidden="true" />
                <span>等待选择字幕</span>
              </div>
              <div class="translate-actions">
                <button class="settings-action" type="button" disabled>开始处理</button>
                <button class="settings-action" type="button" disabled>导出结果</button>
              </div>
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
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  Bot,
  Captions,
  ChevronRight,
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

type DialogOption = {
  value: string
  label: string
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
let isApplyingStoredSettings = false
let hasLoadedOnce = false
let saveSettingsTimer: ReturnType<typeof window.setTimeout> | undefined

const isTauriRuntime = () => '__TAURI_INTERNALS__' in window

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

watch(createSettingsSnapshot, scheduleSaveSettings, { deep: true })

window.addEventListener('keydown', handleKeydown)

onMounted(() => {
  void loadStoredSettings()
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown)

  if (saveSettingsTimer !== undefined) {
    window.clearTimeout(saveSettingsTimer)
    saveSettingsTimer = undefined
    void saveSettingsNow()
  }
})
</script>
