<template>
  <div class="page home-page">
    <header class="translate-header home-header">
      <div class="home-title-block">
        <button
          v-if="isDetailView"
          class="youtube-monitor-back"
          type="button"
          aria-label="返回待办队列"
          @click="goBackToQueue"
        >
          <ArrowLeft :stroke-width="2.2" aria-hidden="true" />
        </button>
        <div>
          <h1 class="page-title">{{ pageTitle }}</h1>
          <div class="youtube-monitor-status-line" :class="toolStatusClass">
            <span class="translate-status-dot" :class="toolStatusDotClass" aria-hidden="true" />
            <span>{{ ytdlpStatus.message }}</span>
          </div>
        </div>
      </div>

      <div v-if="isDetailView && activeTask" class="translate-actions home-header-actions">
        <button
          class="settings-action youtube-monitor-action primary"
          type="button"
          :disabled="isRefreshingDetail || !ytdlpStatus.isAvailable"
          @click="refreshActiveTaskDetail"
        >
          <LoaderCircle v-if="isRefreshingDetail" class="spinning" :stroke-width="2.1" aria-hidden="true" />
          <RefreshCw v-else :stroke-width="2.1" aria-hidden="true" />
          <span>{{ isRefreshingDetail ? '读取中' : '读取详情' }}</span>
        </button>
        <button
          class="settings-action youtube-monitor-action danger"
          type="button"
          :disabled="isDeletingTask"
          @click="openDeleteDialog(activeTask)"
        >
          <LoaderCircle v-if="isDeletingTask" class="spinning" :stroke-width="2.1" aria-hidden="true" />
          <Trash2 v-else :stroke-width="2.1" aria-hidden="true" />
          <span>{{ isDeletingTask ? '移除中' : '移除待办' }}</span>
        </button>
      </div>
    </header>

    <main class="home-workspace">
      <section v-if="!isDetailView" class="home-list-view">
        <div v-if="pageError" class="translate-alert" role="alert">
          <CircleAlert :stroke-width="2.1" aria-hidden="true" />
          <span>{{ pageError }}</span>
        </div>

        <section class="settings-section" aria-labelledby="home-overview-title">
          <div id="home-overview-title" class="section-heading">
            <ListTodo aria-hidden="true" />
            <span>任务概览</span>
          </div>

          <div class="settings-panel youtube-monitor-summary">
            <div class="youtube-monitor-summary-item">
              <span class="youtube-monitor-summary-value">{{ tasks.length }}</span>
              <span class="youtube-monitor-summary-label">全部任务</span>
            </div>
            <div class="youtube-monitor-summary-item">
              <span class="youtube-monitor-summary-value">{{ pendingCount }}</span>
              <span class="youtube-monitor-summary-label">待读取</span>
            </div>
            <div class="youtube-monitor-summary-item">
              <span class="youtube-monitor-summary-value">{{ readyCount }}</span>
              <span class="youtube-monitor-summary-label">已就绪</span>
            </div>
            <div class="youtube-monitor-summary-item">
              <span class="youtube-monitor-summary-value">{{ subtitleCount }}</span>
              <span class="youtube-monitor-summary-label">已下载字幕</span>
            </div>
          </div>
        </section>

        <section class="settings-section" aria-labelledby="home-task-list-title">
          <div id="home-task-list-title" class="section-heading">
            <ListVideo aria-hidden="true" />
            <span>待办队列</span>
          </div>

          <div class="settings-panel youtube-monitor-panel">
            <div class="youtube-monitor-toolbar">
              <label class="youtube-monitor-search">
                <Search :stroke-width="2.1" aria-hidden="true" />
                <input
                  v-model="taskQuery"
                  type="search"
                  placeholder="搜索标题、博主或地址"
                  aria-label="搜索标题、博主或地址"
                />
              </label>

              <div class="home-task-toolbar-actions">
                <div class="youtube-monitor-filter" role="group" aria-label="任务状态">
                  <button
                    v-for="option in taskFilterOptions"
                    :key="option.value"
                    class="youtube-monitor-filter-button"
                    :class="{ active: taskFilter === option.value }"
                    type="button"
                    :aria-pressed="taskFilter === option.value"
                    @click="taskFilter = option.value"
                  >
                    {{ option.label }}
                  </button>
                </div>

                <button class="settings-action youtube-monitor-action primary" type="button" @click="openAddDialog">
                  <Plus :stroke-width="2.1" aria-hidden="true" />
                  <span>添加视频</span>
                </button>
              </div>
            </div>

            <div v-if="isLoadingTasks" class="youtube-monitor-empty">
              <LoaderCircle class="youtube-monitor-empty-icon spinning" :stroke-width="2.1" aria-hidden="true" />
              <span class="translate-empty-title">正在读取待办队列</span>
            </div>

            <div v-else-if="tasks.length === 0" class="youtube-monitor-empty">
              <ListTodo class="youtube-monitor-empty-icon" :stroke-width="2.1" aria-hidden="true" />
              <span class="translate-empty-title">暂无待办任务</span>
              <span class="home-empty-copy">可以粘贴视频地址，或从监控详情的视频列表加入</span>
            </div>

            <div v-else-if="filteredTasks.length === 0" class="youtube-monitor-empty">
              <Search class="youtube-monitor-empty-icon" :stroke-width="2.1" aria-hidden="true" />
              <span class="translate-empty-title">没有匹配的任务</span>
            </div>

            <div v-else class="home-task-list" role="table" aria-label="主页待办任务列表">
              <RouterLink
                v-for="task in filteredTasks"
                :key="task.id"
                class="home-task-row"
                :to="{ name: 'HomeTaskDetail', params: { taskId: task.id } }"
                :aria-label="`查看待办任务：${task.title || task.url}`"
                role="row"
              >
                <span class="home-task-thumb" role="cell">
                  <img v-if="displayThumbnailUrl(task)" :src="displayThumbnailUrl(task)" alt="" />
                  <Video v-else :stroke-width="2.1" aria-hidden="true" />
                </span>

                <span class="home-task-main" role="cell">
                  <span class="home-task-title">{{ task.title || '待读取视频详情' }}</span>
                  <span class="home-task-url">{{ task.webpageUrl || task.url }}</span>
                </span>

                <span class="home-task-channel" role="cell">
                  <span class="youtube-channel-meta-label">博主</span>
                  <span>{{ task.channelTitle || '--' }}</span>
                </span>

                <span class="youtube-video-meta" role="cell">
                  <Clock :stroke-width="2.1" aria-hidden="true" />
                  {{ formatDuration(task.duration) }}
                </span>

                <span class="youtube-video-status" :class="taskStatusClass(task)" role="cell">
                  {{ taskStatusLabel(task) }}
                </span>

                <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
              </RouterLink>
            </div>
          </div>
        </section>
      </section>

      <section v-else class="home-detail-view">
        <div v-if="isLoadingTasks && !activeTask" class="youtube-monitor-empty">
          <LoaderCircle class="youtube-monitor-empty-icon spinning" :stroke-width="2.1" aria-hidden="true" />
          <span class="translate-empty-title">正在读取任务</span>
        </div>

        <div v-else-if="!activeTask" class="youtube-monitor-empty">
          <CircleAlert class="youtube-monitor-empty-icon" :stroke-width="2.1" aria-hidden="true" />
          <span class="translate-empty-title">未找到该待办任务</span>
          <button class="settings-action youtube-monitor-action" type="button" @click="goBackToQueue">
            返回队列
          </button>
        </div>

        <template v-else>
          <section class="settings-section" aria-labelledby="home-video-detail-title">
            <div class="home-section-heading-line">
              <div id="home-video-detail-title" class="section-heading">
                <BadgeInfo aria-hidden="true" />
                <span>视频详情</span>
              </div>

              <div v-if="detailMessage" class="home-info-strip inline" :class="detailMessageClass">
                <LoaderCircle
                  v-if="isRefreshingDetail"
                  class="spinning"
                  :stroke-width="2.1"
                  aria-hidden="true"
                />
                <CircleAlert v-else-if="activeTask.errorMessage" :stroke-width="2.1" aria-hidden="true" />
                <CheckCircle2 v-else :stroke-width="2.1" aria-hidden="true" />
                <span>{{ detailMessage }}</span>
              </div>
            </div>

            <div class="settings-panel home-detail-panel">
              <div class="home-video-overview">
                <div class="home-video-side">
                  <div class="home-video-cover">
                    <img v-if="displayThumbnailUrl(activeTask)" :src="displayThumbnailUrl(activeTask)" alt="" />
                    <Video v-else :stroke-width="2.1" aria-hidden="true" />
                  </div>

                  <div class="home-detail-stats compact">
                    <div class="youtube-detail-stat">
                      <span class="youtube-monitor-summary-value compact">{{ activeTask.channelTitle || '--' }}</span>
                      <span class="youtube-monitor-summary-label">博主</span>
                    </div>
                    <div class="youtube-detail-stat">
                      <span class="youtube-monitor-summary-value compact">{{ formatDuration(activeTask.duration) }}</span>
                      <span class="youtube-monitor-summary-label">时长</span>
                    </div>
                    <div class="youtube-detail-stat">
                      <span class="youtube-monitor-summary-value compact">{{ formatCount(activeTask.viewCount) }}</span>
                      <span class="youtube-monitor-summary-label">播放</span>
                    </div>
                    <div class="youtube-detail-stat">
                      <span class="youtube-monitor-summary-value compact">{{ formatUploadDate(activeTask.uploadDate) }}</span>
                      <span class="youtube-monitor-summary-label">发布时间</span>
                    </div>
                  </div>
                </div>

                <div class="home-video-copy">
                  <div class="home-video-title-line">
                    <h2>{{ activeTask.title || '待读取视频详情' }}</h2>
                    <span class="youtube-video-status" :class="taskStatusClass(activeTask)">
                      {{ taskStatusLabel(activeTask) }}
                    </span>
                  </div>
                  <a
                    class="youtube-channel-detail-url"
                    :href="activeTask.webpageUrl || activeTask.url"
                    target="_blank"
                    rel="noreferrer"
                  >
                    {{ activeTask.webpageUrl || activeTask.url }}
                  </a>
                  <p v-if="activeTask.description" class="home-video-description">
                    {{ activeTask.description }}
                  </p>
                </div>
              </div>

            </div>
          </section>

          <section class="settings-section" aria-labelledby="home-subtitle-options-title">
            <div id="home-subtitle-options-title" class="section-heading">
              <Captions aria-hidden="true" />
              <span>可下载字幕</span>
            </div>

            <div class="settings-panel youtube-monitor-panel">
              <div v-if="activeTask.subtitleOptions.length === 0" class="youtube-monitor-empty home-subtitle-empty">
                <Captions class="youtube-monitor-empty-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="translate-empty-title">{{ subtitleEmptyText }}</span>
              </div>

              <div v-else class="home-subtitle-option-list">
                <article
                  v-for="option in activeTask.subtitleOptions"
                  :key="`${option.sourceKind}:${option.language}`"
                  class="home-subtitle-row"
                  :class="{ downloaded: Boolean(downloadedSubtitleForOption(option)) }"
                >
                  <span class="home-subtitle-main">
                    <span class="home-subtitle-title-line">
                      <span class="home-subtitle-title">{{ option.name || option.language }}</span>
                      <span v-if="downloadedSubtitleForOption(option)" class="youtube-video-status unread">已下载</span>
                    </span>
                    <span class="home-subtitle-meta">
                      {{ option.language }} · {{ subtitleSourceLabel(option.sourceKind) }} · {{ option.formats.join(' / ') }}
                    </span>
                    <span v-if="downloadedSubtitleForOption(option)" class="home-subtitle-meta">
                      {{ downloadedSubtitleForOption(option)?.format || '字幕' }} · {{ formatFileSize(downloadedSubtitleForOption(option)?.fileSize ?? 0) }}
                    </span>
                  </span>

                  <button
                    class="settings-action youtube-monitor-action"
                    type="button"
                    :disabled="isSubtitleDownloading(option)"
                    @click="downloadSubtitle(option)"
                  >
                    <LoaderCircle
                      v-if="isSubtitleDownloading(option)"
                      class="spinning"
                      :stroke-width="2.1"
                      aria-hidden="true"
                    />
                    <CheckCircle2 v-else-if="downloadedSubtitleForOption(option)" :stroke-width="2.1" aria-hidden="true" />
                    <Download v-else :stroke-width="2.1" aria-hidden="true" />
                    <span>{{ subtitleActionLabel(option) }}</span>
                  </button>
                </article>
              </div>

              <div v-if="subtitleError" class="translate-alert" role="alert">
                <CircleAlert :stroke-width="2.1" aria-hidden="true" />
                <span>{{ subtitleError }}</span>
              </div>
            </div>
          </section>

        </template>
      </section>
    </main>

    <Teleport to="body">
      <div v-if="isAddDialogOpen" class="dialog-backdrop" role="presentation" @click.self="closeAddDialog">
        <section
          class="settings-dialog youtube-monitor-dialog home-add-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="home-add-dialog-title"
        >
          <h2 id="home-add-dialog-title" class="dialog-title">添加视频</h2>

          <div class="settings-panel home-add-panel dialog-panel">
            <label class="home-url-field">
              <Link2 :stroke-width="2.1" aria-hidden="true" />
              <input
                v-model="draftUrl"
                type="url"
                placeholder="粘贴 YouTube 视频地址"
                aria-label="粘贴 YouTube 视频地址"
                autocomplete="off"
                autocapitalize="off"
                autocorrect="off"
                spellcheck="false"
                @keydown.enter.prevent="addTaskFromInput"
              />
            </label>
          </div>

          <div v-if="addError" class="translate-alert" role="alert">
            <CircleAlert :stroke-width="2.1" aria-hidden="true" />
            <span>{{ addError }}</span>
          </div>

          <div class="youtube-dialog-actions">
            <button class="settings-action youtube-monitor-action" type="button" @click="closeAddDialog">取消</button>
            <button
              class="settings-action youtube-monitor-action primary"
              type="button"
              :disabled="isAddingTask || !draftUrl.trim()"
              @click="addTaskFromInput"
            >
              <LoaderCircle v-if="isAddingTask" class="spinning" :stroke-width="2.1" aria-hidden="true" />
              <Plus v-else :stroke-width="2.1" aria-hidden="true" />
              <span>{{ isAddingTask ? '添加中' : '加入待办' }}</span>
            </button>
          </div>
        </section>
      </div>

      <div v-if="isDeleteDialogOpen" class="dialog-backdrop" role="presentation" @click.self="closeDeleteDialog">
        <section
          class="settings-dialog youtube-monitor-dialog home-delete-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="home-delete-dialog-title"
        >
          <h2 id="home-delete-dialog-title" class="dialog-title">移除待办任务</h2>
          <p class="youtube-dialog-copy">移除后会从待办队列删除该任务，并清理已下载的字幕文件。</p>
          <p class="home-delete-target">{{ deleteTargetLabel }}</p>

          <div v-if="deleteError" class="translate-alert" role="alert">
            <CircleAlert :stroke-width="2.1" aria-hidden="true" />
            <span>{{ deleteError }}</span>
          </div>

          <div class="youtube-dialog-actions">
            <button class="settings-action youtube-monitor-action" type="button" @click="closeDeleteDialog">取消</button>
            <button
              class="settings-action youtube-monitor-action danger"
              type="button"
              :disabled="isDeletingTask"
              @click="deleteTask"
            >
              <LoaderCircle v-if="isDeletingTask" class="spinning" :stroke-width="2.1" aria-hidden="true" />
              <Trash2 v-else :stroke-width="2.1" aria-hidden="true" />
              <span>{{ isDeletingTask ? '移除中' : '移除' }}</span>
            </button>
          </div>
        </section>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import {
  ArrowLeft,
  BadgeInfo,
  Captions,
  CheckCircle2,
  ChevronRight,
  CircleAlert,
  Clock,
  Download,
  Link2,
  ListTodo,
  ListVideo,
  LoaderCircle,
  Plus,
  RefreshCw,
  Search,
  Trash2,
  Video,
} from 'lucide-vue-next'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'

