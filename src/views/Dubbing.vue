<template>
  <div class="page translate-page">
    <header class="translate-header">
      <div>
        <h1 class="page-title">配音</h1>
      </div>

      <div class="translate-tabs" role="tablist" aria-label="配音工作台">
        <button
          v-for="tab in dubbingTabs"
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
        v-if="activeTab === DubbingTab.Workflow"
        :id="`${DubbingTab.Workflow}-panel`"
        class="translate-panel"
        role="tabpanel"
        :aria-labelledby="`${DubbingTab.Workflow}-tab`"
      >
        <div class="translate-grid">
          <section class="settings-section" aria-labelledby="dubbing-source-title">
            <div id="dubbing-source-title" class="section-heading">
              <Video aria-hidden="true" />
              <span>素材输入</span>
            </div>

            <div class="settings-panel translate-drop-panel">
              <div
                ref="sourceDropZoneRef"
                class="translate-drop-zone"
                :class="{ 'drag-active': isSourceDragActive }"
                @dragenter.prevent="isSourceDragActive = true"
                @dragover.prevent
                @dragleave.prevent="clearSourceDragTarget"
                @drop.prevent="handleSourceBrowserDrop"
              >
                <UploadCloud class="translate-drop-icon" :stroke-width="2.1" aria-hidden="true" />
                <div class="translate-drop-copy">
                  <span class="translate-drop-title">选择或拖入视频和字幕素材</span>
                  <span class="translate-drop-subtitle">支持常见视频格式及常见字幕格式</span>
                </div>
                <button class="settings-action" type="button" @click="selectMaterialFiles">选择素材</button>
              </div>

              <div class="translate-file-strip" aria-label="当前素材">
                <FileVideo :stroke-width="2.1" aria-hidden="true" />
                <span>{{ selectedMaterialSummary }}</span>
              </div>

              <div v-if="sourceInputError" class="translate-alert" role="alert">
                <CircleAlert :stroke-width="2.1" aria-hidden="true" />
                <span>{{ sourceInputError }}</span>
              </div>
            </div>
          </section>

          <section class="settings-section" aria-labelledby="dubbing-options-title">
            <div id="dubbing-options-title" class="section-heading">
              <SlidersHorizontal aria-hidden="true" />
              <span>配音参数</span>
            </div>

            <div class="settings-panel">
              <div class="setting-row">
                <Timer class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <div class="setting-copy">
                  <div class="setting-title">TTS 间隔</div>
                  <div class="setting-subtitle">分段语音停顿时长</div>
                </div>
                <div class="setting-range-control dubbing-range-control">
                  <span class="setting-range-value dubbing-range-value">{{ ttsIntervalMs }} 毫秒</span>
                  <input
                    v-model.number="ttsIntervalMs"
                    class="setting-range"
                    type="range"
                    min="0"
                    max="1000"
                    step="10"
                    aria-label="TTS 间隔"
                  />
                </div>
              </div>

              <button class="setting-row setting-row-button" type="button" @click="openReferenceAudioDialog">
                <FileMusic class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="setting-copy">
                  <span class="setting-title">参考音频</span>
                  <span class="setting-subtitle">选择参考音频来源</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ referenceAudioSourceLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>

              <div class="setting-row">
                <Music class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <div class="setting-copy">
                  <div class="setting-title">背景音乐</div>
                  <div class="setting-subtitle">开启后分离源视频伴奏并跟随变速同步混入最终视频</div>
                </div>
                <button
                  class="setting-toggle"
                  :class="{ active: isBackgroundMusicEnabled }"
                  type="button"
                  :aria-pressed="isBackgroundMusicEnabled"
                  @click="isBackgroundMusicEnabled = !isBackgroundMusicEnabled"
                >
                  <span class="setting-toggle-label">{{ isBackgroundMusicEnabled ? '开' : '关' }}</span>
                  <span class="setting-toggle-track" aria-hidden="true">
                    <span class="setting-toggle-thumb" />
                  </span>
                </button>
              </div>

              <div class="setting-row">
                <Volume2 class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <div class="setting-copy">
                  <div class="setting-title">背景音乐音量</div>
                  <div class="setting-subtitle">背景音乐混入音量</div>
                </div>
                <div class="setting-range-control dubbing-range-control">
                  <span class="setting-range-value dubbing-range-value">{{ backgroundMusicVolume.toFixed(1) }}</span>
                  <input
                    v-model.number="backgroundMusicVolume"
                    class="setting-range"
                    type="range"
                    min="0"
                    max="1"
                    step="0.1"
                    aria-label="背景音乐音量"
                  />
                </div>
              </div>
            </div>
          </section>
        </div>

        <section class="settings-section translate-output-section" aria-labelledby="dubbing-output-title">
          <div id="dubbing-output-title" class="section-heading">
            <ListVideo aria-hidden="true" />
            <span>配音结果</span>
          </div>

          <div class="settings-panel translate-result-panel">
            <div class="translate-status-bar">
              <div class="translate-status">
                <span class="translate-status-dot" :class="dubbingStatusClass" aria-hidden="true" />
                <span>{{ dubbingStatusText }}</span>
              </div>
              <div class="translate-actions">
                <button
                  class="settings-action"
                  type="button"
                  :disabled="!canStartDubbing"
                  @click="startDubbing"
                >
                  {{ startDubbingButtonText }}
                </button>
                <button class="settings-action" type="button" disabled>导出视频</button>
              </div>
            </div>

            <div v-if="currentDubbingStage" class="translate-progress" aria-label="配音处理进度">
              <div class="translate-progress-track">
                <span
                  class="translate-progress-bar"
                  :style="{ width: `${currentDubbingStage.progress}%` }"
                  aria-hidden="true"
                />
              </div>
              <span class="translate-progress-value">{{ currentDubbingStage.progress }}%</span>
            </div>

            <div v-if="dubbingErrorMessage" class="translate-alert" role="alert">
              <CircleAlert :stroke-width="2.1" aria-hidden="true" />
              <span>{{ dubbingErrorMessage }}</span>
            </div>

            <div v-if="dubbingWarningMessage" class="translate-alert translate-alert-warning" role="status">
              <CircleAlert :stroke-width="2.1" aria-hidden="true" />
              <span>{{ dubbingWarningMessage }}</span>
            </div>

            <div v-if="preprocessedSubtitleSegments.length > 0" class="translate-preview translate-subtitle-list">
              <div
                v-for="(segment, index) in preprocessedSubtitleSegments"
                :key="segment.uid || `dubbing-subtitle-${index}`"
                class="translate-subtitle-row"
              >
                <span class="translate-subtitle-index">{{ index + 1 }}</span>
                <span
                  class="translate-subtitle-status"
                  :class="`status-${normalizeDubbingSegmentStatus(segment.status)}`"
                >
                  {{ dubbingSegmentStatusLabel(segment.status) }}
                </span>
                <span class="translate-subtitle-time translate-subtitle-start">
                  {{ formatSegmentTime(segment.startTime) }}
                </span>
                <span class="translate-subtitle-time translate-subtitle-end">
                  {{ formatSegmentTime(segment.endTime) }}
                </span>
                <p>{{ segment.text }}</p>
              </div>
            </div>

            <div v-else-if="visibleDubbingStages.length > 0" class="translate-preview dubbing-stage-preview">
              <div
                v-for="stage in visibleDubbingStages"
                :key="stage.key"
                class="dubbing-stage-row"
              >
                <span class="dubbing-stage-mark" :class="stage.status" aria-hidden="true" />
                <span class="dubbing-stage-label">{{ stage.label }}</span>
                <span class="dubbing-stage-message">{{ stage.message }}</span>
              </div>
            </div>

            <div v-else class="translate-preview translate-preview-empty">
              <MicVocal class="translate-empty-icon" :stroke-width="2.1" aria-hidden="true" />
              <span class="translate-empty-title">暂无配音内容</span>
              <span class="translate-empty-subtitle">选择视频和字幕后，配音任务的字幕、音频和合成状态会显示在这里</span>
            </div>
          </div>
        </section>
      </section>

      <section
        v-else
        :id="`${DubbingTab.Models}-panel`"
        class="translate-panel"
        role="tabpanel"
        :aria-labelledby="`${DubbingTab.Models}-tab`"
      >
        <section class="settings-section" aria-labelledby="dubbing-models-title">
          <div class="dubbing-section-header">
            <div id="dubbing-models-title" class="section-heading">
              <Boxes aria-hidden="true" />
              <span>模型合集</span>
            </div>

            <button class="settings-action dubbing-add-button" type="button" @click="openAddModelDialog">
              <Plus :stroke-width="2.1" aria-hidden="true" />
              <span>添加模型</span>
            </button>
          </div>

          <div v-if="modelsError" class="translate-alert" role="alert">
            <CircleAlert :stroke-width="2.1" aria-hidden="true" />
            <span>{{ modelsError }}</span>
          </div>

          <div v-if="dubbingModels.length > 0" class="dubbing-model-grid">
            <article
              v-for="model in dubbingModels"
              :key="model.id"
              class="dubbing-model-card"
              :class="{ disabled: !model.enabled, 'index-model': isIndexTts2Model(model) }"
            >
              <header class="dubbing-model-card-header">
                <span class="dubbing-engine-pill">{{ model.engineLabel }}</span>
                <button
                  class="dubbing-icon-button"
                  type="button"
                  :aria-label="`删除${modelTitle(model)}`"
                  @click="openDeleteConfirmDialog(model)"
                >
                  <Trash2 :stroke-width="2.1" aria-hidden="true" />
                </button>
              </header>

              <div class="dubbing-model-copy">
                <h2 :title="modelTitle(model)">{{ modelTitle(model) }}</h2>
                <p>{{ modelSubtitle(model) }}</p>
              </div>

              <dl v-if="!isIndexTts2Model(model)" class="dubbing-model-meta">
                <div>
                  <dt>语言</dt>
                  <dd>{{ model.locale || '未知' }}</dd>
                </div>
                <div>
                  <dt>声音</dt>
                  <dd>{{ genderLabel(model.gender) }}</dd>
                </div>
              </dl>

              <footer class="dubbing-model-actions">
                <button
                  class="setting-toggle"
                  :class="{ active: model.enabled }"
                  type="button"
                  :aria-pressed="model.enabled"
                  :disabled="isModelUpdating(model.id)"
                  @click="toggleModel(model)"
                >
                  <span class="setting-toggle-label">{{ model.enabled ? '开' : '关' }}</span>
                  <span class="setting-toggle-track" aria-hidden="true">
                    <span class="setting-toggle-thumb" />
                  </span>
                </button>

                <button
                  class="settings-action dubbing-preview-button"
                  type="button"
                  :disabled="isPreviewing(model.engine, model.modelKey, modelEndpoint(model))"
                  @click="previewVoice(model.engine, model.modelKey, model.locale, modelEndpoint(model))"
                >
                  <Volume2 :stroke-width="2.1" aria-hidden="true" />
                  <span>{{ isPreviewing(model.engine, model.modelKey, modelEndpoint(model)) ? '试听中' : '试听' }}</span>
                </button>
              </footer>
            </article>
          </div>

          <div v-else class="settings-panel dubbing-empty-panel">
            <MicVocal class="translate-empty-icon" :stroke-width="2.1" aria-hidden="true" />
            <span class="translate-empty-title">{{ isModelsLoading ? '正在读取模型合集' : '暂无配音模型' }}</span>
            <span class="translate-empty-subtitle">添加语音后，会作为独立配音模型显示在这里</span>
            <button class="settings-action dubbing-add-button" type="button" @click="openAddModelDialog">
              <Plus :stroke-width="2.1" aria-hidden="true" />
              <span>添加模型</span>
            </button>
          </div>
        </section>
      </section>
    </main>

    <Teleport to="body">
      <div
        v-if="isReferenceAudioDialogOpen"
        class="dialog-backdrop"
        role="presentation"
        @click.self="closeReferenceAudioDialog"
      >
        <section
          class="settings-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="reference-audio-dialog-title"
        >
          <h2 id="reference-audio-dialog-title" class="dialog-title">参考音频</h2>
          <div class="dialog-options" role="radiogroup" aria-label="参考音频">
            <button
              v-for="option in referenceAudioSourceOptions"
              :key="option.value"
              class="dialog-option"
              :class="{ active: selectedReferenceAudioSource === option.value }"
              type="button"
              role="radio"
              :aria-checked="selectedReferenceAudioSource === option.value"
              @click="selectReferenceAudioSource(option.value)"
            >
              <span class="dialog-radio" aria-hidden="true" />
              <span>{{ option.label }}</span>
            </button>
          </div>
        </section>
      </div>

      <div v-if="isAddModelDialogOpen" class="dialog-backdrop" role="presentation" @click.self="closeAddModelDialog">
        <section
          class="settings-dialog dubbing-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="dubbing-add-model-title"
        >
          <header class="dubbing-dialog-header">
            <h2 id="dubbing-add-model-title" class="dialog-title">添加配音模型</h2>
            <button class="dubbing-icon-button" type="button" aria-label="关闭添加模型" @click="closeAddModelDialog">
              <X :stroke-width="2.1" aria-hidden="true" />
            </button>
          </header>

          <div class="dubbing-dialog-block">
            <div class="dubbing-dialog-label">配音引擎</div>
            <div class="dialog-options dubbing-engine-options" role="radiogroup" aria-label="配音引擎">
              <button
                v-for="option in dubbingEngineOptions"
                :key="option.value"
                class="dialog-option"
                :class="{ active: selectedEngine === option.value }"
                type="button"
                role="radio"
                :aria-checked="selectedEngine === option.value"
                @click="selectEngine(option.value)"
              >
                <span class="dialog-radio" aria-hidden="true" />
                <span>{{ option.label }}</span>
              </button>
            </div>
          </div>

          <div class="dubbing-dialog-block">
            <div class="dubbing-dialog-label">语音</div>
            <label class="language-search-field dubbing-search-field">
              <Search class="language-search-icon" :stroke-width="2.1" aria-hidden="true" />
              <input
                v-model="voiceSearch"
                class="settings-input language-search-input"
                type="search"
                placeholder="搜索语音、语言或 ID"
                aria-label="搜索语音"
              />
            </label>

            <div v-if="dialogError" class="translate-alert" role="alert">
              <CircleAlert :stroke-width="2.1" aria-hidden="true" />
              <span>{{ dialogError }}</span>
            </div>

            <div
              class="dialog-options dubbing-voice-options"
              role="radiogroup"
              aria-label="配音语音"
            >
              <button
                v-for="voice in filteredVoices"
                :key="voiceOptionKey(voice)"
                class="dialog-option dubbing-voice-option"
                :class="{ active: selectedVoiceKey === voiceOptionKey(voice) }"
                type="button"
                role="radio"
                :aria-checked="selectedVoiceKey === voiceOptionKey(voice)"
                :disabled="isVoiceAdded(voice)"
                @click="selectVoice(voice)"
              >
                <span class="dialog-radio" aria-hidden="true" />
                <span class="dubbing-voice-copy">
                  <span class="dubbing-voice-name">{{ voice.displayName }}</span>
                  <span class="dubbing-voice-meta">{{ voiceMeta(voice) }}</span>
                </span>
                <span v-if="isVoiceAdded(voice)" class="dubbing-added-label">已添加</span>
              </button>

              <span v-if="isVoicesLoading" class="language-empty">正在加载语音列表</span>
              <span v-else-if="filteredVoices.length === 0" class="language-empty">未找到语音</span>
            </div>

            <div v-if="isIndexTts2Selected" class="dubbing-endpoint-field">
              <label class="dubbing-dialog-label" for="index-tts-endpoint">服务地址</label>
              <input
                id="index-tts-endpoint"
                v-model="indexTts2Endpoint"
                class="settings-input dubbing-endpoint-input"
                type="url"
                placeholder="http://127.0.0.1:7860"
                autocomplete="off"
              />
            </div>
          </div>

          <footer class="dubbing-dialog-actions">
            <button class="settings-action" type="button" @click="closeAddModelDialog">取消</button>
            <button
              class="settings-action dubbing-preview-button"
              type="button"
              :disabled="!canSubmitSelectedVoice || !selectedVoice || isPreviewing(selectedVoice.engine, selectedVoice.modelKey, endpointForVoice(selectedVoice))"
              @click="previewSelectedVoice"
            >
              <Volume2 :stroke-width="2.1" aria-hidden="true" />
              <span>{{ selectedVoice && isPreviewing(selectedVoice.engine, selectedVoice.modelKey, endpointForVoice(selectedVoice)) ? '试听中' : '试听' }}</span>
            </button>
            <button
              class="settings-action"
              type="button"
              :disabled="!canSubmitSelectedVoice || isAddingModel"
              @click="addSelectedVoice"
            >
              {{ isAddingModel ? '添加中' : '添加' }}
            </button>
          </footer>
        </section>
      </div>

      <div
        v-if="modelPendingDelete"
        class="dialog-backdrop"
        role="presentation"
        @click.self="closeDeleteConfirmDialog"
      >
        <section
          class="settings-dialog dubbing-confirm-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="dubbing-delete-model-title"
        >
          <header class="dubbing-dialog-header">
            <h2 id="dubbing-delete-model-title" class="dialog-title">删除配音模型</h2>
            <button class="dubbing-icon-button" type="button" aria-label="关闭删除确认" @click="closeDeleteConfirmDialog">
              <X :stroke-width="2.1" aria-hidden="true" />
            </button>
          </header>

          <p class="dubbing-confirm-copy">
            <CircleAlert :stroke-width="2.1" aria-hidden="true" />
            <span>确认删除“{{ modelTitle(modelPendingDelete) }}”吗？删除后需要重新添加该语音模型。</span>
          </p>

          <footer class="dubbing-dialog-actions">
            <button class="settings-action" type="button" :disabled="isDeletingModel" @click="closeDeleteConfirmDialog">
              取消
            </button>
            <button
              class="settings-action dubbing-danger-action"
              type="button"
              :disabled="isDeletingModel"
              @click="confirmDeleteModel"
            >
              {{ isDeletingModel ? '删除中' : '删除' }}
            </button>
          </footer>
        </section>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { DragDropEvent } from '@tauri-apps/api/webview'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import {
  Boxes,
  ChevronRight,
  CircleAlert,
  FileMusic,
  FileVideo,
  ListVideo,
  MicVocal,
  Music,
  Plus,
  Search,
  SlidersHorizontal,
  Timer,
  Trash2,
  UploadCloud,
  Video,
  Volume2,
  X,
} from 'lucide-vue-next'

