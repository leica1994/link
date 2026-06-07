<template>
  <div class="page settings-page">
    <h1 class="page-title">设置</h1>

    <div class="settings-stack">
      <section class="settings-section" aria-labelledby="transcription-settings-title">
        <div id="transcription-settings-title" class="section-heading">
          <Captions aria-hidden="true" />
          <span>转录配置</span>
        </div>

        <div class="settings-panel">
          <button class="setting-row setting-row-button" type="button" @click="openTranscriptionModelDialog">
            <Bot class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <span class="setting-copy">
              <span class="setting-title">转录模型</span>
              <span class="setting-subtitle">语音识别模型</span>
            </span>
            <span class="setting-inline-action">
              <span class="setting-value">{{ transcriptionModelLabel }}</span>
              <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
            </span>
          </button>
        </div>
      </section>

      <section class="settings-section" aria-labelledby="llm-settings-title">
        <div id="llm-settings-title" class="section-heading">
          <Brain aria-hidden="true" />
          <span>LLM配置</span>
        </div>

        <div class="settings-panel">
          <button class="setting-row setting-row-button" type="button" @click="openLlmServiceDialog">
            <Server class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <span class="setting-copy">
              <span class="setting-title">LLM服务</span>
              <span class="setting-subtitle">用于字幕断句、字幕优化、字幕翻译</span>
            </span>
            <span class="setting-inline-action">
              <span class="setting-value">{{ llmServiceLabel }}</span>
              <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
            </span>
          </button>

          <div class="setting-row">
            <LinkIcon class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">Base URL</div>
              <div class="setting-subtitle">服务地址，需要包含 /v1</div>
            </div>
            <input
              v-model="llmBaseUrl"
              class="setting-control settings-input"
              type="url"
              placeholder="输入 Base URL"
              aria-label="Base URL"
            />
          </div>

          <div class="setting-row">
            <KeyRound class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">API Key</div>
              <div class="setting-subtitle">输入对应服务的 API Key</div>
            </div>
            <div class="setting-control api-key-field">
              <input
                v-model="llmApiKey"
                class="settings-input api-key-input"
                :type="isLlmApiKeyVisible ? 'text' : 'password'"
                autocomplete="off"
                placeholder="输入 API Key"
                aria-label="API Key"
              />
              <button
                class="api-key-visibility"
                type="button"
                :aria-label="isLlmApiKeyVisible ? '隐藏 API Key' : '显示 API Key'"
                @click="isLlmApiKeyVisible = !isLlmApiKeyVisible"
              >
                <EyeOff v-if="isLlmApiKeyVisible" :stroke-width="2.1" aria-hidden="true" />
                <Eye v-else :stroke-width="2.1" aria-hidden="true" />
              </button>
            </div>
          </div>

          <div class="setting-row">
            <Bot class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">模型</div>
              <div class="setting-subtitle">输入模型名称</div>
            </div>
            <input
              v-model="llmModel"
              class="setting-control settings-input"
              type="text"
              placeholder="输入模型名称"
              aria-label="模型"
            />
          </div>

          <button class="setting-row setting-row-button" type="button" @click="openReasoningEffortDialog">
            <Gauge class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <span class="setting-copy">
              <span class="setting-title">推理强度</span>
              <span class="setting-subtitle">模型思考等级，越高推理更充分但耗时和消耗更高</span>
            </span>
            <span class="setting-inline-action">
              <span class="setting-value">{{ reasoningEffortLabel }}</span>
              <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
            </span>
          </button>

          <div class="setting-row">
            <RefreshCw class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">是否流式输出</div>
              <div class="setting-subtitle">启用后将以流式方式获取模型响应</div>
            </div>
            <button
              class="setting-toggle"
              :class="{ active: isLlmStreaming }"
              type="button"
              :aria-pressed="isLlmStreaming"
              @click="isLlmStreaming = !isLlmStreaming"
            >
              <span class="setting-toggle-label">{{ isLlmStreaming ? '开' : '关' }}</span>
              <span class="setting-toggle-track" aria-hidden="true">
                <span class="setting-toggle-thumb" />
              </span>
            </button>
          </div>

          <div class="setting-row">
            <Plug class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">检查 LLM 连接</div>
              <div class="setting-subtitle">检查 API 连接是否正常，并获取模型列表</div>
            </div>
            <button class="settings-action" type="button" disabled>检查连接</button>
          </div>
        </div>
      </section>

      <section class="settings-section" aria-labelledby="translation-settings-title">
        <div id="translation-settings-title" class="section-heading">
          <Languages aria-hidden="true" />
          <span>翻译服务</span>
        </div>

        <div class="settings-panel">
          <button class="setting-row setting-row-button" type="button" @click="openTranslationServiceDialog">
            <Bot class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <span class="setting-copy">
              <span class="setting-title">翻译服务</span>
              <span class="setting-subtitle">选择翻译服务</span>
            </span>
            <span class="setting-inline-action">
              <span class="setting-value">{{ translationServiceLabel }}</span>
              <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
            </span>
          </button>

          <div class="setting-row">
            <Pencil class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">需要反思翻译</div>
              <div class="setting-subtitle">启用反思翻译可以提高翻译质量，但耗费更多时间和 token</div>
            </div>
            <button
              class="setting-toggle"
              :class="{ active: needsReflectionTranslation }"
              type="button"
              :aria-pressed="needsReflectionTranslation"
              @click="needsReflectionTranslation = !needsReflectionTranslation"
            >
              <span class="setting-toggle-label">{{ needsReflectionTranslation ? '开' : '关' }}</span>
              <span class="setting-toggle-track" aria-hidden="true">
                <span class="setting-toggle-thumb" />
              </span>
            </button>
          </div>

          <div class="setting-row">
            <ListChecks class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">批处理大小</div>
              <div class="setting-subtitle">每批处理字幕的数量</div>
            </div>
            <div class="setting-range-control">
              <span class="setting-range-value">{{ translationBatchSize }}</span>
              <input
                v-model.number="translationBatchSize"
                class="setting-range"
                type="range"
                min="10"
                max="100"
                step="10"
                aria-label="批处理大小"
              />
            </div>
          </div>

          <div class="setting-row">
            <Gauge class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">线程数</div>
              <div class="setting-subtitle">请求并行处理的数量，数值越大速度越快</div>
            </div>
            <div class="setting-range-control">
              <span class="setting-range-value">{{ translationThreadCount }}</span>
              <input
                v-model.number="translationThreadCount"
                class="setting-range"
                type="range"
                min="1"
                max="100"
                step="1"
                aria-label="线程数"
              />
            </div>
          </div>
        </div>
      </section>

      <section class="settings-section" aria-labelledby="personalization-settings-title">
        <div id="personalization-settings-title" class="section-heading">
          <SlidersHorizontal aria-hidden="true" />
          <span>个性化</span>
        </div>

        <div class="settings-panel">
          <div class="setting-row">
            <Moon class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">主题模式</div>
              <div class="setting-subtitle">{{ themeLabel }} · 立即生效</div>
            </div>
            <div class="theme-switch" role="group" aria-label="主题模式">
              <button
                class="theme-option"
                :class="{ active: currentTheme === 'light' }"
                type="button"
                :aria-pressed="currentTheme === 'light'"
                @click="setTheme('light')"
              >
                <Sun :stroke-width="2.1" aria-hidden="true" />
                浅色
              </button>
              <button
                class="theme-option"
                :class="{ active: currentTheme === 'dark' }"
                type="button"
                :aria-pressed="currentTheme === 'dark'"
                @click="setTheme('dark')"
              >
                <Moon :stroke-width="2.1" aria-hidden="true" />
                深色
              </button>
            </div>
          </div>

          <button class="setting-row setting-row-button" type="button">
            <CircleHelp class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <span class="setting-copy">
              <span class="setting-title">关于与支持</span>
              <span class="setting-subtitle">版本信息 · 帮助反馈</span>
            </span>
            <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
          </button>
        </div>
      </section>
    </div>

    <Teleport to="body">
      <div
        v-if="isTranscriptionModelDialogOpen"
        class="dialog-backdrop"
        role="presentation"
        @click.self="closeTranscriptionModelDialog"
      >
        <section
          class="settings-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="transcription-model-dialog-title"
        >
          <h2 id="transcription-model-dialog-title" class="dialog-title">转录模型</h2>
          <div class="dialog-options" role="radiogroup" aria-label="转录模型">
            <button
              v-for="option in transcriptionModelOptions"
              :key="option.value"
              class="dialog-option"
              :class="{ active: selectedTranscriptionModel === option.value }"
              type="button"
              role="radio"
              :aria-checked="selectedTranscriptionModel === option.value"
              @click="selectTranscriptionModel(option.value)"
            >
              <span class="dialog-radio" aria-hidden="true" />
              <span>{{ option.label }}</span>
            </button>
          </div>
        </section>
      </div>

      <div
        v-if="isLlmServiceDialogOpen"
        class="dialog-backdrop"
        role="presentation"
        @click.self="closeLlmServiceDialog"
      >
        <section
          class="settings-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="llm-service-dialog-title"
        >
          <h2 id="llm-service-dialog-title" class="dialog-title">LLM服务</h2>
          <div class="dialog-options" role="radiogroup" aria-label="LLM服务">
            <button
              v-for="option in llmServiceOptions"
              :key="option.value"
              class="dialog-option"
              :class="{ active: selectedLlmService === option.value }"
              type="button"
              role="radio"
              :aria-checked="selectedLlmService === option.value"
              @click="selectLlmService(option.value)"
            >
              <span class="dialog-radio" aria-hidden="true" />
              <span>{{ option.label }}</span>
            </button>
          </div>
        </section>
      </div>

      <div
        v-if="isReasoningEffortDialogOpen"
        class="dialog-backdrop"
        role="presentation"
        @click.self="closeReasoningEffortDialog"
      >
        <section
          class="settings-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="reasoning-effort-dialog-title"
        >
          <h2 id="reasoning-effort-dialog-title" class="dialog-title">推理强度</h2>
          <div class="dialog-options" role="radiogroup" aria-label="推理强度">
            <button
              v-for="option in reasoningEffortOptions"
              :key="option.value"
              class="dialog-option"
              :class="{ active: selectedReasoningEffort === option.value }"
              type="button"
              role="radio"
              :aria-checked="selectedReasoningEffort === option.value"
              @click="selectReasoningEffort(option.value)"
            >
              <span class="dialog-radio" aria-hidden="true" />
              <span>{{ option.label }}</span>
            </button>
          </div>
        </section>
      </div>

      <div
        v-if="isTranslationServiceDialogOpen"
        class="dialog-backdrop"
        role="presentation"
        @click.self="closeTranslationServiceDialog"
      >
        <section
          class="settings-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="translation-service-dialog-title"
        >
          <h2 id="translation-service-dialog-title" class="dialog-title">翻译服务</h2>
          <div class="dialog-options" role="radiogroup" aria-label="翻译服务">
            <button
              v-for="option in translationServiceOptions"
              :key="option.value"
              class="dialog-option"
              :class="{ active: selectedTranslationService === option.value }"
              type="button"
              role="radio"
              :aria-checked="selectedTranslationService === option.value"
              @click="selectTranslationService(option.value)"
            >
              <span class="dialog-radio" aria-hidden="true" />
              <span>{{ option.label }}</span>
            </button>
          </div>
        </section>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import {
  Bot,
  Brain,
  Captions,
  ChevronRight,
  CircleHelp,
  Eye,
  EyeOff,
  Gauge,
  KeyRound,
  Languages,
  Link as LinkIcon,
  ListChecks,
  Moon,
  Pencil,
  Plug,
  RefreshCw,
  Server,
  SlidersHorizontal,
  Sun,
} from 'lucide-vue-next'
import { useTheme } from '../composables/useTheme'