defineOptions({ name: 'Home' })

type TaskStatusFilter = 'all' | 'pending' | 'ready' | 'failed'

type YtdlpStatus = {
  isAvailable: boolean
  version: string
  message: string
}

type HomeVideoSubtitleOption = {
  language: string
  name: string
  sourceKind: string
  formats: string[]
  isAuto: boolean
}

type HomeVideoSubtitle = {
  id: string
  taskId: string
  language: string
  languageName: string
  sourceKind: string
  format: string
  filePath: string
  fileSize: number
  createdAt: string
  updatedAt: string
}

type HomeVideoTask = {
  id: string
  url: string
  sourceChannelId: string
  sourceVideoId: string
  externalId: string
  title: string
  channelTitle: string
  channelUrl: string
  thumbnailUrl: string
  duration?: number | null
  webpageUrl: string
  description: string
  viewCount?: number | null
  likeCount?: number | null
  commentCount?: number | null
  uploadDate: string
  detailStatus: string
  subtitleOptions: HomeVideoSubtitleOption[]
  metadata: Record<string, unknown>
  errorMessage: string
  createdAt: string
  updatedAt: string
  detailCheckedAt?: string | null
  downloadedSubtitles: HomeVideoSubtitle[]
}

const route = useRoute()
const router = useRouter()

