<template>
  <div class="page translate-page subtitle-style-page">
    <header class="translate-header">
      <h1 class="page-title">字幕样式</h1>
    </header>

    <main class="translate-workspace subtitle-style-workspace">
      <div v-if="styleError" class="translate-alert subtitle-style-alert" role="alert">
        <CircleAlert :stroke-width="2.1" aria-hidden="true" />
        <span>{{ styleError }}</span>
      </div>

      <div class="subtitle-style-layout">
        <aside class="subtitle-style-sidebar">
          <section class="settings-section" aria-labelledby="style-list-title">
            <div id="style-list-title" class="section-heading">
              <List aria-hidden="true" />
              <span>样式预设</span>
            </div>

            <div class="settings-panel subtitle-style-list-panel">
              <div v-if="styles.length === 0" class="subtitle-style-empty">
                <Palette :stroke-width="2.1" aria-hidden="true" />
                <span>暂无样式</span>
              </div>

              <div v-else class="subtitle-style-list">
                <button
                  v-for="style in styles"
                  :key="style.id"
                  class="subtitle-style-item"
                  :class="{ active: selectedStyleId === style.id }"
                  type="button"
                  @click="selectStyle(style)"
                >
                  <span class="subtitle-style-item-main">
                    <Palette class="subtitle-style-item-icon" :stroke-width="2.1" aria-hidden="true" />
                    <span class="subtitle-style-item-copy">
                      <span class="subtitle-style-item-name">{{ style.name }}</span>
                      <span class="subtitle-style-item-meta">
                        {{ getOptionLabel(renderModeOptions, style.renderMode) }}
                        <span v-if="style.isDefault"> · 默认</span>
                      </span>
                    </span>
                  </span>
                  <Check
                    v-if="selectedStyleId === style.id"
                    class="subtitle-style-item-check"
                    :stroke-width="2.4"
                    aria-hidden="true"
                  />
                </button>
              </div>

              <div class="subtitle-style-actions">
                <button class="settings-action" type="button" @click="openCreateDialog">
                  <Plus :stroke-width="2.1" aria-hidden="true" />
                  <span>新建样式</span>
                </button>
                <button
                  class="settings-action subtitle-style-danger-action"
                  type="button"
                  :disabled="!selectedStyleId || draftStyle.isDefault"
                  @click="openDeleteDialog"
                >
                  <Trash2 :stroke-width="2.1" aria-hidden="true" />
                  <span>删除样式</span>
                </button>
              </div>
            </div>
          </section>

          <section class="settings-section" aria-labelledby="preview-settings-title">
            <div id="preview-settings-title" class="section-heading">
              <SlidersHorizontal aria-hidden="true" />
              <span>预览设置</span>
            </div>

            <div class="settings-panel">
              <button class="setting-row setting-row-button compact" type="button" @click="openChoiceDialog('renderMode')">
                <span class="setting-copy">
                  <span class="setting-title">渲染模式</span>
                  <span class="setting-subtitle">选择字幕绘制方式</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ renderModeLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>

              <button class="setting-row setting-row-button compact" type="button" @click="openChoiceDialog('subtitleLayout')">
                <span class="setting-copy">
                  <span class="setting-title">字幕排布</span>
                  <span class="setting-subtitle">设置原文与译文位置</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ subtitleLayoutLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>

              <button class="setting-row setting-row-button compact" type="button" @click="openChoiceDialog('previewTextMode')">
                <span class="setting-copy">
                  <span class="setting-title">预览文字</span>
                  <span class="setting-subtitle">切换字幕长度样例</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ previewTextModeLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>
            </div>
          </section>
        </aside>

        <section class="settings-section subtitle-preview-section" aria-labelledby="subtitle-preview-title">
          <div id="subtitle-preview-title" class="section-heading">
            <ImageIcon aria-hidden="true" />
            <span>效果预览</span>
          </div>

          <div class="settings-panel subtitle-preview-panel">
            <div class="subtitle-preview-toolbar">
              <div class="subtitle-preview-copy">
                <span class="subtitle-preview-name">{{ draftStyle.name || '字幕样式' }}</span>
                <span class="subtitle-preview-detail">
                  {{ renderModeLabel }} · {{ subtitleLayoutLabel }} · 1280×720
                </span>
              </div>
              <span class="subtitle-preview-badge">{{ previewTextModeLabel }}</span>
            </div>

            <div ref="previewViewportRef" class="subtitle-preview-viewport">
              <div class="subtitle-preview-stage" :style="previewStageStyle">
                <img class="subtitle-preview-image" :src="previewBackgroundUrl" alt="" draggable="false" />
                <div
                  v-if="draftStyle.renderMode === RenderMode.Ass"
                  class="subtitle-preview-overlay ass"
                  :style="assOverlayStyle"
                >
                  <span
                    v-for="line in previewLines"
                    :key="line.role"
                    class="subtitle-preview-ass-line"
                    :style="line.role === 'primary' ? primaryAssStyle : secondaryAssStyle"
                  >
                    {{ line.text }}
                  </span>
                </div>
                <div v-else class="subtitle-preview-overlay rounded" :style="roundedOverlayStyle">
                  <span
                    v-for="line in previewLines"
                    :key="line.role"
                    class="subtitle-preview-rounded-line"
                    :style="roundedLineStyle"
                  >
                    {{ line.text }}
                  </span>
                </div>
              </div>
            </div>
          </div>
        </section>
      </div>

      <div class="subtitle-style-controls">
        <section class="settings-section" aria-labelledby="ass-primary-title">
          <div id="ass-primary-title" class="section-heading">
            <Type aria-hidden="true" />
            <span>主字幕样式</span>
          </div>

          <div class="settings-panel">
            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">字体</span>
                <span class="setting-subtitle">主字幕字体</span>
              </span>
              <input
                v-model="draftStyle.primaryFontName"
                class="settings-input subtitle-text-input"
                type="text"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">字号</span>
                <span class="setting-subtitle">主字幕大小</span>
              </span>
              <input
                v-model.number="draftStyle.primaryFontSize"
                class="settings-input subtitle-number-input"
                type="number"
                min="8"
                max="200"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">文字颜色</span>
                <span class="setting-subtitle">主字幕文字颜色</span>
              </span>
              <input
                v-model="draftStyle.primaryColor"
                class="subtitle-color-input"
                type="color"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">描边颜色</span>
                <span class="setting-subtitle">主字幕边框颜色</span>
              </span>
              <input
                v-model="draftStyle.primaryOutlineColor"
                class="subtitle-color-input"
                type="color"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">描边宽度</span>
                <span class="setting-subtitle">主字幕边框粗细</span>
              </span>
              <input
                v-model.number="draftStyle.primaryOutlineWidth"
                class="settings-input subtitle-number-input"
                type="number"
                min="0"
                max="10"
                step="0.1"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">字符间距</span>
                <span class="setting-subtitle">主字幕字符间距</span>
              </span>
              <input
                v-model.number="draftStyle.primarySpacing"
                class="settings-input subtitle-number-input"
                type="number"
                min="0"
                max="20"
                step="0.1"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">底部边距</span>
                <span class="setting-subtitle">字幕距画面底部距离</span>
              </span>
              <input
                v-model.number="draftStyle.primaryMarginBottom"
                class="settings-input subtitle-number-input"
                type="number"
                min="0"
                max="240"
                @change="saveCurrentStyle"
              />
            </label>
          </div>
        </section>

        <section class="settings-section" aria-labelledby="ass-secondary-title">
          <div id="ass-secondary-title" class="section-heading">
            <Captions aria-hidden="true" />
            <span>副字幕样式</span>
          </div>

          <div class="settings-panel">
            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">字体</span>
                <span class="setting-subtitle">副字幕字体</span>
              </span>
              <input
                v-model="draftStyle.secondaryFontName"
                class="settings-input subtitle-text-input"
                type="text"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">字号</span>
                <span class="setting-subtitle">副字幕大小</span>
              </span>
              <input
                v-model.number="draftStyle.secondaryFontSize"
                class="settings-input subtitle-number-input"
                type="number"
                min="8"
                max="200"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">文字颜色</span>
                <span class="setting-subtitle">副字幕文字颜色</span>
              </span>
              <input
                v-model="draftStyle.secondaryColor"
                class="subtitle-color-input"
                type="color"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">描边颜色</span>
                <span class="setting-subtitle">副字幕边框颜色</span>
              </span>
              <input
                v-model="draftStyle.secondaryOutlineColor"
                class="subtitle-color-input"
                type="color"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">描边宽度</span>
                <span class="setting-subtitle">副字幕边框粗细</span>
              </span>
              <input
                v-model.number="draftStyle.secondaryOutlineWidth"
                class="settings-input subtitle-number-input"
                type="number"
                min="0"
                max="10"
                step="0.1"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">字符间距</span>
                <span class="setting-subtitle">副字幕字符间距</span>
              </span>
              <input
                v-model.number="draftStyle.secondarySpacing"
                class="settings-input subtitle-number-input"
                type="number"
                min="0"
                max="20"
                step="0.1"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">垂直间距</span>
                <span class="setting-subtitle">主副字幕之间的间距</span>
              </span>
              <input
                v-model.number="draftStyle.verticalSpacing"
                class="settings-input subtitle-number-input"
                type="number"
                min="0"
                max="120"
                @change="saveCurrentStyle"
              />
            </label>
          </div>
        </section>

        <section class="settings-section" aria-labelledby="rounded-style-title">
          <div id="rounded-style-title" class="section-heading">
            <PanelBottom aria-hidden="true" />
            <span>圆角背景样式</span>
          </div>

          <div class="settings-panel">
            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">字体</span>
                <span class="setting-subtitle">圆角字幕字体</span>
              </span>
              <input
                v-model="draftStyle.roundedFontName"
                class="settings-input subtitle-text-input"
                type="text"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">字号</span>
                <span class="setting-subtitle">圆角字幕大小</span>
              </span>
              <input
                v-model.number="draftStyle.roundedFontSize"
                class="settings-input subtitle-number-input"
                type="number"
                min="12"
                max="120"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">文字颜色</span>
                <span class="setting-subtitle">圆角字幕文字颜色</span>
              </span>
              <input
                v-model="draftStyle.roundedTextColor"
                class="subtitle-color-input"
                type="color"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">背景颜色</span>
                <span class="setting-subtitle">支持 #RRGGBBAA 透明度</span>
              </span>
              <input
                v-model="draftStyle.roundedBackgroundColor"
                class="settings-input subtitle-hex-input"
                type="text"
                spellcheck="false"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">圆角半径</span>
                <span class="setting-subtitle">背景圆角大小</span>
              </span>
              <input
                v-model.number="draftStyle.roundedCornerRadius"
                class="settings-input subtitle-number-input"
                type="number"
                min="0"
                max="60"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">水平内边距</span>
                <span class="setting-subtitle">文字左右留白</span>
              </span>
              <input
                v-model.number="draftStyle.roundedPaddingX"
                class="settings-input subtitle-number-input"
                type="number"
                min="0"
                max="120"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">垂直内边距</span>
                <span class="setting-subtitle">文字上下留白</span>
              </span>
              <input
                v-model.number="draftStyle.roundedPaddingY"
                class="settings-input subtitle-number-input"
                type="number"
                min="0"
                max="80"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">底部边距</span>
                <span class="setting-subtitle">背景距底部距离</span>
              </span>
              <input
                v-model.number="draftStyle.roundedMarginBottom"
                class="settings-input subtitle-number-input"
                type="number"
                min="0"
                max="240"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">行间距</span>
                <span class="setting-subtitle">多行字幕间距</span>
              </span>
              <input
                v-model.number="draftStyle.roundedLineSpacing"
                class="settings-input subtitle-number-input"
                type="number"
                min="0"
                max="60"
                @change="saveCurrentStyle"
              />
            </label>

            <label class="setting-row compact">
              <span class="setting-copy">
                <span class="setting-title">字符间距</span>
                <span class="setting-subtitle">圆角字幕字符间距</span>
              </span>
              <input
                v-model.number="draftStyle.roundedLetterSpacing"
                class="settings-input subtitle-number-input"
                type="number"
                min="0"
                max="20"
                @change="saveCurrentStyle"
              />
            </label>
          </div>
        </section>
      </div>
    </main>

    <Teleport to="body">
      <div v-if="activeChoiceDialog" class="dialog-backdrop" role="presentation" @click.self="closeChoiceDialog">
        <section
          class="settings-dialog subtitle-choice-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="subtitle-choice-dialog-title"
        >
          <h2 id="subtitle-choice-dialog-title" class="dialog-title">{{ choiceDialogTitle }}</h2>
          <div class="dialog-options" role="radiogroup" :aria-label="choiceDialogTitle">
            <button
              v-for="option in choiceDialogOptions"
              :key="option.value"
              class="dialog-option"
              :class="{ active: choiceDialogValue === option.value }"
              type="button"
              role="radio"
              :aria-checked="choiceDialogValue === option.value"
              @click="selectChoice(option.value)"
            >
              <span class="dialog-radio" aria-hidden="true" />
              <span>{{ option.label }}</span>
            </button>
          </div>
        </section>
      </div>
    </Teleport>

    <Teleport to="body">
      <div v-if="showCreateDialog" class="dialog-backdrop" role="presentation" @click.self="closeCreateDialog">
        <section
          class="settings-dialog subtitle-name-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="create-style-dialog-title"
        >
          <h2 id="create-style-dialog-title" class="dialog-title">新建样式</h2>
          <label class="subtitle-dialog-field">
            <span class="subtitle-dialog-label">样式名称</span>
            <input
              v-model="newStyleName"
              class="settings-input subtitle-dialog-input"
              type="text"
              placeholder="输入样式名称"
              autofocus
              @keyup.enter="confirmCreateStyle"
            />
          </label>
          <div class="subtitle-dialog-actions">
            <button class="settings-action" type="button" @click="closeCreateDialog">取消</button>
            <button class="settings-action" type="button" :disabled="!newStyleName.trim()" @click="confirmCreateStyle">
              创建
            </button>
          </div>
        </section>
      </div>
    </Teleport>

    <Teleport to="body">
      <div v-if="showDeleteDialog" class="dialog-backdrop" role="presentation" @click.self="closeDeleteDialog">
        <section
          class="settings-dialog subtitle-name-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="delete-style-dialog-title"
        >
          <h2 id="delete-style-dialog-title" class="dialog-title">删除样式</h2>
          <p class="subtitle-dialog-copy">确定要删除样式「{{ draftStyle.name }}」吗？此操作无法撤销。</p>
          <div class="subtitle-dialog-actions">
            <button class="settings-action" type="button" @click="closeDeleteDialog">取消</button>
            <button class="settings-action subtitle-style-danger-action" type="button" @click="confirmDeleteStyle">
              删除
            </button>
          </div>
        </section>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, type CSSProperties } from 'vue'