const { currentTheme, setTheme, themeLabel } = useTheme()

enum TranscriptionModel {
  Bilibili = 'bilibili',
}

const transcriptionModelOptions = [
  { value: TranscriptionModel.Bilibili, label: 'B站转录' },
] as const

enum LlmService {
  OpenAI = 'openai',
  OpenAIResponses = 'openai-responses',
  Anthropic = 'anthropic',
}

enum ReasoningEffort {
  UltraHigh = 'ultra-high',
  High = 'high',
  Medium = 'medium',
  Low = 'low',
  Off = 'off',
}

enum TranslationService {
  Llm = 'llm',
  DeepLx = 'deeplx',
  Microsoft = 'microsoft',
  Google = 'google',
}

const llmServiceOptions = [
  { value: LlmService.OpenAI, label: 'OpenAI' },
  { value: LlmService.OpenAIResponses, label: 'OpenAI Responses' },
  { value: LlmService.Anthropic, label: 'Anthropic' },
] as const

const reasoningEffortOptions = [
  { value: ReasoningEffort.UltraHigh, label: '超高' },
  { value: ReasoningEffort.High, label: '高' },
  { value: ReasoningEffort.Medium, label: '中' },
  { value: ReasoningEffort.Low, label: '低' },
  { value: ReasoningEffort.Off, label: '关闭' },
] as const