const tasks = ref<HomeVideoTask[]>([])
const ytdlpStatus = ref<YtdlpStatus>({
  isAvailable: false,
  version: '',
  message: '正在检测 yt-dlp',
})
const draftUrl = ref('')
const taskQuery = ref('')
const taskFilter = ref<TaskStatusFilter>('all')
const isLoadingTasks = ref(false)
const isAddingTask = ref(false)
const isDeletingTask = ref(false)
const isRefreshingDetail = ref(false)
const isAddDialogOpen = ref(false)
const isDeleteDialogOpen = ref(false)
const pageError = ref('')
const addError = ref('')
const deleteError = ref('')
const subtitleError = ref('')
const downloadingSubtitleKeys = ref(new Set<string>())
const autoRefreshedTaskIds = ref(new Set<string>())
const taskPendingDelete = ref<HomeVideoTask | null>(null)

const taskFilterOptions: { value: TaskStatusFilter; label: string }[] = [
  { value: 'all', label: '全部' },
  { value: 'pending', label: '待读取' },
  { value: 'ready', label: '已就绪' },
  { value: 'failed', label: '异常' },
]

const isTauriRuntime = () => '__TAURI_INTERNALS__' in window

const activeTaskId = computed(() => {
  const value = route.params.taskId
  return typeof value === 'string' ? value : ''
})