defineOptions({ name: 'Dubbing' })

enum DubbingTab {
  Workflow = 'dubbing-workflow',
  Models = 'dubbing-models',
}

enum DubbingEngineKind {
  EdgeTts = 'edge-tts',
  NanoAiTts = 'nano-ai-tts',
  IndexTts2 = 'index-tts-2',
}

enum ReferenceAudioSource {
  ExistingDubbing = 'existing-dubbing',
  CustomAudioFile = 'custom-audio-file',
}

type DubbingVoiceOption = {
  engine: DubbingEngineKind
  engineLabel: string
  modelKey: string
  displayName: string
  locale: string
  gender: string
  metadata: Record<string, unknown>
}

type DubbingModel = DubbingVoiceOption & {
  id: string
  enabled: boolean
  createdAt: string
  updatedAt: string
}

type PreviewDubbingVoiceResult = {
  audioDataUrl: string
}

type DubbingStageStatus = 'pending' | 'active' | 'done' | 'failed' | 'interrupted'

type DubbingProgressStage = {
  progress: number
  message: string
  status: DubbingStageStatus
}

type DubbingStageProgress = {
  material?: DubbingProgressStage
  subtitlePreprocess?: DubbingProgressStage
  ttsSynthesis?: DubbingProgressStage
  audioMerge?: DubbingProgressStage
  videoCompose?: DubbingProgressStage
}