import {
  Captions,
  Check,
  ChevronRight,
  CircleAlert,
  Image as ImageIcon,
  List,
  Palette,
  PanelBottom,
  Plus,
  SlidersHorizontal,
  Trash2,
  Type,
} from 'lucide-vue-next'
import previewBackgroundUrl from '../assets/subtitle-preview-default.jpg'

defineOptions({ name: 'SubtitleStyle' })

enum RenderMode {
  Ass = 'ass',
  Rounded = 'rounded',
}

enum SubtitleLayout {
  TargetAbove = 'target-above',
  SourceAbove = 'source-above',
  TargetOnly = 'target-only',
  SourceOnly = 'source-only',
}

enum PreviewTextMode {
  Long = 'long',
  Medium = 'medium',
  Short = 'short',
}

type ChoiceDialog = 'renderMode' | 'subtitleLayout' | 'previewTextMode'

type Option<T extends string> = {
  value: T
  label: string
}

type PreviewLine = {
  role: 'primary' | 'secondary'
  text: string
}

type SubtitleStyle = {
  id: string
  name: string
  isDefault: boolean
  renderMode: RenderMode
  subtitleLayout: SubtitleLayout
  previewTextMode: PreviewTextMode
  primaryFontName: string
  primaryFontSize: number
  primaryColor: string
  primaryOutlineColor: string
  primaryOutlineWidth: number
  primarySpacing: number
  primaryMarginBottom: number
  secondaryFontName: string
  secondaryFontSize: number
  secondaryColor: string
  secondaryOutlineColor: string
  secondaryOutlineWidth: number
  secondarySpacing: number
  verticalSpacing: number
  roundedFontName: string
  roundedFontSize: number
  roundedTextColor: string
  roundedBackgroundColor: string
  roundedCornerRadius: number
  roundedPaddingX: number
  roundedPaddingY: number
  roundedMarginBottom: number
  roundedLineSpacing: number
  roundedLetterSpacing: number
  createdAt: string
  updatedAt: string
}