const isDetailView = computed(() => Boolean(activeTaskId.value))

const activeTask = computed(() => tasks.value.find((task) => task.id === activeTaskId.value) ?? null)

const pageTitle = computed(() => {
  if (!isDetailView.value) {
    return '主页'
  }

  return activeTask.value?.title || '视频详情'
})

const filteredTasks = computed(() => {
  const query = taskQuery.value.trim().toLowerCase()
  return tasks.value.filter((task) => {
    const status = normalizedTaskStatus(task)
    const matchesStatus =
      taskFilter.value === 'all' ||
      (taskFilter.value === 'pending' && (status === 'pending' || status === 'loading')) ||
      (taskFilter.value === 'ready' && status === 'ready') ||
      (taskFilter.value === 'failed' && status === 'failed')
    const matchesQuery =
      !query ||
      task.title.toLowerCase().includes(query) ||
      task.channelTitle.toLowerCase().includes(query) ||
      task.url.toLowerCase().includes(query) ||
      task.webpageUrl.toLowerCase().includes(query)

    return matchesStatus && matchesQuery
  })
})

const pendingCount = computed(() => {
  return tasks.value.filter((task) => ['pending', 'loading'].includes(normalizedTaskStatus(task))).length
})
const readyCount = computed(() => tasks.value.filter((task) => normalizedTaskStatus(task) === 'ready').length)
const subtitleCount = computed(() => {
  return tasks.value.reduce((total, task) => total + task.downloadedSubtitles.length, 0)
})