type DubbingTaskArtifact = {
  kind: string
  path: string
  metadata: Record<string, unknown>
  createdAt: string
  updatedAt: string
}

type DubbingSubtitleSegmentStatus = 'raw' | 'active' | 'done' | 'failed'

type DubbingSubtitleSegment = {
  uid?: string
  text: string
  startTime: number
  endTime: number
  status?: DubbingSubtitleSegmentStatus | string
}

type DubbingTaskSnapshot = {
  id: string
  pairKey: string
  videoPath: string
  subtitlePath: string
  workDir: string
  currentStage: string
  status: string
  progress: number
  message: string
  stages: DubbingStageProgress
  artifacts: DubbingTaskArtifact[]
  segments: DubbingSubtitleSegment[]
  warnings: string[]
  errorMessage: string
  revision: number
  createdAt: string
  updatedAt: string
}

type VisibleDubbingStage = {
  key: string
  label: string
  progress: number
  message: string
  status: DubbingStageStatus
}

const INDEX_TTS2_DEFAULT_ENDPOINT = 'http://127.0.0.1:7860'

const dubbingTabs = [
  { value: DubbingTab.Workflow, label: '配音流程', icon: MicVocal },
  { value: DubbingTab.Models, label: '模型合集', icon: Boxes },
]

const dubbingEngineOptions = [
  { value: DubbingEngineKind.EdgeTts, label: 'EDGE-TTS' },
  { value: DubbingEngineKind.NanoAiTts, label: '纳米AI TTS' },
  { value: DubbingEngineKind.IndexTts2, label: 'Index-TTS 2.0' },
] as const

