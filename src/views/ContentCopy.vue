<template>
  <div class="page translate-page content-copy-page">
    <header class="translate-header">
      <h1 class="page-title">文案</h1>
    </header>

    <main class="translate-workspace content-copy-workspace">
      <div class="content-copy-grid">
        <div class="content-copy-side">
          <section class="settings-section" aria-labelledby="content-copy-input-title">
            <div id="content-copy-input-title" class="section-heading">
              <Captions aria-hidden="true" />
              <span>字幕输入</span>
            </div>

            <div class="settings-panel translate-drop-panel content-copy-input-panel">
              <div
                ref="subtitleDropZoneRef"
                class="translate-drop-zone content-copy-drop-zone"
                :class="{ 'drag-active': isDragActive }"
                @dragenter.prevent="isDragActive = true"
                @dragover.prevent
                @dragleave.prevent="isDragActive = false"
                @drop.prevent="handleBrowserDrop"
              >
                <UploadCloud class="translate-drop-icon" :stroke-width="2.1" aria-hidden="true" />
                <div class="translate-drop-copy">
                  <span class="translate-drop-title">选择或拖入字幕</span>
                  <span class="translate-drop-subtitle">支持 SRT、VTT、ASS</span>
                </div>
                <button class="settings-action" type="button" :disabled="isGenerating" @click="selectSubtitleFile">
                  选择字幕
                </button>
              </div>

              <div class="translate-file-strip" aria-label="当前字幕">
                <FileText :stroke-width="2.1" aria-hidden="true" />
                <span>{{ selectedSubtitleName }}</span>
              </div>

              <label class="content-copy-context">
                <span>补充信息</span>
                <textarea
                  v-model="extraContext"
                  class="content-copy-textarea"
                  :disabled="isGenerating"
                  rows="4"
                  placeholder="可填写内容定位、目标观众、特殊卖点或需要避开的表达"
                />
              </label>

              <button
                class="settings-action content-copy-primary-action"
                type="button"
                :disabled="!canGenerate"
                @click="generateCopy"
              >
                <WandSparkles :stroke-width="2.1" aria-hidden="true" />
                <span>{{ isGenerating ? '生成中' : '生成文案' }}</span>
              </button>
            </div>
          </section>

          <section class="settings-section content-copy-history-section" aria-labelledby="content-copy-history-title">
            <div id="content-copy-history-title" class="section-heading">
              <History aria-hidden="true" />
              <span>历史记录</span>
            </div>

            <div class="settings-panel content-copy-history-panel">
              <button
                v-for="record in records"
                :key="record.id"
                class="content-copy-history-item"
                :class="{ active: record.id === currentRecord?.id }"
                type="button"
                @click="selectRecord(record)"
              >
                <span class="content-copy-history-title">{{ primaryTitle(record) }}</span>
                <span class="content-copy-history-meta">
                  {{ record.subtitleFileName || '字幕文件' }} · {{ formatDate(record.updatedAt) }}
                </span>
              </button>

              <div v-if="!records.length" class="content-copy-history-empty">
                <ClipboardList :stroke-width="2.1" aria-hidden="true" />
                <span>暂无历史记录</span>
              </div>
            </div>
          </section>
        </div>

        <section class="settings-section content-copy-result-section" aria-labelledby="content-copy-result-title">
          <div id="content-copy-result-title" class="section-heading">
            <Sparkles aria-hidden="true" />
            <span>生成结果</span>
          </div>

          <div class="settings-panel content-copy-result-panel">
            <div class="translate-status-bar">
              <div class="translate-status">
                <span class="translate-status-dot" :class="statusDotClass" aria-hidden="true" />
                <span>{{ statusText }}</span>
              </div>

              <div class="translate-actions content-copy-actions">
                <button
                  class="settings-action"
                  :class="{ 'copy-confirmed': isCopied(CopyTarget.Full) }"
                  type="button"
                  :disabled="!currentRecord"
                  @click="copyFullRecord"
                >
                  <Check v-if="isCopied(CopyTarget.Full)" :stroke-width="2.1" aria-hidden="true" />
                  <Copy v-else :stroke-width="2.1" aria-hidden="true" />
                  <span>{{ copiedLabel === CopyTarget.Full ? '已复制' : '复制全部' }}</span>
                </button>
                <button class="settings-action" type="button" :disabled="!currentRecord || isGenerating" @click="deleteCurrentRecord">
                  <Trash2 :stroke-width="2.1" aria-hidden="true" />
                  <span>删除</span>
                </button>
              </div>
            </div>

            <div v-if="generationError" class="translate-alert" role="alert">
              <CircleAlert :stroke-width="2.1" aria-hidden="true" />
              <span>{{ generationError }}</span>
            </div>

            <div v-if="isGenerating" class="translate-progress" aria-label="处理进度">
              <div class="translate-progress-track">
                <span class="translate-progress-bar content-copy-progress-bar" />
              </div>
              <span class="translate-progress-value">AI</span>
            </div>

            <div v-if="currentRecord" class="content-copy-result-scroll">
              <div class="content-copy-overview">
                <div>
                  <span class="content-copy-label">内容摘要</span>
                  <p>{{ currentRecord.result.summary }}</p>
                </div>
                <div>
                  <span class="content-copy-label">目标观众</span>
                  <p>{{ currentRecord.result.audience }}</p>
                </div>
                <div>
                  <span class="content-copy-label">推荐分类</span>
                  <p>{{ categoryText(currentRecord) }}</p>
                </div>
              </div>

              <section class="content-copy-block" aria-labelledby="content-copy-titles-heading">
                <div class="content-copy-block-heading">
                  <h2 id="content-copy-titles-heading">标题候选</h2>
                </div>
                <div class="content-copy-title-list">
                  <article v-for="(title, index) in currentRecord.result.titles" :key="`${title.title}-${index}`" class="content-copy-title-item">
                    <span class="content-copy-index">{{ index + 1 }}</span>
                    <div class="content-copy-title-body">
                      <h3>{{ title.title }}</h3>
                      <p>{{ title.hook }} · {{ title.reason }}</p>
                    </div>
                    <button
                      class="content-copy-copy-button content-copy-title-copy"
                      :class="{ 'copy-confirmed': isCopied(titleCopyTarget(index)) }"
                      type="button"
                      :aria-label="isCopied(titleCopyTarget(index)) ? `标题 ${index + 1} 已复制` : `复制标题 ${index + 1}`"
                      @click="copyTitle(title, index)"
                    >
                      <Check v-if="isCopied(titleCopyTarget(index))" :stroke-width="2.1" aria-hidden="true" />
                      <Copy v-else :stroke-width="2.1" aria-hidden="true" />
                      <span>{{ isCopied(titleCopyTarget(index)) ? '已复制' : '复制' }}</span>
                    </button>
                  </article>
                </div>
              </section>

              <section class="content-copy-block" aria-labelledby="content-copy-cover-heading">
                <div class="content-copy-block-heading">
                  <h2 id="content-copy-cover-heading">封面字</h2>
                </div>
                <div class="content-copy-cover-grid">
                  <article v-for="(cover, index) in currentRecord.result.coverTexts" :key="`${cover.lines.join('-')}-${index}`" class="content-copy-cover-item">
                    <div class="content-copy-cover-head">
                      <div class="content-copy-cover-lines">
                        <span v-for="line in cover.lines" :key="line">{{ line }}</span>
                      </div>
                      <button
                        class="content-copy-copy-button content-copy-cover-copy"
                        :class="{ 'copy-confirmed': isCopied(coverCopyTarget(index)) }"
                        type="button"
                        :aria-label="isCopied(coverCopyTarget(index)) ? `封面字 ${index + 1} 已复制` : `复制封面字 ${index + 1}`"
                        @click="copyCoverText(cover, index)"
                      >
                        <Check v-if="isCopied(coverCopyTarget(index))" :stroke-width="2.1" aria-hidden="true" />
                        <Copy v-else :stroke-width="2.1" aria-hidden="true" />
                        <span>{{ isCopied(coverCopyTarget(index)) ? '已复制' : '复制' }}</span>
                      </button>
                    </div>
                    <p>{{ cover.reason }}</p>
                  </article>
                </div>
              </section>

              <section class="content-copy-block" aria-labelledby="content-copy-description-heading">
                <div class="content-copy-block-heading">
                  <h2 id="content-copy-description-heading">内容简介</h2>
                  <button
                    class="content-copy-icon-button"
                    :class="{ 'copy-confirmed': isCopied(CopyTarget.Description) }"
                    type="button"
                    :aria-label="isCopied(CopyTarget.Description) ? '简介已复制' : '复制简介'"
                    @click="copyDescription"
                  >
                    <Check v-if="isCopied(CopyTarget.Description)" :stroke-width="2.1" aria-hidden="true" />
                    <Copy v-else :stroke-width="2.1" aria-hidden="true" />
                  </button>
                </div>
                <div class="content-copy-description">
                  <p>{{ currentRecord.result.description.intro }}</p>
                  <div v-if="currentRecord.result.description.timeline.length" class="content-copy-timeline">
                    <div v-for="item in currentRecord.result.description.timeline" :key="`${item.time}-${item.text}`">
                      <span>{{ item.time }}</span>
                      <p>{{ item.text }}</p>
                    </div>
                  </div>
                  <p>{{ currentRecord.result.description.callToAction }}</p>
                </div>
              </section>

              <section class="content-copy-block" aria-labelledby="content-copy-tags-heading">
                <div class="content-copy-block-heading">
                  <h2 id="content-copy-tags-heading">标签组合</h2>
                  <button
                    class="content-copy-icon-button"
                    :class="{ 'copy-confirmed': isCopied(CopyTarget.Tags) }"
                    type="button"
                    :aria-label="isCopied(CopyTarget.Tags) ? '标签已复制' : '复制标签'"
                    @click="copyTags"
                  >
                    <Check v-if="isCopied(CopyTarget.Tags)" :stroke-width="2.1" aria-hidden="true" />
                    <Copy v-else :stroke-width="2.1" aria-hidden="true" />
                  </button>
                </div>
                <div class="content-copy-tags">
                  <button
                    v-for="(tag, index) in allTags(currentRecord)"
                    :key="`${tag}-${index}`"
                    class="content-copy-tag-button"
                    :class="{ 'copy-confirmed': isCopied(tagCopyTarget(index)) }"
                    type="button"
                    :aria-label="isCopied(tagCopyTarget(index)) ? `标签 ${tag} 已复制` : `复制标签 ${tag}`"
                    @click="copyTag(tag, index)"
                  >
                    {{ tag }}
                  </button>
                </div>
              </section>

              <section class="content-copy-block" aria-labelledby="content-copy-comment-heading">
                <div class="content-copy-block-heading">
                  <h2 id="content-copy-comment-heading">互动评论</h2>
                  <button
                    class="content-copy-icon-button"
                    :class="{ 'copy-confirmed': isCopied(CopyTarget.Comment) }"
                    type="button"
                    :aria-label="isCopied(CopyTarget.Comment) ? '互动评论已复制' : '复制互动评论'"
                    @click="copyPinnedComment"
                  >
                    <Check v-if="isCopied(CopyTarget.Comment)" :stroke-width="2.1" aria-hidden="true" />
                    <Copy v-else :stroke-width="2.1" aria-hidden="true" />
                  </button>
                </div>
                <p class="content-copy-comment">{{ currentRecord.result.pinnedComment }}</p>
              </section>
            </div>

            <div v-else class="translate-preview translate-preview-empty content-copy-empty">
              <Sparkles class="translate-empty-icon" :stroke-width="2.1" aria-hidden="true" />
              <span class="translate-empty-title">等待生成</span>
              <span class="translate-empty-subtitle">选择字幕后，标题、封面字和标签会显示在这里</span>
            </div>
          </div>
        </section>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import type { DragDropEvent } from '@tauri-apps/api/webview'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import {
  Captions,
  Check,
  CircleAlert,
  ClipboardList,
  Copy,
  FileText,
  History,
  Sparkles,
  Trash2,
  UploadCloud,
  WandSparkles,
} from 'lucide-vue-next'