const toolStatusClass = computed(() => ({
  ready: ytdlpStatus.value.isAvailable,
  error: !ytdlpStatus.value.isAvailable,
}))

const toolStatusDotClass = computed(() => (ytdlpStatus.value.isAvailable ? 'success' : 'error'))

const detailMessage = computed(() => {
  if (!activeTask.value) {
    return ''
  }
  if (isRefreshingDetail.value || activeTask.value.detailStatus === 'loading') {
    return '正在使用 yt-dlp 读取视频详情'
  }
  if (activeTask.value.errorMessage) {
    return activeTask.value.errorMessage
  }
  if (activeTask.value.detailStatus === 'ready') {
    return activeTask.value.detailCheckedAt
      ? `详情已读取 · ${formatDateTime(activeTask.value.detailCheckedAt)}`
      : '详情已读取'
  }

  return '等待读取视频详情'
})

const detailMessageClass = computed(() => ({
  error: Boolean(activeTask.value?.errorMessage),
  ready: activeTask.value?.detailStatus === 'ready' && !activeTask.value?.errorMessage,
}))

const subtitleEmptyText = computed(() => {
  if (isRefreshingDetail.value) {
    return '正在读取字幕选项'
  }
  if (activeTask.value?.detailStatus === 'ready') {
    return '该视频没有可下载字幕'
  }
  return '读取视频详情后会显示可下载字幕'
})