const previewWidth = 1280
const previewHeight = 720

const renderModeOptions: readonly Option<RenderMode>[] = [
  { value: RenderMode.Ass, label: 'ASS 样式' },
  { value: RenderMode.Rounded, label: '圆角背景' },
] as const

const subtitleLayoutOptions: readonly Option<SubtitleLayout>[] = [
  { value: SubtitleLayout.TargetAbove, label: '译文在上' },
  { value: SubtitleLayout.SourceAbove, label: '原文在上' },
  { value: SubtitleLayout.TargetOnly, label: '仅译文' },
  { value: SubtitleLayout.SourceOnly, label: '仅原文' },
] as const

const previewTextModeOptions: readonly Option<PreviewTextMode>[] = [
  { value: PreviewTextMode.Medium, label: '中文本' },
  { value: PreviewTextMode.Long, label: '长文本' },
  { value: PreviewTextMode.Short, label: '短文本' },
] as const

const previewTextSamples: Record<PreviewTextMode, { source: string; target: string }> = {
  [PreviewTextMode.Long]: {
    source: 'This is a long text for testing subtitle preview, text wrapping, and style settings.',
    target: '这是一段用于测试字幕预览、自动换行以及样式设置的较长文本内容。',
  },
  [PreviewTextMode.Medium]: {
    source: 'Welcome to apply for the prestigious South China Normal University!',
    target: '欢迎报考百年名校华南师范大学',
  },
  [PreviewTextMode.Short]: {
    source: 'Elementary school students know this',
    target: '小学二年级的都知道',
  },
}