const referenceAudioSourceOptions = [
  { value: ReferenceAudioSource.ExistingDubbing, label: '克隆现有配音' },
  { value: ReferenceAudioSource.CustomAudioFile, label: '自定义音频文件' },
] as const

const activeTab = ref<DubbingTab>(DubbingTab.Workflow)
const dubbingModels = ref<DubbingModel[]>([])
const voices = ref<DubbingVoiceOption[]>([])
const selectedEngine = ref<DubbingEngineKind>(DubbingEngineKind.EdgeTts)
const selectedReferenceAudioSource = ref<ReferenceAudioSource>(ReferenceAudioSource.ExistingDubbing)
const isReferenceAudioDialogOpen = ref(false)
const ttsIntervalMs = ref(150)
const isBackgroundMusicEnabled = ref(true)
const backgroundMusicVolume = ref(0.5)
const sourceDropZoneRef = ref<HTMLElement | null>(null)
const isSourceDragActive = ref(false)
const selectedMaterialVideoPath = ref('')
const selectedMaterialSubtitlePath = ref('')
const sourceInputError = ref('')
const activeDubbingTask = ref<DubbingTaskSnapshot | null>(null)
const isPreparingMaterial = ref(false)
const isDubbingRunning = ref(false)
const dubbingError = ref('')
const lastDubbingRevision = ref(0)
const selectedVoiceKey = ref('')
const voiceSearch = ref('')
const indexTts2Endpoint = ref(INDEX_TTS2_DEFAULT_ENDPOINT)
const isAddModelDialogOpen = ref(false)
const isModelsLoading = ref(false)
const isVoicesLoading = ref(false)
const isAddingModel = ref(false)
const isDeletingModel = ref(false)
const modelsError = ref('')
const dialogError = ref('')
const updatingModelIds = ref<string[]>([])
const previewingKey = ref('')
const modelPendingDelete = ref<DubbingModel | null>(null)
let previewAudio: HTMLAudioElement | null = null
let previewAudioUrl = ''
let materialPrepareToken = 0
let unlistenSourceDragDrop: UnlistenFn | undefined
let unlistenDubbingProgress: UnlistenFn | undefined

const isTauriRuntime = () => '__TAURI_INTERNALS__' in window
const dubbingVideoExtensions = ['mp4', 'mov', 'mkv', 'avi', 'flv', 'wmv', 'webm', 'm4v']
const dubbingSubtitleExtensions = ['srt', 'vtt', 'ass', 'ssa', 'lrc', 'sbv', 'smi', 'sami', 'ttml', 'dfxp', 'txt']