defineOptions({ name: 'ContentCopy' })

enum CopyTarget {
  Full = 'full',
  Description = 'description',
  Tags = 'tags',
  Comment = 'comment',
}

type TitleCopyTarget = `title-${number}`
type CoverCopyTarget = `cover-${number}`
type TagCopyTarget = `tag-${number}`
type CopyFeedbackTarget = CopyTarget | TitleCopyTarget | CoverCopyTarget | TagCopyTarget

type ContentCopyCategory = {
  primary: string
  secondary: string
  reason: string
}

type ContentCopyTitle = {
  title: string
  hook: string
  reason: string
}

type ContentCopyCoverText = {
  lines: string[]
  reason: string
}

type ContentCopyTimelineItem = {
  time: string
  text: string
}

type ContentCopyDescription = {
  intro: string
  timeline: ContentCopyTimelineItem[]
  callToAction: string
}

type ContentCopyTags = {
  core: string[]
  category: string[]
  longTail: string[]
}

type ContentCopyResult = {
  summary: string
  audience: string
  category: ContentCopyCategory
  titles: ContentCopyTitle[]
  coverTexts: ContentCopyCoverText[]
  description: ContentCopyDescription
  tags: ContentCopyTags
  pinnedComment: string
}

type ContentCopyOptions = {
  platform: string
  titleCount: number
  coverTextCount: number
}

