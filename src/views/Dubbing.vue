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
      />

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
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { Boxes, CircleAlert, MicVocal, Plus, Search, Trash2, Volume2, X } from 'lucide-vue-next'

enum DubbingTab {
  Workflow = 'dubbing-workflow',
  Models = 'dubbing-models',
}

enum DubbingEngineKind {
  EdgeTts = 'edge-tts',
  NanoAiTts = 'nano-ai-tts',
  IndexTts2 = 'index-tts-2',
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

const activeTab = ref<DubbingTab>(DubbingTab.Workflow)
const dubbingModels = ref<DubbingModel[]>([])
const voices = ref<DubbingVoiceOption[]>([])
const selectedEngine = ref<DubbingEngineKind>(DubbingEngineKind.EdgeTts)
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

const isTauriRuntime = () => '__TAURI_INTERNALS__' in window

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

const selectTab = (tab: DubbingTab) => {
  activeTab.value = tab
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

const handleKeydown = (event: KeyboardEvent) => {
  if (event.key === 'Escape') {
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
  window.addEventListener('keydown', handleKeydown)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown)
  stopPreviewAudio()
})
</script>