const addedVoiceKeys = computed(() => {
  return new Set(
    dubbingModels.value
      .filter((model) => model.engine !== DubbingEngineKind.IndexTts2)
      .map((model) => voiceKey(model.engine, model.modelKey)),
  )
})

const filteredVoices = computed(() => {
  const query = voiceSearch.value.trim().toLowerCase()
  const availableVoices = voices.value.filter((voice) => voice.engine === selectedEngine.value)

  if (!query) {
    return availableVoices
  }

  return availableVoices.filter((voice) => {
    return [voice.displayName, voice.modelKey, voice.locale, voice.gender, voice.engineLabel]
      .some((value) => value.toLowerCase().includes(query))
  })
})

const selectedVoice = computed(() => {
  return voices.value.find((voice) => voiceOptionKey(voice) === selectedVoiceKey.value)
})

const isIndexTts2Selected = computed(() => selectedEngine.value === DubbingEngineKind.IndexTts2)

const canSubmitSelectedVoice = computed(() => {
  return Boolean(selectedVoice.value) && (!isIndexTts2Selected.value || indexTts2Endpoint.value.trim())
})

const referenceAudioSourceLabel = computed(() => {
  return referenceAudioSourceOptions.find((option) => option.value === selectedReferenceAudioSource.value)?.label ?? ''
})

const selectedMaterialSummary = computed(() => {
  if (!selectedMaterialVideoPath.value && !selectedMaterialSubtitlePath.value) {
    return '未选择素材'
  }

  const videoLabel = selectedMaterialVideoPath.value
    ? `视频：${fileNameFromPath(selectedMaterialVideoPath.value)}`
    : '未选择视频'
  const subtitleLabel = selectedMaterialSubtitlePath.value
    ? `字幕：${fileNameFromPath(selectedMaterialSubtitlePath.value)}`
    : '未选择字幕'

  return `${videoLabel} · ${subtitleLabel}`
})

const preprocessedSubtitleSegments = computed(() => activeDubbingTask.value?.segments ?? [])

const visibleDubbingStages = computed<VisibleDubbingStage[]>(() => {
  const stages = activeDubbingTask.value?.stages
  if (!stages) {
    return []
  }

  return [
    { key: 'material', label: '素材准备', stage: stages.material },
    { key: 'subtitle-preprocess', label: '字幕预处理', stage: stages.subtitlePreprocess },
    { key: 'tts-synthesis', label: 'TTS 配音', stage: stages.ttsSynthesis },
    { key: 'audio-merge', label: '音频合成', stage: stages.audioMerge },
    { key: 'video-compose', label: '视频合成', stage: stages.videoCompose },
  ]
    .filter((item): item is { key: string; label: string; stage: DubbingProgressStage } => Boolean(item.stage))
    .map((item) => ({
      key: item.key,
      label: item.label,
      progress: clampProgress(item.stage.progress),
      message: item.stage.message,
      status: item.stage.status,
    }))
})

const currentDubbingStage = computed(() => {
  const stages = visibleDubbingStages.value
  return (
    [...stages].reverse().find((stage) => stage.status === 'active') ??
    [...stages].reverse().find((stage) => stage.status === 'failed') ??
    [...stages].reverse().find((stage) => stage.status === 'interrupted') ??
    [...stages].reverse().find((stage) => stage.status === 'pending') ??
    [...stages].reverse().find((stage) => stage.status === 'done') ??
    null
  )
})

const dubbingErrorMessage = computed(() => {
  return dubbingError.value || activeDubbingTask.value?.errorMessage || ''
})

const dubbingWarningMessage = computed(() => {
  const warnings = activeDubbingTask.value?.warnings ?? []
  if (warnings.length === 0 || dubbingErrorMessage.value) {
    return ''
  }

  const firstWarning = warnings[0]
  return warnings.length > 1 ? `${firstWarning}，另有 ${warnings.length - 1} 条提示` : firstWarning
})

const dubbingStatusText = computed(() => {
  if (isPreparingMaterial.value) {
    return '素材准备中'
  }

  if (dubbingErrorMessage.value) {
    return '配音失败'
  }

  if (isDubbingRunning.value && currentDubbingStage.value) {
    return currentDubbingStage.value.message
  }

  if (!activeDubbingTask.value) {
    return '等待素材'
  }

  if (activeDubbingTask.value.status === 'preprocessed') {
    return '字幕预处理完成'
  }

  if (activeDubbingTask.value.status === 'interrupted') {
    return '任务已中断'
  }

  if (activeDubbingTask.value.status === 'ready') {
    return '素材准备完成'
  }

  return activeDubbingTask.value.message || '素材准备完成'
})

const dubbingStatusClass = computed(() => ({
  active: isPreparingMaterial.value || isDubbingRunning.value,
  success:
    !isPreparingMaterial.value &&
    !isDubbingRunning.value &&
    Boolean(activeDubbingTask.value) &&
    activeDubbingTask.value?.status !== 'interrupted' &&
    !dubbingErrorMessage.value,
  warning: activeDubbingTask.value?.status === 'interrupted' && !dubbingErrorMessage.value,
  error: Boolean(dubbingErrorMessage.value),
}))

const canStartDubbing = computed(() => {
  if (!activeDubbingTask.value || isPreparingMaterial.value || isDubbingRunning.value) {
    return false
  }

  return ['ready', 'failed', 'interrupted'].includes(activeDubbingTask.value.status)
})

const startDubbingButtonText = computed(() => {
  if (isPreparingMaterial.value) {
    return '准备素材'
  }

  if (isDubbingRunning.value) {
    return '处理中'
  }

  if (activeDubbingTask.value && ['failed', 'interrupted'].includes(activeDubbingTask.value.status)) {
    return '继续配音'
  }

  return '开始配音'
})

const selectTab = (tab: DubbingTab) => {
  activeTab.value = tab
}