const styles = ref<SubtitleStyle[]>([])
const selectedStyleId = ref('')
const draftStyle = ref<SubtitleStyle>(createDefaultStyle())
const styleError = ref('')
const activeChoiceDialog = ref<ChoiceDialog | null>(null)
const showCreateDialog = ref(false)
const newStyleName = ref('')
const showDeleteDialog = ref(false)
const previewViewportRef = ref<HTMLElement | null>(null)
const previewScale = ref(1)

let previewObserver: ResizeObserver | null = null

const getOptionLabel = <T extends string>(options: readonly Option<T>[], value: T) => {
  return options.find((option) => option.value === value)?.label ?? ''
}

const renderModeLabel = computed(() => getOptionLabel(renderModeOptions, draftStyle.value.renderMode))
const subtitleLayoutLabel = computed(() => getOptionLabel(subtitleLayoutOptions, draftStyle.value.subtitleLayout))
const previewTextModeLabel = computed(() => getOptionLabel(previewTextModeOptions, draftStyle.value.previewTextMode))

const choiceDialogTitle = computed(() => {
  if (activeChoiceDialog.value === 'renderMode') {
    return '渲染模式'
  }
  if (activeChoiceDialog.value === 'subtitleLayout') {
    return '字幕排布'
  }
  return '预览文字'
})

const choiceDialogOptions = computed(() => {
  if (activeChoiceDialog.value === 'renderMode') {
    return renderModeOptions
  }
  if (activeChoiceDialog.value === 'subtitleLayout') {
    return subtitleLayoutOptions
  }
  return previewTextModeOptions
})

const choiceDialogValue = computed(() => {
  if (activeChoiceDialog.value === 'renderMode') {
    return draftStyle.value.renderMode
  }
  if (activeChoiceDialog.value === 'subtitleLayout') {
    return draftStyle.value.subtitleLayout
  }
  return draftStyle.value.previewTextMode
})