const deleteTargetLabel = computed(() => {
  const task = taskPendingDelete.value
  if (!task) {
    return '该待办任务'
  }

  return task.title || task.webpageUrl || task.url
})

watch(activeTaskId, async () => {
  subtitleError.value = ''
  if (!activeTaskId.value) {
    return
  }
  await ensureActiveTaskLoaded()
  maybeAutoRefreshActiveTask()
})

onMounted(async () => {
  window.addEventListener('keydown', handleKeydown)
  await loadAll()
  maybeAutoRefreshActiveTask()
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown)
})

const loadAll = async () => {
  await Promise.all([loadYtdlpStatus(), loadTasks()])
}

const loadYtdlpStatus = async () => {
  if (!isTauriRuntime()) {
    ytdlpStatus.value = {
      isAvailable: false,
      version: '',
      message: '请在桌面应用中使用主页任务',
    }
    return
  }

  ytdlpStatus.value = await invoke<YtdlpStatus>('get_ytdlp_status')
}

const loadTasks = async () => {
  pageError.value = ''

  if (!isTauriRuntime()) {
    tasks.value = []
    pageError.value = '请在桌面应用中读取待办队列'
    return
  }

  isLoadingTasks.value = true
  try {
    tasks.value = await invoke<HomeVideoTask[]>('list_home_video_tasks')
  } catch (error) {
    pageError.value = stringifyError(error, '读取待办队列失败')
  } finally {
    isLoadingTasks.value = false
  }
}

const ensureActiveTaskLoaded = async () => {
  if (!activeTaskId.value || activeTask.value || !isTauriRuntime()) {
    return
  }

  isLoadingTasks.value = true
  try {
    const task = await invoke<HomeVideoTask>('get_home_video_task', {
      request: { taskId: activeTaskId.value },
    })
    upsertTask(task)
  } catch (error) {
    pageError.value = stringifyError(error, '读取待办任务失败')
  } finally {
    isLoadingTasks.value = false
  }
}

const addTaskFromInput = async () => {
  const url = draftUrl.value.trim()
  if (!url || isAddingTask.value || !isTauriRuntime()) {
    return
  }

  isAddingTask.value = true
  addError.value = ''

  try {
    const task = await invoke<HomeVideoTask>('add_home_video_task', {
      request: { url },
    })
    upsertTask(task)
    draftUrl.value = ''
    closeAddDialog()
    await router.push({ name: 'HomeTaskDetail', params: { taskId: task.id } })
    maybeAutoRefreshActiveTask()
  } catch (error) {
    addError.value = stringifyError(error, '添加待办任务失败')
  } finally {
    isAddingTask.value = false
  }
}

const maybeAutoRefreshActiveTask = () => {
  if (!activeTask.value || !ytdlpStatus.value.isAvailable || isRefreshingDetail.value) {
    return
  }
  const status = normalizedTaskStatus(activeTask.value)
  const needsThumbnailRefresh = hasRemoteThumbnail(activeTask.value)
  if (
    status === 'loading' ||
    (status === 'ready' && !needsThumbnailRefresh) ||
    autoRefreshedTaskIds.value.has(activeTask.value.id)
  ) {
    return
  }
  autoRefreshedTaskIds.value = new Set(autoRefreshedTaskIds.value).add(activeTask.value.id)
  void refreshActiveTaskDetail()
}