const translationServiceOptions = [
  { value: TranslationService.Llm, label: 'LLM 大模型翻译' },
  { value: TranslationService.DeepLx, label: 'DeepLx 翻译' },
  { value: TranslationService.Microsoft, label: '微软翻译' },
  { value: TranslationService.Google, label: '谷歌翻译' },
] as const

const selectedTranscriptionModel = ref<TranscriptionModel>(TranscriptionModel.Bilibili)
const isTranscriptionModelDialogOpen = ref(false)
const selectedLlmService = ref<LlmService>(LlmService.OpenAI)
const isLlmServiceDialogOpen = ref(false)
const llmApiKey = ref('')
const isLlmApiKeyVisible = ref(false)
const llmBaseUrl = ref('')
const llmModel = ref('')
const selectedReasoningEffort = ref<ReasoningEffort>(ReasoningEffort.Off)
const isReasoningEffortDialogOpen = ref(false)
const isLlmStreaming = ref(true)
const selectedTranslationService = ref<TranslationService>(TranslationService.Llm)
const isTranslationServiceDialogOpen = ref(false)
const needsReflectionTranslation = ref(true)
const translationBatchSize = ref(30)
const translationThreadCount = ref(10)

const transcriptionModelLabel = computed(() => {
  return transcriptionModelOptions.find((option) => option.value === selectedTranscriptionModel.value)?.label ?? ''
})