const previewLines = computed<PreviewLine[]>(() => {
  const sample = previewTextSamples[draftStyle.value.previewTextMode] ?? previewTextSamples[PreviewTextMode.Medium]

  if (draftStyle.value.subtitleLayout === SubtitleLayout.TargetOnly) {
    return [{ role: 'primary', text: sample.target }]
  }
  if (draftStyle.value.subtitleLayout === SubtitleLayout.SourceOnly) {
    return [{ role: 'primary', text: sample.source }]
  }
  if (draftStyle.value.subtitleLayout === SubtitleLayout.SourceAbove) {
    return [
      { role: 'primary', text: sample.source },
      { role: 'secondary', text: sample.target },
    ]
  }

  return [
    { role: 'primary', text: sample.target },
    { role: 'secondary', text: sample.source },
  ]
})

const previewStageStyle = computed<CSSProperties>(() => ({
  width: `${previewWidth}px`,
  height: `${previewHeight}px`,
  transform: `scale(${previewScale.value})`,
}))

const assOverlayStyle = computed<CSSProperties>(() => ({
  bottom: `${clampNumber(draftStyle.value.primaryMarginBottom, 0, 260)}px`,
  gap: `${clampNumber(draftStyle.value.verticalSpacing, 0, 120)}px`,
}))

const roundedOverlayStyle = computed<CSSProperties>(() => ({
  bottom: `${clampNumber(draftStyle.value.roundedMarginBottom, 0, 260)}px`,
  gap: `${clampNumber(draftStyle.value.roundedLineSpacing, 0, 80)}px`,
}))

const primaryAssStyle = computed<CSSProperties>(() =>
  buildAssLineStyle({
    fontFamily: draftStyle.value.primaryFontName,
    fontSize: draftStyle.value.primaryFontSize,
    color: draftStyle.value.primaryColor,
    outlineColor: draftStyle.value.primaryOutlineColor,
    outlineWidth: draftStyle.value.primaryOutlineWidth,
    letterSpacing: draftStyle.value.primarySpacing,
  }),
)

const secondaryAssStyle = computed<CSSProperties>(() =>
  buildAssLineStyle({
    fontFamily: draftStyle.value.secondaryFontName,
    fontSize: draftStyle.value.secondaryFontSize,
    color: draftStyle.value.secondaryColor,
    outlineColor: draftStyle.value.secondaryOutlineColor,
    outlineWidth: draftStyle.value.secondaryOutlineWidth,
    letterSpacing: draftStyle.value.secondarySpacing,
  }),
)

const roundedLineStyle = computed<CSSProperties>(() => ({
  maxWidth: '1080px',
  borderRadius: `${clampNumber(draftStyle.value.roundedCornerRadius, 0, 80)}px`,
  background: normalizeCssColor(draftStyle.value.roundedBackgroundColor, '#191919CC'),
  color: normalizeCssColor(draftStyle.value.roundedTextColor, '#FFFFFF'),
  fontFamily: quoteFontFamily(draftStyle.value.roundedFontName),
  fontSize: `${clampNumber(draftStyle.value.roundedFontSize, 8, 160)}px`,
  letterSpacing: `${clampNumber(draftStyle.value.roundedLetterSpacing, 0, 40)}px`,
  padding: `${clampNumber(draftStyle.value.roundedPaddingY, 0, 100)}px ${clampNumber(
    draftStyle.value.roundedPaddingX,
    0,
    160,
  )}px`,
}))

const loadStyles = async () => {
  try {
    const result = await invoke<SubtitleStyle[]>('list_subtitle_styles')
    styles.value = result.map(normalizeStyle)

    const selected = styles.value.find((style) => style.id === selectedStyleId.value) ?? styles.value[0]
    if (selected) {
      selectStyle(selected)
    }
  } catch (error) {
    styleError.value = `加载字幕样式失败: ${stringifyError(error)}`
  }
}

const selectStyle = (style: SubtitleStyle) => {
  selectedStyleId.value = style.id
  draftStyle.value = cloneStyle(normalizeStyle(style))
  styleError.value = ''
}

const saveCurrentStyle = async () => {
  if (!selectedStyleId.value) {
    return
  }

  try {
    const updated = await invoke<SubtitleStyle>('update_subtitle_style', {
      request: toUpdateRequest(draftStyle.value),
    })
    const normalized = normalizeStyle(updated)
    draftStyle.value = cloneStyle(normalized)
    selectedStyleId.value = normalized.id
    const index = styles.value.findIndex((style) => style.id === normalized.id)
    if (index >= 0) {
      styles.value[index] = normalized
    }
    styleError.value = ''
  } catch (error) {
    styleError.value = `保存字幕样式失败: ${stringifyError(error)}`
  }
}

const openChoiceDialog = (dialog: ChoiceDialog) => {
  activeChoiceDialog.value = dialog
}

const closeChoiceDialog = () => {
  activeChoiceDialog.value = null
}

const selectChoice = (value: string) => {
  if (activeChoiceDialog.value === 'renderMode') {
    draftStyle.value.renderMode = readOptionValue(value, renderModeOptions, RenderMode.Ass)
  } else if (activeChoiceDialog.value === 'subtitleLayout') {
    draftStyle.value.subtitleLayout = readOptionValue(value, subtitleLayoutOptions, SubtitleLayout.TargetAbove)
  } else if (activeChoiceDialog.value === 'previewTextMode') {
    draftStyle.value.previewTextMode = readOptionValue(value, previewTextModeOptions, PreviewTextMode.Medium)
  }

  closeChoiceDialog()
  void saveCurrentStyle()
}