const refreshActiveTaskDetail = async () => {
  if (!activeTask.value || isRefreshingDetail.value || !isTauriRuntime()) {
    return
  }

  const refreshingTaskId = activeTask.value.id
  isRefreshingDetail.value = true
  pageError.value = ''

  try {
    const task = await invoke<HomeVideoTask>('refresh_home_video_task_detail', {
      request: { taskId: refreshingTaskId },
    })
    if (!hasTask(refreshingTaskId)) {
      return
    }
    upsertTask(task)
  } catch (error) {
    if (!hasTask(refreshingTaskId)) {
      return
    }
    pageError.value = stringifyError(error, '读取视频详情失败')
    await reloadActiveTask()
  } finally {
    isRefreshingDetail.value = false
  }
}

const reloadActiveTask = async () => {
  if (!activeTaskId.value || !isTauriRuntime()) {
    return
  }

  try {
    const task = await invoke<HomeVideoTask>('get_home_video_task', {
      request: { taskId: activeTaskId.value },
    })
    upsertTask(task)
  } catch {
    // 页面顶部已经展示了主错误，这里不覆盖上下文。
  }
}

const downloadSubtitle = async (option: HomeVideoSubtitleOption) => {
  if (!activeTask.value || !isTauriRuntime()) {
    return
  }

  const key = subtitleKey(option)
  if (downloadingSubtitleKeys.value.has(key)) {
    return
  }

  const next = new Set(downloadingSubtitleKeys.value)
  next.add(key)
  downloadingSubtitleKeys.value = next
  subtitleError.value = ''

  try {
    const task = await invoke<HomeVideoTask>('download_home_video_task_subtitle', {
      request: {
        taskId: activeTask.value.id,
        language: option.language,
        sourceKind: option.sourceKind,
      },
    })
    upsertTask(task)
  } catch (error) {
    subtitleError.value = stringifyError(error, '下载字幕失败')
  } finally {
    const cleared = new Set(downloadingSubtitleKeys.value)
    cleared.delete(key)
    downloadingSubtitleKeys.value = cleared
  }
}

const deleteTask = async () => {
  const task = taskPendingDelete.value
  if (!task || isDeletingTask.value || !isTauriRuntime()) {
    return
  }

  isDeletingTask.value = true
  deleteError.value = ''

  try {
    await invoke('delete_home_video_task', {
      request: { taskId: task.id },
    })
    removeTask(task.id)
    const shouldReturnToQueue = activeTaskId.value === task.id
    isDeleteDialogOpen.value = false
    taskPendingDelete.value = null
    if (shouldReturnToQueue) {
      await router.push({ name: 'Home' })
    }
  } catch (error) {
    deleteError.value = stringifyError(error, '移除待办任务失败')
  } finally {
    isDeletingTask.value = false
  }
}

const upsertTask = (task: HomeVideoTask) => {
  const index = tasks.value.findIndex((item) => item.id === task.id)
  if (index >= 0) {
    const next = [...tasks.value]
    next[index] = task
    tasks.value = next
    return
  }

  tasks.value = [task, ...tasks.value]
}

const removeTask = (taskId: string) => {
  tasks.value = tasks.value.filter((task) => task.id !== taskId)
  const refreshed = new Set(autoRefreshedTaskIds.value)
  refreshed.delete(taskId)
  autoRefreshedTaskIds.value = refreshed
}

const hasTask = (taskId: string) => {
  return tasks.value.some((task) => task.id === taskId)
}

const goBackToQueue = async () => {
  await router.push({ name: 'Home' })
}

const openAddDialog = () => {
  draftUrl.value = ''
  addError.value = ''
  isAddDialogOpen.value = true
}

const closeAddDialog = () => {
  if (isAddingTask.value) {
    return
  }
  isAddDialogOpen.value = false
}

const openDeleteDialog = (task: HomeVideoTask) => {
  taskPendingDelete.value = task
  deleteError.value = ''
  isDeleteDialogOpen.value = true
}