const selectMaterialFiles = async () => {
  if (!isTauriRuntime()) {
    sourceInputError.value = '请在桌面应用中选择素材'
    return
  }

  try {
    const selected = await open({
      title: '选择视频和字幕素材',
      multiple: true,
      filters: [
        {
          name: '视频和字幕素材',
          extensions: [...dubbingVideoExtensions, ...dubbingSubtitleExtensions],
        },
        {
          name: '视频文件',
          extensions: dubbingVideoExtensions,
        },
        {
          name: '字幕文件',
          extensions: dubbingSubtitleExtensions,
        },
      ],
    })

    if (!selected) {
      return
    }

    applyMaterialFiles(Array.isArray(selected) ? selected : [selected])
  } catch (error) {
    sourceInputError.value = stringifyError(error, '选择素材失败')
  }
}

const applyMaterialFiles = (paths: string[]) => {
  const unsupportedPaths: string[] = []
  let hasAcceptedFile = false

  paths.forEach((path) => {
    if (applyMaterialFile(path)) {
      hasAcceptedFile = true
      return
    }

    unsupportedPaths.push(path)
  })

  if (hasAcceptedFile) {
    sourceInputError.value = ''
    resetDubbingTaskState()
  }

  if (unsupportedPaths.length > 0) {
    sourceInputError.value = `不支持的素材：${fileNameFromPath(unsupportedPaths[0])}`
  }

  if (hasAcceptedFile) {
    void prepareDubbingMaterial()
  }
}

const applyMaterialFile = (path: string) => {
  const extension = fileExtension(path)

  if (dubbingVideoExtensions.includes(extension)) {
    selectedMaterialVideoPath.value = path
    return true
  }

  if (dubbingSubtitleExtensions.includes(extension)) {
    selectedMaterialSubtitlePath.value = path
    return true
  }

  return false
}

const resetDubbingTaskState = () => {
  activeDubbingTask.value = null
  dubbingError.value = ''
  lastDubbingRevision.value = 0
}

const prepareDubbingMaterial = async () => {
  if (!selectedMaterialVideoPath.value || !selectedMaterialSubtitlePath.value) {
    return
  }

  if (!isTauriRuntime()) {
    sourceInputError.value = '请在桌面应用中准备素材'
    return
  }

  const token = ++materialPrepareToken
  const videoPath = selectedMaterialVideoPath.value
  const subtitlePath = selectedMaterialSubtitlePath.value
  isPreparingMaterial.value = true
  sourceInputError.value = ''
  dubbingError.value = ''

  try {
    const snapshot = await invoke<DubbingTaskSnapshot>('prepare_dubbing_material', {
      request: {
        videoPath,
        subtitlePath,
      },
    })

    if (
      token !== materialPrepareToken ||
      videoPath !== selectedMaterialVideoPath.value ||
      subtitlePath !== selectedMaterialSubtitlePath.value
    ) {
      return
    }

    applyDubbingTaskSnapshot(snapshot)
  } catch (error) {
    if (token === materialPrepareToken) {
      sourceInputError.value = stringifyError(error, '素材准备失败')
      activeDubbingTask.value = null
    }
  } finally {
    if (token === materialPrepareToken) {
      isPreparingMaterial.value = false
    }
  }
}

const startDubbing = async () => {
  const task = activeDubbingTask.value
  if (!task || !canStartDubbing.value) {
    return
  }

  if (!isTauriRuntime()) {
    dubbingError.value = '请在桌面应用中开始配音'
    return
  }

  isDubbingRunning.value = true
  dubbingError.value = ''

  try {
    const snapshot = await invoke<DubbingTaskSnapshot>('start_dubbing_task', {
      request: {
        taskId: task.id,
        options: currentDubbingTaskOptions(),
      },
    })
    applyDubbingTaskSnapshot(snapshot)
  } catch (error) {
    dubbingError.value = stringifyError(error, '配音失败')
  } finally {
    isDubbingRunning.value = false
  }
}

const currentDubbingTaskOptions = () => ({
  ttsIntervalMs: ttsIntervalMs.value,
  referenceAudioSource: selectedReferenceAudioSource.value,
  isBackgroundMusicEnabled: isBackgroundMusicEnabled.value,
  backgroundMusicVolume: backgroundMusicVolume.value,
})

const applyDubbingTaskSnapshot = (snapshot: DubbingTaskSnapshot) => {
  activeDubbingTask.value = snapshot
  lastDubbingRevision.value = snapshot.revision
  selectedMaterialVideoPath.value = snapshot.videoPath || selectedMaterialVideoPath.value
  selectedMaterialSubtitlePath.value = snapshot.subtitlePath || selectedMaterialSubtitlePath.value

  if (!snapshot.errorMessage) {
    dubbingError.value = ''
  }
}

const handleSourceBrowserDrop = (event: DragEvent) => {
  isSourceDragActive.value = false

  const paths = Array.from(event.dataTransfer?.files ?? [])
    .map((file) => (file as File & { path?: string }).path)
    .filter((path): path is string => Boolean(path))

  if (paths.length === 0) {
    return
  }

  applyMaterialFiles(paths)
}

const clearSourceDragTarget = () => {
  isSourceDragActive.value = false
}

const openReferenceAudioDialog = () => {
  isReferenceAudioDialogOpen.value = true
}

const closeReferenceAudioDialog = () => {
  isReferenceAudioDialogOpen.value = false
}

const selectReferenceAudioSource = (source: ReferenceAudioSource) => {
  selectedReferenceAudioSource.value = source
  closeReferenceAudioDialog()
}

const selectEngine = (engine: DubbingEngineKind) => {
  selectedEngine.value = engine
  selectedVoiceKey.value = ''
  voiceSearch.value = ''
  void loadVoices()
}

const selectVoice = (voice: DubbingVoiceOption) => {
  if (isVoiceAdded(voice)) {
    return
  }

  selectedVoiceKey.value = voiceOptionKey(voice)
}

const openAddModelDialog = () => {
  dialogError.value = ''
  selectedVoiceKey.value = ''
  voiceSearch.value = ''
  indexTts2Endpoint.value = INDEX_TTS2_DEFAULT_ENDPOINT
  isAddModelDialogOpen.value = true
  void loadVoices()
}