const llmServiceLabel = computed(() => {
  return llmServiceOptions.find((option) => option.value === selectedLlmService.value)?.label ?? ''
})

const reasoningEffortLabel = computed(() => {
  return reasoningEffortOptions.find((option) => option.value === selectedReasoningEffort.value)?.label ?? ''
})

const translationServiceLabel = computed(() => {
  return translationServiceOptions.find((option) => option.value === selectedTranslationService.value)?.label ?? ''
})

const openTranscriptionModelDialog = () => {
  isTranscriptionModelDialogOpen.value = true
}

const closeTranscriptionModelDialog = () => {
  isTranscriptionModelDialogOpen.value = false
}

const selectTranscriptionModel = (model: TranscriptionModel) => {
  selectedTranscriptionModel.value = model
  closeTranscriptionModelDialog()
}

const openLlmServiceDialog = () => {
  isLlmServiceDialogOpen.value = true
}

const closeLlmServiceDialog = () => {
  isLlmServiceDialogOpen.value = false
}

const selectLlmService = (service: LlmService) => {
  selectedLlmService.value = service
  closeLlmServiceDialog()
}

const openReasoningEffortDialog = () => {
  isReasoningEffortDialogOpen.value = true
}

const closeReasoningEffortDialog = () => {
  isReasoningEffortDialogOpen.value = false
}

const selectReasoningEffort = (effort: ReasoningEffort) => {
  selectedReasoningEffort.value = effort
  closeReasoningEffortDialog()
}

const openTranslationServiceDialog = () => {
  isTranslationServiceDialogOpen.value = true
}

const closeTranslationServiceDialog = () => {
  isTranslationServiceDialogOpen.value = false
}

const selectTranslationService = (service: TranslationService) => {
  selectedTranslationService.value = service
  closeTranslationServiceDialog()
}

const handleKeydown = (event: KeyboardEvent) => {
  if (event.key === 'Escape') {
    closeTranscriptionModelDialog()
    closeLlmServiceDialog()
    closeReasoningEffortDialog()
    closeTranslationServiceDialog()
  }
}

window.addEventListener('keydown', handleKeydown)

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown)
})
</script>