const closeDeleteDialog = () => {
  if (isDeletingTask.value) {
    return
  }
  isDeleteDialogOpen.value = false
  taskPendingDelete.value = null
  deleteError.value = ''
}

const handleKeydown = (event: KeyboardEvent) => {
  if (event.key === 'Escape') {
    closeAddDialog()
    closeDeleteDialog()
  }
}

const normalizedTaskStatus = (task: HomeVideoTask) => task.detailStatus || 'pending'

const taskStatusLabel = (task: HomeVideoTask) => {
  const status = normalizedTaskStatus(task)
  if (status === 'loading') {
    return '读取中'
  }
  if (status === 'ready') {
    return '已就绪'
  }
  if (status === 'failed') {
    return '异常'
  }
  return '待读取'
}

const taskStatusClass = (task: HomeVideoTask) => ({
  unread: normalizedTaskStatus(task) === 'ready',
  checking: normalizedTaskStatus(task) === 'loading',
  failed: normalizedTaskStatus(task) === 'failed',
})

const subtitleKey = (option: HomeVideoSubtitleOption) => `${option.sourceKind}:${option.language}`

const isSubtitleDownloading = (option: HomeVideoSubtitleOption) => {
  return downloadingSubtitleKeys.value.has(subtitleKey(option))
}

const downloadedSubtitleForOption = (option: HomeVideoSubtitleOption) => {
  return activeTask.value?.downloadedSubtitles.find((subtitle) => {
    return subtitle.language === option.language && subtitle.sourceKind === option.sourceKind
  }) ?? null
}

const subtitleActionLabel = (option: HomeVideoSubtitleOption) => {
  if (isSubtitleDownloading(option)) {
    return '下载中'
  }
  if (downloadedSubtitleForOption(option)) {
    return '重新下载'
  }
  return '下载字幕'
}

const subtitleSourceLabel = (sourceKind: string) => (sourceKind === 'automatic' ? '自动字幕' : '手动字幕')

const displayThumbnailUrl = (task: HomeVideoTask) => {
  return isInlineThumbnailUrl(task.thumbnailUrl) ? task.thumbnailUrl : ''
}

const hasRemoteThumbnail = (task: HomeVideoTask) => {
  return /^https?:\/\//i.test(task.thumbnailUrl)
}

const isInlineThumbnailUrl = (value: string) => {
  return value.startsWith('data:image/')
}

const formatDuration = (duration?: number | null) => {
  if (!duration || duration <= 0) {
    return '--'
  }

  const totalSeconds = Math.round(duration)
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  const padded = `${minutes.toString().padStart(hours > 0 ? 2 : 1, '0')}:${seconds
    .toString()
    .padStart(2, '0')}`
  return hours > 0 ? `${hours}:${padded}` : padded
}

const formatCount = (value?: number | null) => {
  if (value === null || value === undefined || Number.isNaN(value)) {
    return '--'
  }
  if (value >= 10000) {
    return `${(value / 10000).toFixed(value >= 100000 ? 0 : 1)}万`
  }
  return value.toLocaleString('zh-CN')
}

const formatUploadDate = (value: string) => {
  if (!value) {
    return '--'
  }
  if (/^\d{8}$/.test(value)) {
    return `${value.slice(0, 4)}-${value.slice(4, 6)}-${value.slice(6, 8)}`
  }
  return value
}

const formatDateTime = (value?: string | null) => {
  if (!value) {
    return '--'
  }

  const date = new Date(value)
  if (Number.isNaN(date.getTime())) {
    return value
  }

  return date.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

const formatFileSize = (value: number) => {
  if (!value || value <= 0) {
    return '--'
  }
  if (value >= 1024 * 1024) {
    return `${(value / 1024 / 1024).toFixed(1)} MB`
  }
  if (value >= 1024) {
    return `${(value / 1024).toFixed(1)} KB`
  }
  return `${value} B`
}

const stringifyError = (error: unknown, fallback: string) => {
  if (typeof error === 'string' && error.trim()) {
    return error
  }
  if (error instanceof Error && error.message.trim()) {
    return error.message
  }
  return fallback
}
</script>