type ContentCopyRecord = {
  id: string
  source: string
  platform: string
  subtitlePath: string
  subtitleFileName: string
  subtitleFormat: string
  segmentCount: number
  durationMs: number
  extraContext: string
  options: ContentCopyOptions
  result: ContentCopyResult
  logPath: string
  createdAt: string
  updatedAt: string
}

const subtitleDropZoneRef = ref<HTMLElement | null>(null)
const selectedSubtitlePath = ref('')
const extraContext = ref('')
const isDragActive = ref(false)
const isGenerating = ref(false)
const generationError = ref('')
const records = ref<ContentCopyRecord[]>([])
const currentRecord = ref<ContentCopyRecord | null>(null)
const copiedLabel = ref<CopyFeedbackTarget | null>(null)
let unlistenDragDrop: UnlistenFn | undefined
let copiedTimer: ReturnType<typeof window.setTimeout> | undefined

const subtitleExtensions = ['srt', 'vtt', 'ass']
const isTauriRuntime = () => '__TAURI_INTERNALS__' in window

const selectedSubtitleName = computed(() => {
  return selectedSubtitlePath.value ? fileNameFromPath(selectedSubtitlePath.value) : '尚未选择字幕'
})

const canGenerate = computed(() => Boolean(selectedSubtitlePath.value) && !isGenerating.value)