const closeAddModelDialog = () => {
  isAddModelDialogOpen.value = false
  dialogError.value = ''
}

const openDeleteConfirmDialog = (model: DubbingModel) => {
  modelsError.value = ''
  modelPendingDelete.value = model
}

const closeDeleteConfirmDialog = () => {
  if (isDeletingModel.value) {
    return
  }

  modelPendingDelete.value = null
}

const loadModels = async () => {
  if (!isTauriRuntime()) {
    return
  }

  isModelsLoading.value = true
  modelsError.value = ''

  try {
    dubbingModels.value = await invoke<DubbingModel[]>('list_dubbing_models')
  } catch (error) {
    modelsError.value = stringifyError(error, '读取配音模型失败')
  } finally {
    isModelsLoading.value = false
  }
}

const loadVoices = async () => {
  if (!isTauriRuntime()) {
    dialogError.value = '请在桌面应用中加载语音列表'
    return
  }

  isVoicesLoading.value = true
  dialogError.value = ''

  try {
    voices.value = await invoke<DubbingVoiceOption[]>('list_dubbing_voices', {
      request: { engine: selectedEngine.value },
    })
  } catch (error) {
    dialogError.value = stringifyError(error, '加载语音列表失败')
  } finally {
    isVoicesLoading.value = false
  }
}

const addSelectedVoice = async () => {
  if (!selectedVoice.value || !canSubmitSelectedVoice.value || isAddingModel.value) {
    return
  }

  isAddingModel.value = true
  dialogError.value = ''

  try {
    const model = await invoke<DubbingModel>('add_dubbing_model', {
      request: {
        engine: selectedVoice.value.engine,
        modelKey: selectedVoice.value.modelKey,
        endpoint: endpointForVoice(selectedVoice.value),
      },
    })
    dubbingModels.value = [model, ...dubbingModels.value]
    closeAddModelDialog()
  } catch (error) {
    dialogError.value = stringifyError(error, '添加配音模型失败')
  } finally {
    isAddingModel.value = false
  }
}

const toggleModel = async (model: DubbingModel) => {
  if (isModelUpdating(model.id)) {
    return
  }

  updatingModelIds.value = [...updatingModelIds.value, model.id]
  modelsError.value = ''

  try {
    const updatedModel = await invoke<DubbingModel>('set_dubbing_model_enabled', {
      request: {
        id: model.id,
        enabled: !model.enabled,
      },
    })
    dubbingModels.value = dubbingModels.value.map((item) => (item.id === updatedModel.id ? updatedModel : item))
  } catch (error) {
    modelsError.value = stringifyError(error, '更新配音模型失败')
  } finally {
    updatingModelIds.value = updatingModelIds.value.filter((id) => id !== model.id)
  }
}

const confirmDeleteModel = async () => {
  const model = modelPendingDelete.value
  if (!model || isDeletingModel.value) {
    return
  }

  modelsError.value = ''
  isDeletingModel.value = true

  try {
    await invoke('delete_dubbing_model', {
      request: { id: model.id },
    })
    dubbingModels.value = dubbingModels.value.filter((item) => item.id !== model.id)
    modelPendingDelete.value = null
  } catch (error) {
    modelsError.value = stringifyError(error, '删除配音模型失败')
  } finally {
    isDeletingModel.value = false
  }
}

const previewSelectedVoice = async () => {
  if (!selectedVoice.value) {
    return
  }

  await previewVoice(
    selectedVoice.value.engine,
    selectedVoice.value.modelKey,
    selectedVoice.value.locale,
    endpointForVoice(selectedVoice.value),
  )
}

const previewVoice = async (engine: DubbingEngineKind, modelKey: string, locale = '', endpoint = '') => {
  if (!modelKey || isPreviewing(engine, modelKey, endpoint)) {
    return
  }

  if (!isTauriRuntime()) {
    setPreviewError('请在桌面应用中试听')
    return
  }

  const key = voicePreviewKey(engine, modelKey, endpoint)
  previewingKey.value = key
  clearErrors()

  try {
    const request = endpoint.trim()
      ? { engine, modelKey, locale, endpoint: endpoint.trim() }
      : { engine, modelKey, locale }
    const result = await invoke<PreviewDubbingVoiceResult>('preview_dubbing_voice', {
      request,
    })
    await playPreview(result.audioDataUrl)
  } catch (error) {
    setPreviewError(stringifyError(error, '试听失败'))
  } finally {
    previewingKey.value = ''
  }
}

const playPreview = async (audioDataUrl: string) => {
  stopPreviewAudio()

  const response = await fetch(audioDataUrl)
  const audioBlob = await response.blob()
  previewAudioUrl = URL.createObjectURL(audioBlob)
  previewAudio = new Audio(previewAudioUrl)
  await previewAudio.play()
}

const stopPreviewAudio = () => {
  if (previewAudio) {
    previewAudio.pause()
    previewAudio = null
  }

  if (previewAudioUrl) {
    URL.revokeObjectURL(previewAudioUrl)
    previewAudioUrl = ''
  }
}

const isVoiceAdded = (voice: DubbingVoiceOption) => {
  if (voice.engine === DubbingEngineKind.IndexTts2) {
    return false
  }

  return addedVoiceKeys.value.has(voiceKey(voice.engine, voice.modelKey))
}

const isPreviewing = (engine: DubbingEngineKind, modelKey: string, endpoint = '') => {
  return previewingKey.value === voicePreviewKey(engine, modelKey, endpoint)
}

const isModelUpdating = (id: string) => {
  return updatingModelIds.value.includes(id)
}

const isIndexTts2Model = (model: Pick<DubbingModel, 'engine'>) => {
  return model.engine === DubbingEngineKind.IndexTts2
}

const modelTitle = (model: DubbingModel) => {
  if (isIndexTts2Model(model)) {
    return modelEndpoint(model) || model.modelKey
  }

  return model.displayName
}