const openCreateDialog = () => {
  newStyleName.value = ''
  showCreateDialog.value = true
}

const closeCreateDialog = () => {
  showCreateDialog.value = false
  newStyleName.value = ''
}

const confirmCreateStyle = async () => {
  const name = newStyleName.value.trim()
  if (!name) {
    return
  }

  try {
    const created = await invoke<SubtitleStyle>('create_subtitle_style', {
      request: toCreateRequest({ ...draftStyle.value, name }),
    })
    const normalized = normalizeStyle(created)
    styles.value.push(normalized)
    selectStyle(normalized)
    closeCreateDialog()
  } catch (error) {
    styleError.value = `创建字幕样式失败: ${stringifyError(error)}`
  }
}

const openDeleteDialog = () => {
  if (!selectedStyleId.value || draftStyle.value.isDefault) {
    return
  }
  showDeleteDialog.value = true
}

const closeDeleteDialog = () => {
  showDeleteDialog.value = false
}

const confirmDeleteStyle = async () => {
  if (!selectedStyleId.value || draftStyle.value.isDefault) {
    return
  }

  try {
    const deletedId = selectedStyleId.value
    await invoke('delete_subtitle_style', { id: deletedId })
    styles.value = styles.value.filter((style) => style.id !== deletedId)
    const nextStyle = styles.value[0]
    if (nextStyle) {
      selectStyle(nextStyle)
    } else {
      selectedStyleId.value = ''
      draftStyle.value = createDefaultStyle()
    }
    closeDeleteDialog()
  } catch (error) {
    styleError.value = `删除字幕样式失败: ${stringifyError(error)}`
  }
}

const updatePreviewScale = () => {
  const element = previewViewportRef.value
  if (!element) {
    return
  }

  previewScale.value = Math.min(1, element.clientWidth / previewWidth)
}

const handleKeydown = (event: KeyboardEvent) => {
  if (event.key !== 'Escape') {
    return
  }

  if (activeChoiceDialog.value) {
    closeChoiceDialog()
  } else if (showCreateDialog.value) {
    closeCreateDialog()
  } else if (showDeleteDialog.value) {
    closeDeleteDialog()
  }
}

onMounted(() => {
  void loadStyles()
  window.addEventListener('keydown', handleKeydown)

  void nextTick(() => {
    updatePreviewScale()
    if (previewViewportRef.value) {
      previewObserver = new ResizeObserver(updatePreviewScale)
      previewObserver.observe(previewViewportRef.value)
    }
  })
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown)
  previewObserver?.disconnect()
})

function createDefaultStyle(): SubtitleStyle {
  const now = new Date().toISOString()

  return {
    id: '',
    name: '默认样式',
    isDefault: true,
    renderMode: RenderMode.Ass,
    subtitleLayout: SubtitleLayout.TargetAbove,
    previewTextMode: PreviewTextMode.Medium,
    primaryFontName: 'Microsoft YaHei',
    primaryFontSize: 48,
    primaryColor: '#FFFFFF',
    primaryOutlineColor: '#000000',
    primaryOutlineWidth: 2,
    primarySpacing: 0,
    primaryMarginBottom: 48,
    secondaryFontName: 'Microsoft YaHei',
    secondaryFontSize: 36,
    secondaryColor: '#FFFFFF',
    secondaryOutlineColor: '#000000',
    secondaryOutlineWidth: 2,
    secondarySpacing: 0,
    verticalSpacing: 15,
    roundedFontName: 'Microsoft YaHei',
    roundedFontSize: 34,
    roundedTextColor: '#FFFFFF',
    roundedBackgroundColor: '#191919CC',
    roundedCornerRadius: 14,
    roundedPaddingX: 24,
    roundedPaddingY: 14,
    roundedMarginBottom: 60,
    roundedLineSpacing: 10,
    roundedLetterSpacing: 0,
    createdAt: now,
    updatedAt: now,
  }
}

function normalizeStyle(style: SubtitleStyle): SubtitleStyle {
  const fallback = createDefaultStyle()

  return {
    ...fallback,
    ...style,
    renderMode: readOptionValue(style.renderMode, renderModeOptions, fallback.renderMode),
    subtitleLayout: readOptionValue(style.subtitleLayout, subtitleLayoutOptions, fallback.subtitleLayout),
    previewTextMode: readOptionValue(style.previewTextMode, previewTextModeOptions, fallback.previewTextMode),
    primaryColor: normalizeHexRgb(style.primaryColor, fallback.primaryColor),
    primaryOutlineColor: normalizeHexRgb(style.primaryOutlineColor, fallback.primaryOutlineColor),
    secondaryColor: normalizeHexRgb(style.secondaryColor, fallback.secondaryColor),
    secondaryOutlineColor: normalizeHexRgb(style.secondaryOutlineColor, fallback.secondaryOutlineColor),
    roundedTextColor: normalizeHexRgb(style.roundedTextColor, fallback.roundedTextColor),
    roundedBackgroundColor: normalizeHexRgba(style.roundedBackgroundColor, fallback.roundedBackgroundColor),
  }
}