const statusText = computed(() => {
  if (isGenerating.value) {
    return '正在生成文案'
  }
  if (generationError.value) {
    return '生成失败'
  }
  if (currentRecord.value) {
    return '已生成'
  }
  return '等待选择字幕'
})

const statusDotClass = computed(() => ({
  active: isGenerating.value,
  success: Boolean(currentRecord.value) && !generationError.value && !isGenerating.value,
  error: Boolean(generationError.value),
}))

const selectSubtitleFile = async () => {
  if (!isTauriRuntime()) {
    generationError.value = '请在桌面应用中选择字幕文件'
    return
  }

  try {
    const selected = await open({
      title: '选择字幕文件',
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
    generationError.value = stringifyError(error, '选择字幕失败')
  }
}

const applySubtitleFile = (path: string) => {
  if (isGenerating.value) {
    return
  }

  if (!subtitleExtensions.includes(fileExtension(path))) {
    generationError.value = '请选择 SRT、VTT 或 ASS 字幕文件'
    return
  }

  selectedSubtitlePath.value = path
  generationError.value = ''
}

const generateCopy = async () => {
  if (!canGenerate.value) {
    return
  }

  if (!isTauriRuntime()) {
    generationError.value = '请在桌面应用中生成文案'
    return
  }

  isGenerating.value = true
  generationError.value = ''
  clearCopiedFeedback()

  try {
    const record = await invoke<ContentCopyRecord>('generate_content_copy', {
      request: {
        subtitlePath: selectedSubtitlePath.value,
        extraContext: extraContext.value,
        source: 'copywriting',
      },
    })
    currentRecord.value = record
    records.value = [record, ...records.value.filter((item) => item.id !== record.id)].slice(0, 30)
  } catch (error) {
    generationError.value = stringifyError(error, '生成文案失败')
  } finally {
    isGenerating.value = false
  }
}

const loadRecords = async () => {
  if (!isTauriRuntime()) {
    return
  }

  try {
    const result = await invoke<ContentCopyRecord[]>('list_content_copy_records', {
      request: { limit: 30, source: 'copywriting' },
    })
    records.value = result
    if (!currentRecord.value && result.length) {
      currentRecord.value = result[0]
    }
  } catch (error) {
    generationError.value = stringifyError(error, '加载历史记录失败')
  }
}

const selectRecord = (record: ContentCopyRecord) => {
  currentRecord.value = record
  generationError.value = ''
  clearCopiedFeedback()
  selectedSubtitlePath.value = record.subtitlePath
  extraContext.value = record.extraContext
}

const deleteCurrentRecord = async () => {
  const record = currentRecord.value
  if (!record || isGenerating.value || !isTauriRuntime()) {
    return
  }

  try {
    await invoke('delete_content_copy_record', { request: { id: record.id } })
    records.value = records.value.filter((item) => item.id !== record.id)
    currentRecord.value = records.value[0] ?? null
  } catch (error) {
    generationError.value = stringifyError(error, '删除历史记录失败')
  }
}

const handleBrowserDrop = (event: DragEvent) => {
  isDragActive.value = false
  const file = Array.from(event.dataTransfer?.files ?? [])[0] as (File & { path?: string }) | undefined
  if (file?.path) {
    applySubtitleFile(file.path)
  }
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
      isDragActive.value = false
      return
    }

    if (payload.type === 'over' || payload.type === 'enter') {
      isDragActive.value = isPayloadInsideDropZone(payload, await currentWindow.scaleFactor())
      return
    }

    if (payload.type !== 'drop') {
      return
    }

    const isInside = isPayloadInsideDropZone(payload, await currentWindow.scaleFactor())
    isDragActive.value = false
    const path = payload.paths[0]
    if (isInside && path) {
      applySubtitleFile(path)
    }
  })
}