const modelSubtitle = (model: DubbingModel) => {
  if (isIndexTts2Model(model)) {
    return model.modelKey
  }

  return model.modelKey
}

const voiceKey = (engine: DubbingEngineKind, modelKey: string) => `${engine}:${modelKey}`

const voicePreviewKey = (engine: DubbingEngineKind, modelKey: string, endpoint = '') => {
  return `${voiceKey(engine, modelKey)}:${endpoint.trim()}`
}

const voiceOptionKey = (voice: DubbingVoiceOption) => {
  return `${voiceKey(voice.engine, voice.modelKey)}:${voice.displayName}`
}

const endpointForVoice = (voice: DubbingVoiceOption) => {
  if (voice.engine !== DubbingEngineKind.IndexTts2) {
    return ''
  }

  return indexTts2Endpoint.value.trim()
}

const modelEndpoint = (model: DubbingModel) => {
  if (model.engine !== DubbingEngineKind.IndexTts2) {
    return ''
  }

  return typeof model.metadata.endpoint === 'string' ? model.metadata.endpoint : ''
}

const voiceMeta = (voice: DubbingVoiceOption) => {
  if (voice.engine === DubbingEngineKind.EdgeTts) {
    return [voice.modelKey, voice.locale, genderLabel(voice.gender)].filter(Boolean).join(' · ')
  }

  return voice.modelKey
}

const genderLabel = (gender: string) => {
  switch (gender) {
    case 'Female':
      return '女声'
    case 'Male':
      return '男声'
    default:
      return gender || '未知'
  }
}

const clearErrors = () => {
  modelsError.value = ''
  dialogError.value = ''
}

const setPreviewError = (message: string) => {
  if (isAddModelDialogOpen.value) {
    dialogError.value = message
    return
  }

  modelsError.value = message
}

const registerDubbingProgressListener = async () => {
  if (!isTauriRuntime()) {
    return
  }

  unlistenDubbingProgress = await listen<DubbingTaskSnapshot>('dubbing-progress', (event) => {
    const snapshot = event.payload

    if (activeDubbingTask.value && snapshot.id !== activeDubbingTask.value.id) {
      return
    }

    if (snapshot.revision <= lastDubbingRevision.value) {
      return
    }

    applyDubbingTaskSnapshot(snapshot)
  })
}

const registerSourceDragDropListener = async () => {
  if (!isTauriRuntime()) {
    return
  }

  const [{ getCurrentWebview }, { getCurrentWindow }] = await Promise.all([
    import('@tauri-apps/api/webview'),
    import('@tauri-apps/api/window'),
  ])
  const webview = getCurrentWebview()
  const currentWindow = getCurrentWindow()

  unlistenSourceDragDrop = await webview.onDragDropEvent(async (event) => {
    const payload = event.payload

    if (payload.type === 'leave') {
      isSourceDragActive.value = false
      return
    }

    if (payload.type === 'over') {
      isSourceDragActive.value = isSourceDropEvent(payload, await currentWindow.scaleFactor())
      return
    }

    if (payload.type !== 'enter' && payload.type !== 'drop') {
      return
    }

    const isInsideDropZone = isSourceDropEvent(payload, await currentWindow.scaleFactor())
    isSourceDragActive.value = isInsideDropZone

    if (payload.type !== 'drop') {
      return
    }

    isSourceDragActive.value = false

    if (isInsideDropZone) {
      applyMaterialFiles(payload.paths)
    }
  })
}

const isSourceDropEvent = (
  payload: Extract<DragDropEvent, { type: 'enter' | 'over' | 'drop' }>,
  scaleFactor: number,
) => {
  const logicalPosition = payload.position.toLogical(scaleFactor)
  return isPointInsideElement(
    {
      x: logicalPosition.x,
      y: logicalPosition.y,
    },
    sourceDropZoneRef.value,
  )
}

const isPointInsideElement = (point: { x: number; y: number }, element: HTMLElement | null) => {
  if (!element) {
    return false
  }

  const rect = element.getBoundingClientRect()
  return point.x >= rect.left && point.x <= rect.right && point.y >= rect.top && point.y <= rect.bottom
}

const clampProgress = (value: number) => Math.min(Math.max(Math.round(value), 0), 100)

const normalizeDubbingSegmentStatus = (status?: string): DubbingSubtitleSegmentStatus => {
  return status === 'done' || status === 'active' || status === 'failed' || status === 'raw' ? status : 'raw'
}

const dubbingSegmentStatusLabel = (status?: string) => {
  switch (normalizeDubbingSegmentStatus(status)) {
    case 'active':
      return '处理中'
    case 'done':
      return '已处理'
    case 'failed':
      return '失败'
    default:
      return '原始'
  }
}

const formatSegmentTime = (ms: number) => {
  const totalMs = Math.max(Math.round(ms), 0)
  const hours = Math.floor(totalMs / 3_600_000)
  const minutes = Math.floor((totalMs % 3_600_000) / 60_000)
  const seconds = Math.floor((totalMs % 60_000) / 1000)
  const millis = totalMs % 1000

  return `${padTime(hours)}:${padTime(minutes)}:${padTime(seconds)},${millis.toString().padStart(3, '0')}`
}

const padTime = (value: number) => value.toString().padStart(2, '0')

const fileNameFromPath = (path: string) => {
  const normalizedPath = path.replace(/\\/g, '/')
  return normalizedPath.split('/').filter(Boolean).pop() ?? path
}

const fileExtension = (path: string) => {
  const fileName = fileNameFromPath(path)
  const extension = fileName.split('.').pop()
  return extension ? extension.toLowerCase() : ''
}

const handleKeydown = (event: KeyboardEvent) => {
  if (event.key === 'Escape') {
    closeReferenceAudioDialog()
    closeAddModelDialog()
    closeDeleteConfirmDialog()
  }
}

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
  void loadModels()
  void registerDubbingProgressListener()
  void registerSourceDragDropListener()
  window.addEventListener('keydown', handleKeydown)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown)
  unlistenDubbingProgress?.()
  unlistenSourceDragDrop?.()
  stopPreviewAudio()
})
</script>