function cloneStyle(style: SubtitleStyle): SubtitleStyle {
  return { ...style }
}

function toCreateRequest(style: SubtitleStyle) {
  const { id: _id, isDefault: _isDefault, createdAt: _createdAt, updatedAt: _updatedAt, ...request } = normalizeStyle(style)
  return request
}

function toUpdateRequest(style: SubtitleStyle) {
  const { isDefault: _isDefault, createdAt: _createdAt, updatedAt: _updatedAt, ...request } = normalizeStyle(style)
  return request
}

function buildAssLineStyle(input: {
  fontFamily: string
  fontSize: number
  color: string
  outlineColor: string
  outlineWidth: number
  letterSpacing: number
}): CSSProperties {
  const outlineWidth = clampNumber(input.outlineWidth, 0, 16)
  const outlineColor = normalizeCssColor(input.outlineColor, '#000000')

  return {
    color: normalizeCssColor(input.color, '#FFFFFF'),
    fontFamily: quoteFontFamily(input.fontFamily),
    fontSize: `${clampNumber(input.fontSize, 8, 200)}px`,
    fontWeight: 800,
    letterSpacing: `${clampNumber(input.letterSpacing, 0, 40)}px`,
    WebkitTextStroke: outlineWidth > 0 ? `${outlineWidth}px ${outlineColor}` : undefined,
    textShadow: outlineWidth > 0 ? buildTextShadow(outlineWidth, outlineColor) : 'none',
  }
}

function buildTextShadow(width: number, color: string) {
  const offset = Math.max(1, Math.round(width))
  return [
    `${offset}px 0 ${color}`,
    `-${offset}px 0 ${color}`,
    `0 ${offset}px ${color}`,
    `0 -${offset}px ${color}`,
    `${offset}px ${offset}px ${color}`,
    `-${offset}px ${offset}px ${color}`,
    `${offset}px -${offset}px ${color}`,
    `-${offset}px -${offset}px ${color}`,
  ].join(', ')
}

function quoteFontFamily(fontName: string) {
  const value = fontName.trim() || 'Microsoft YaHei'
  return `"${value}", "Microsoft YaHei", "Segoe UI", sans-serif`
}

function clampNumber(value: unknown, min: number, max: number) {
  const numericValue = typeof value === 'number' && Number.isFinite(value) ? value : min
  return Math.min(Math.max(numericValue, min), max)
}

function readOptionValue<T extends string>(value: unknown, options: readonly Option<T>[], fallback: T) {
  return typeof value === 'string' && options.some((option) => option.value === value) ? (value as T) : fallback
}

function normalizeCssColor(value: string, fallback: string) {
  return normalizeHexRgba(value, fallback)
}

function normalizeHexRgb(value: string, fallback: string) {
  const normalized = value.trim()
  return /^#[0-9a-fA-F]{6}$/.test(normalized) ? normalized.toUpperCase() : fallback
}

function normalizeHexRgba(value: string, fallback: string) {
  const normalized = value.trim()
  return /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(normalized) ? normalized.toUpperCase() : fallback
}