const isPayloadInsideDropZone = (
  payload: Extract<DragDropEvent, { type: 'enter' | 'over' | 'drop' }>,
  scaleFactor: number,
) => {
  const logicalPosition = payload.position.toLogical(scaleFactor)
  return isPointInsideElement({ x: logicalPosition.x, y: logicalPosition.y }, subtitleDropZoneRef.value)
}

const isPointInsideElement = (point: { x: number; y: number }, element: HTMLElement | null) => {
  if (!element) {
    return false
  }

  const rect = element.getBoundingClientRect()
  return point.x >= rect.left && point.x <= rect.right && point.y >= rect.top && point.y <= rect.bottom
}

const copyFullRecord = () => {
  if (currentRecord.value) {
    void copyText(formatFullRecord(currentRecord.value), CopyTarget.Full)
  }
}

const titleCopyTarget = (index: number): TitleCopyTarget => `title-${index}`
const coverCopyTarget = (index: number): CoverCopyTarget => `cover-${index}`
const tagCopyTarget = (index: number): TagCopyTarget => `tag-${index}`

const isCopied = (target: CopyFeedbackTarget) => copiedLabel.value === target

const copyTitle = (title: ContentCopyTitle, index: number) => {
  void copyText(title.title, titleCopyTarget(index))
}

const copyCoverText = (cover: ContentCopyCoverText, index: number) => {
  void copyText(cover.lines.join('\n'), coverCopyTarget(index))
}

const copyTag = (tag: string, index: number) => {
  void copyText(tag, tagCopyTarget(index))
}

const copyDescription = () => {
  if (currentRecord.value) {
    const description = currentRecord.value.result.description
    const timeline = description.timeline.map((item) => `${item.time} ${item.text}`).join('\n')
    void copyText([description.intro, timeline, description.callToAction].filter(Boolean).join('\n\n'), CopyTarget.Description)
  }
}

const copyTags = () => {
  if (currentRecord.value) {
    void copyText(allTags(currentRecord.value).join(' '), CopyTarget.Tags)
  }
}

const copyPinnedComment = () => {
  if (currentRecord.value) {
    void copyText(currentRecord.value.result.pinnedComment, CopyTarget.Comment)
  }
}

const copyText = async (text: string, target: CopyFeedbackTarget) => {
  try {
    await navigator.clipboard.writeText(text)
    copiedLabel.value = target
    if (copiedTimer !== undefined) {
      window.clearTimeout(copiedTimer)
    }
    copiedTimer = window.setTimeout(() => {
      copiedLabel.value = null
      copiedTimer = undefined
    }, 1300)
  } catch (error) {
    generationError.value = stringifyError(error, '复制失败')
  }
}

const clearCopiedFeedback = () => {
  copiedLabel.value = null
  if (copiedTimer !== undefined) {
    window.clearTimeout(copiedTimer)
    copiedTimer = undefined
  }
}

const formatFullRecord = (record: ContentCopyRecord) => {
  const titles = record.result.titles.map((title, index) => `${index + 1}. ${title.title}`).join('\n')
  const coverTexts = record.result.coverTexts.map((cover, index) => `${index + 1}. ${cover.lines.join(' / ')}`).join('\n')
  const timeline = record.result.description.timeline.map((item) => `${item.time} ${item.text}`).join('\n')
  return [
    `内容摘要：${record.result.summary}`,
    `目标观众：${record.result.audience}`,
    `推荐分类：${categoryText(record)}`,
    `标题候选：\n${titles}`,
    `封面字：\n${coverTexts}`,
    `内容简介：\n${record.result.description.intro}`,
    timeline ? `时间轴：\n${timeline}` : '',
    record.result.description.callToAction,
    `标签组合：${allTags(record).join(' ')}`,
    `互动评论：${record.result.pinnedComment}`,
  ]
    .filter(Boolean)
    .join('\n\n')
}

const primaryTitle = (record: ContentCopyRecord) => {
  return record.result.titles[0]?.title || record.result.summary || '未命名文案'
}

const categoryText = (record: ContentCopyRecord) => {
  const category = record.result.category
  return [category.primary, category.secondary].filter(Boolean).join(' / ') || category.reason || '暂无分类'
}

const allTags = (record: ContentCopyRecord) => {
  return [...record.result.tags.core, ...record.result.tags.category, ...record.result.tags.longTail]
}

const formatDate = (value: string) => {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) {
    return value
  }

  return new Intl.DateTimeFormat('zh-Hans', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date)
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
  void loadRecords()
  void registerDragDropListener()
})

onBeforeUnmount(() => {
  unlistenDragDrop?.()
  if (copiedTimer !== undefined) {
    window.clearTimeout(copiedTimer)
  }
})
</script>

<style scoped>
.content-copy-workspace {
  min-height: 0;
}

.content-copy-grid {
  display: grid;
  grid-template-columns: minmax(300px, 0.82fr) minmax(520px, 1.45fr);
  gap: 26px;
  align-items: start;
}

.content-copy-side {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 26px;
}

.content-copy-side > .settings-section + .settings-section,
.content-copy-result-section {
  margin-top: 0;
}

.content-copy-input-panel {
  gap: 14px;
}

.content-copy-drop-zone {
  min-height: 190px;
}

.content-copy-context {
  display: flex;
  flex-direction: column;
  gap: 8px;
  color: var(--text-muted);
  font-size: 13px;
  font-weight: 800;
}

.content-copy-textarea {
  width: 100%;
  min-height: 112px;
  resize: vertical;
  border: 1px solid var(--hairline);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.26);
  color: var(--text);
  padding: 12px 14px;
  font: inherit;
  font-size: 14px;
  font-weight: 650;
  line-height: 1.45;
  outline: none;
  transition: background 0.15s, border-color 0.15s, box-shadow 0.15s;
}

html[data-theme='dark'] .content-copy-textarea {
  background: rgba(0, 0, 0, 0.14);
}

.content-copy-textarea:focus {
  border-color: color-mix(in srgb, var(--accent) 42%, var(--hairline));
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent-soft) 58%, transparent);
}

.content-copy-textarea::placeholder {
  color: var(--text-subtle);
}

.content-copy-primary-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  width: 100%;
}

.content-copy-primary-action svg,
.content-copy-actions .settings-action svg {
  width: 16px;
  height: 16px;
}

.content-copy-primary-action:not(:disabled) {
  border-color: color-mix(in srgb, var(--accent) 46%, var(--hairline));
  background: var(--accent);
  color: #fff;
}

html[data-theme='dark'] .content-copy-primary-action:not(:disabled) {
  color: #18140f;
}

.content-copy-primary-action:not(:disabled):hover {
  border-color: var(--accent-strong);
  background: var(--accent-strong);
}

.content-copy-history-panel {
  max-height: 360px;
  overflow: auto;
  padding: 8px;
  scrollbar-width: none;
}

.content-copy-history-panel::-webkit-scrollbar {
  display: none;
}

.content-copy-history-item {
  width: 100%;
  min-height: 66px;
  border: 0;
  border-radius: 12px;
  background: transparent;
  color: var(--text);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 6px;
  padding: 10px 12px;
  text-align: left;
  transition: background 0.15s;
}

.content-copy-history-item:hover,
.content-copy-history-item.active {
  background: var(--bg-surface-hover);
}

.content-copy-history-title,
.content-copy-history-meta {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.content-copy-history-title {
  font-size: 14px;
  font-weight: 850;
  line-height: 1.2;
}

.content-copy-history-meta {
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 750;
  line-height: 1.2;
}

.content-copy-history-empty {
  min-height: 150px;
  color: var(--text-muted);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 9px;
  font-size: 14px;
  font-weight: 750;
}

.content-copy-history-empty svg {
  width: 26px;
  height: 26px;
  color: var(--accent);
}

.content-copy-result-panel {
  display: flex;
  min-height: clamp(560px, calc(100dvh - var(--titlebar-h) - 132px), 1120px);
  flex-direction: column;
  padding: 18px;
}

.content-copy-actions .settings-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
}