function stringifyError(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
</script>

<style scoped>
.subtitle-style-workspace {
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.subtitle-style-alert {
  margin: 0;
}

.subtitle-style-layout {
  display: grid;
  grid-template-columns: minmax(320px, 0.42fr) minmax(580px, 1fr);
  gap: 24px;
  align-items: start;
}

.subtitle-style-sidebar {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.subtitle-style-list-panel {
  display: flex;
  flex-direction: column;
  padding: 14px;
}

.subtitle-style-list {
  min-height: 0;
  max-height: 420px;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
  scrollbar-width: thin;
  scrollbar-color: var(--accent-soft) transparent;
}

.subtitle-style-list::-webkit-scrollbar {
  width: 6px;
}

.subtitle-style-list::-webkit-scrollbar-track {
  background: transparent;
}

.subtitle-style-list::-webkit-scrollbar-thumb {
  background: var(--accent-soft);
  border-radius: 3px;
}

.subtitle-style-list::-webkit-scrollbar-thumb:hover {
  background: color-mix(in srgb, var(--accent-soft) 80%, var(--accent));
}

.subtitle-style-item {
  min-height: 58px;
  border: 1px solid var(--hairline);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.2);
  color: var(--text);
  cursor: pointer;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 22px;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  text-align: left;
  transition: background 0.15s, border-color 0.15s, color 0.15s;
}

html[data-theme='dark'] .subtitle-style-item {
  background: rgba(0, 0, 0, 0.12);
}

.subtitle-style-item:hover,
.subtitle-style-item.active {
  border-color: color-mix(in srgb, var(--accent) 46%, var(--hairline));
  background: color-mix(in srgb, var(--accent-soft) 42%, transparent);
}

.subtitle-style-item-main {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 11px;
}

.subtitle-style-item-icon,
.subtitle-style-item-check {
  width: 19px;
  height: 19px;
  flex: 0 0 auto;
  color: var(--accent);
}

.subtitle-style-item-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.subtitle-style-item-name {
  overflow: hidden;
  color: var(--text);
  font-size: 14px;
  font-weight: 850;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.subtitle-style-item-meta {
  overflow: hidden;
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 750;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.subtitle-style-empty {
  min-height: 170px;
  color: var(--text-muted);
  display: flex;
  flex: 1;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 800;
}

.subtitle-style-empty svg {
  width: 24px;
  height: 24px;
}

.subtitle-style-actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
  margin-top: 14px;
}

.subtitle-style-actions .settings-action {
  min-width: 0;
  justify-self: stretch;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  padding: 0 12px;
}

.subtitle-style-actions .settings-action svg {
  width: 16px;
  height: 16px;
}

.subtitle-style-danger-action {
  border-color: rgba(159, 48, 48, 0.24);
  color: #9f3030;
}

.subtitle-style-danger-action:not(:disabled):hover {
  border-color: rgba(159, 48, 48, 0.38);
  color: #842323;
  background: rgba(184, 59, 59, 0.11);
}

html[data-theme='dark'] .subtitle-style-danger-action {
  color: #f0a2a2;
}

.subtitle-preview-section {
  min-width: 0;
}

.subtitle-preview-panel {
  min-height: 520px;
  display: flex;
  flex-direction: column;
  padding: 18px;
}

.subtitle-preview-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
}

.subtitle-preview-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.subtitle-preview-name {
  overflow: hidden;
  color: var(--text);
  font-size: 16px;
  font-weight: 850;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.subtitle-preview-detail {
  overflow: hidden;
  color: var(--text-muted);
  font-size: 13px;
  font-weight: 750;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.subtitle-preview-badge {
  flex: 0 0 auto;
  border: 1px solid var(--hairline);
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.2);
  color: var(--text-muted);
  padding: 6px 10px;
  font-size: 12px;
  font-weight: 850;
  line-height: 1;
}

html[data-theme='dark'] .subtitle-preview-badge {
  background: rgba(0, 0, 0, 0.14);
}

.subtitle-preview-viewport {
  overflow: hidden;
  width: 100%;
  min-height: min(48vw, 720px);
  border-radius: 14px;
  background: linear-gradient(135deg, #1a1d29 0%, #2d1b2e 50%, #1f2937 100%);
}

.subtitle-preview-stage {
  position: relative;
  overflow: hidden;
  transform-origin: top left;
}

.subtitle-preview-image {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  user-select: none;
}

.subtitle-preview-overlay {
  position: absolute;
  left: 72px;
  right: 72px;
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  pointer-events: none;
}

.subtitle-preview-ass-line,
.subtitle-preview-rounded-line {
  max-width: 100%;
  overflow-wrap: anywhere;
  line-height: 1.18;
}

.subtitle-preview-ass-line {
  text-wrap: balance;
}

.subtitle-preview-rounded-line {
  box-shadow: 0 10px 28px rgba(0, 0, 0, 0.18);
  text-wrap: balance;
}

.subtitle-style-controls {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 24px;
  align-items: start;
}

.subtitle-text-input {
  justify-self: end;
  width: min(220px, 30vw);
}

.subtitle-number-input {
  justify-self: end;
  width: 92px;
  text-align: center;
}

.subtitle-hex-input {
  justify-self: end;
  width: 132px;
  text-transform: uppercase;
}

.subtitle-color-input {
  justify-self: end;
  width: 58px;
  height: 34px;
  border: 1px solid var(--hairline);
  border-radius: 10px;
  background: color-mix(in srgb, var(--bg-surface-hover) 58%, var(--bg) 42%);
  cursor: pointer;
  padding: 4px;
}

.subtitle-color-input:focus-visible {
  outline: 3px solid var(--accent-soft);
  outline-offset: 2px;
}

.subtitle-choice-dialog,
.subtitle-name-dialog {
  width: min(360px, calc(100vw - 56px));
}

.subtitle-dialog-field {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 22px;
}

.subtitle-dialog-label {
  color: var(--text);
  font-size: 13px;
  font-weight: 800;
}

.subtitle-dialog-input {
  width: 100%;
}

.subtitle-dialog-copy {
  margin-top: 18px;
  color: var(--text-muted);
  font-size: 14px;
  font-weight: 650;
  line-height: 1.45;
}

.subtitle-dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 22px;
}

@media (max-width: 1180px) {
  .subtitle-style-layout {
    grid-template-columns: 1fr;
  }

  .subtitle-style-sidebar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  }

  .subtitle-style-controls {
    grid-template-columns: 1fr 1fr;
  }
}

@media (max-width: 860px) {
  .subtitle-style-sidebar,
  .subtitle-style-controls {
    grid-template-columns: 1fr;
  }

  .subtitle-preview-panel {
    padding: 14px;
  }

  .subtitle-preview-toolbar {
    align-items: flex-start;
    flex-direction: column;
  }

  .subtitle-preview-viewport {
    min-height: min(58vw, 720px);
  }

  .subtitle-text-input,
  .subtitle-number-input,
  .subtitle-hex-input,
  .subtitle-color-input {
    grid-column: 1;
    justify-self: start;
  }

  .subtitle-text-input {
    width: min(280px, 100%);
  }

  .subtitle-style-actions {
    grid-template-columns: 1fr;
  }

  .subtitle-dialog-actions {
    align-items: stretch;
    flex-direction: column-reverse;
  }

  .subtitle-dialog-actions .settings-action {
    width: 100%;
  }
}
</style>