.content-copy-actions .settings-action.copy-confirmed {
  border-color: color-mix(in srgb, var(--accent) 40%, var(--hairline));
  background: color-mix(in srgb, var(--accent-soft) 64%, var(--bg-surface));
  color: var(--accent-strong);
}

.content-copy-progress-bar {
  width: 42%;
  animation: content-copy-progress 1.2s ease-in-out infinite alternate;
}

@keyframes content-copy-progress {
  from { transform: translateX(-24%); }
  to { transform: translateX(144%); }
}

.content-copy-result-scroll {
  min-height: 0;
  overflow: auto;
  padding: 16px 2px 2px;
  scrollbar-width: none;
}

.content-copy-result-scroll::-webkit-scrollbar {
  display: none;
}

.content-copy-overview {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  overflow: hidden;
  border: 1px solid var(--hairline);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.2);
}

html[data-theme='dark'] .content-copy-overview {
  background: rgba(0, 0, 0, 0.12);
}

.content-copy-overview > div {
  min-width: 0;
  padding: 15px 16px;
}

.content-copy-overview > div + div {
  border-left: 1px solid var(--hairline);
}

.content-copy-label {
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 850;
  line-height: 1;
}

.content-copy-overview p {
  margin-top: 8px;
  color: var(--text);
  font-size: 14px;
  font-weight: 720;
  line-height: 1.45;
}

.content-copy-block {
  margin-top: 18px;
}

.content-copy-block-heading {
  min-height: 32px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.content-copy-block-heading h2 {
  color: var(--text);
  font-size: 16px;
  font-weight: 850;
  line-height: 1.2;
}

.content-copy-icon-button {
  width: 32px;
  height: 32px;
  border: 1px solid var(--hairline);
  border-radius: 9px;
  background: rgba(255, 255, 255, 0.2);
  color: var(--text-muted);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s, color 0.15s, border-color 0.15s;
}

html[data-theme='dark'] .content-copy-icon-button {
  background: rgba(0, 0, 0, 0.12);
}

.content-copy-icon-button:hover {
  border-color: color-mix(in srgb, var(--accent) 36%, var(--hairline));
  background: var(--bg-surface-hover);
  color: var(--accent);
}

.content-copy-icon-button.copy-confirmed,
.content-copy-copy-button.copy-confirmed,
.content-copy-tag-button.copy-confirmed {
  border-color: color-mix(in srgb, var(--accent) 46%, var(--hairline));
  background: color-mix(in srgb, var(--accent-soft) 68%, var(--bg-surface));
  color: var(--accent-strong);
}

.content-copy-tag-button.copy-confirmed {
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 20%, transparent);
  transform: translateY(-1px);
}

.content-copy-icon-button svg {
  width: 16px;
  height: 16px;
}

.content-copy-title-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-top: 10px;
}

.content-copy-title-item {
  min-height: 68px;
  border: 1px solid var(--hairline);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.2);
  display: grid;
  grid-template-columns: 28px minmax(0, 1fr) auto;
  gap: 12px;
  align-items: start;
  padding: 13px 14px;
}

html[data-theme='dark'] .content-copy-title-item {
  background: rgba(0, 0, 0, 0.12);
}

.content-copy-index {
  width: 28px;
  height: 28px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent-strong);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  font-weight: 900;
  line-height: 1;
}

.content-copy-title-body {
  min-width: 0;
}

.content-copy-title-item h3 {
  color: var(--text);
  font-size: 16px;
  font-weight: 850;
  line-height: 1.28;
}

.content-copy-title-item p,
.content-copy-cover-item p,
.content-copy-description,
.content-copy-comment {
  color: var(--text-muted);
  font-size: 13px;
  font-weight: 680;
  line-height: 1.5;
}

.content-copy-title-item p {
  margin-top: 5px;
}

.content-copy-copy-button {
  min-width: 72px;
  height: 32px;
  border: 1px solid var(--hairline);
  border-radius: 9px;
  background: rgba(255, 255, 255, 0.2);
  color: var(--text-muted);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 0 10px;
  font-size: 12px;
  font-weight: 850;
  line-height: 1;
  transition: background 0.15s, color 0.15s, border-color 0.15s, transform 0.15s;
}

html[data-theme='dark'] .content-copy-copy-button {
  background: rgba(0, 0, 0, 0.12);
}

.content-copy-copy-button:hover {
  border-color: color-mix(in srgb, var(--accent) 36%, var(--hairline));
  background: var(--bg-surface-hover);
  color: var(--accent);
}

.content-copy-copy-button:active {
  transform: translateY(1px);
}

.content-copy-copy-button svg {
  width: 14px;
  height: 14px;
  flex: 0 0 auto;
}

.content-copy-cover-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  margin-top: 10px;
}

.content-copy-cover-item {
  min-width: 0;
  border: 1px solid var(--hairline);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.2);
  padding: 14px;
}

html[data-theme='dark'] .content-copy-cover-item {
  background: rgba(0, 0, 0, 0.12);
}

.content-copy-cover-head {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 12px;
  align-items: start;
}

.content-copy-cover-lines {
  min-height: 54px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 5px;
  min-width: 0;
}

.content-copy-cover-lines span {
  color: var(--text);
  font-size: 22px;
  font-weight: 900;
  line-height: 1.05;
}

.content-copy-cover-item p {
  margin-top: 10px;
}

.content-copy-cover-copy {
  min-width: 72px;
}

.content-copy-description {
  margin-top: 10px;
  border: 1px solid var(--hairline);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.2);
  padding: 14px;
}

html[data-theme='dark'] .content-copy-description {
  background: rgba(0, 0, 0, 0.12);
}

.content-copy-description > p + p {
  margin-top: 10px;
}

.content-copy-timeline {
  margin: 12px 0;
  border-top: 1px solid var(--hairline);
  border-bottom: 1px solid var(--hairline);
  padding: 6px 0;
}

.content-copy-timeline > div {
  display: grid;
  grid-template-columns: 56px minmax(0, 1fr);
  gap: 10px;
  padding: 6px 0;
}

.content-copy-timeline span {
  color: var(--accent);
  font-size: 12px;
  font-weight: 900;
  font-variant-numeric: tabular-nums;
}

.content-copy-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 10px;
}

.content-copy-tag-button {
  appearance: none;
  min-height: 28px;
  border: 1px solid color-mix(in srgb, var(--accent) 26%, var(--hairline));
  border-radius: 999px;
  background: color-mix(in srgb, var(--accent-soft) 46%, transparent);
  color: var(--text);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0 11px;
  font-family: inherit;
  font-size: 12px;
  font-weight: 850;
  line-height: 1;
  transition: background 0.15s, border-color 0.15s, color 0.15s, transform 0.15s, box-shadow 0.15s;
}

.content-copy-tag-button:hover {
  border-color: color-mix(in srgb, var(--accent) 40%, var(--hairline));
  background: var(--bg-surface-hover);
  color: var(--accent-strong);
}

.content-copy-tag-button:active {
  transform: translateY(1px);
}

.content-copy-tag-button:focus-visible {
  outline: 2px solid color-mix(in srgb, var(--accent) 44%, transparent);
  outline-offset: 2px;
}

.content-copy-tag-button.copy-confirmed:hover {
  border-color: color-mix(in srgb, var(--accent) 46%, var(--hairline));
  background: color-mix(in srgb, var(--accent-soft) 68%, var(--bg-surface));
  color: var(--accent-strong);
}

.content-copy-comment {
  margin-top: 10px;
  border: 1px solid var(--hairline);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.2);
  padding: 14px;
}

html[data-theme='dark'] .content-copy-comment {
  background: rgba(0, 0, 0, 0.12);
}

.content-copy-empty {
  min-height: 380px;
}

@media (max-width: 1180px) {
  .content-copy-grid {
    grid-template-columns: 1fr;
  }

  .content-copy-result-panel {
    min-height: 560px;
  }
}

@media (max-width: 760px) {
  .content-copy-overview,
  .content-copy-cover-grid {
    grid-template-columns: 1fr;
  }

  .content-copy-overview > div + div {
    border-left: 0;
    border-top: 1px solid var(--hairline);
  }

  .content-copy-actions {
    width: 100%;
    justify-content: flex-start;
    flex-wrap: wrap;
  }

  .content-copy-cover-lines span {
    font-size: 20px;
  }

  .content-copy-title-item {
    grid-template-columns: 28px minmax(0, 1fr);
  }

  .content-copy-title-copy {
    grid-column: 2;
    justify-self: start;
  }
}
</style>
