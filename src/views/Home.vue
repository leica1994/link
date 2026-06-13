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
                <div ref="videoSideRef" class="home-video-side">
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

                <div ref="videoCopyRef" class="home-video-copy">
                  <div ref="videoTitleLineRef" class="home-video-title-line">
                    <h2>{{ activeTask.title || '待读取视频详情' }}</h2>
                    <span class="youtube-video-status" :class="taskStatusClass(activeTask)">
                      {{ taskStatusLabel(activeTask) }}
                    </span>
                  </div>
                  <a
                    ref="videoUrlRef"
                    class="youtube-channel-detail-url"
                    :href="activeTask.webpageUrl || activeTask.url"
                    target="_blank"
                    rel="noreferrer"
                  >
                    {{ activeTask.webpageUrl || activeTask.url }}
                  </a>
                  <p v-if="activeTask.description" class="home-video-description" :style="videoDescriptionStyle">
                    {{ activeTask.description }}
                  </p>
                </div>
              </div>

            </div>
          </section>

          <section class="settings-section" aria-labelledby="home-video-download-title">
            <div id="home-video-download-title" class="section-heading">
              <Video aria-hidden="true" />
              <span>视频文件</span>
            </div>

            <div class="settings-panel youtube-monitor-panel home-video-download-panel">
              <div
                class="home-video-download-strip"
                :class="{
                  downloaded: Boolean(activeTask.downloadedVideo),
                  partial: hasPartialVideoDownload,
                }"
              >
                <span class="home-video-download-copy">
                  <span class="home-video-download-title">
                    <Video :stroke-width="2.1" aria-hidden="true" />
                    <span>视频文件</span>
                    <span v-if="activeTask.downloadedVideo" class="youtube-video-status unread">已下载</span>
                    <span v-else-if="hasPartialVideoDownload" class="youtube-video-status checking">未完成</span>
                  </span>
                  <span class="home-video-download-meta">
                    {{ videoDownloadMeta }}
                  </span>
                  <span
                    v-if="isActiveTaskDownloadingVideo && videoDownloadProgressMessage"
                    class="home-download-progress-message"
                  >
                    {{ videoDownloadProgressMessage }}
                  </span>
                </span>

                <span class="home-video-download-actions">
                  <button
                    class="settings-action youtube-monitor-action home-video-download-button"
                    type="button"
                    :disabled="isActiveTaskDownloadingVideo || !ytdlpStatus.isAvailable"
                    @click="downloadVideo"
                  >
                    <LoaderCircle
                      v-if="isActiveTaskDownloadingVideo"
                      class="spinning"
                      :stroke-width="2.1"
                      aria-hidden="true"
                    />
                    <CheckCircle2 v-else-if="activeTask.downloadedVideo" :stroke-width="2.1" aria-hidden="true" />
                    <Download v-else :stroke-width="2.1" aria-hidden="true" />
                    <span>{{ videoActionLabel }}</span>
                  </button>

                  <button
                    v-if="activeTask.downloadedVideo"
                    class="settings-action youtube-monitor-action"
                    type="button"
                    :disabled="isVideoAddedToWorkbench || isAddingWorkbenchVideo || isWorkbenchRunning"
                    @click="addVideoToWorkbench"
                  >
                    <LoaderCircle v-if="isAddingWorkbenchVideo" class="spinning" :stroke-width="2.1" aria-hidden="true" />
                    <CheckCircle2 v-else-if="isVideoAddedToWorkbench" :stroke-width="2.1" aria-hidden="true" />
                    <Plus v-else :stroke-width="2.1" aria-hidden="true" />
                    <span>{{ isVideoAddedToWorkbench ? '已加入工作台' : '添加到工作台' }}</span>
                  </button>
                </span>
              </div>

              <div
                v-if="isActiveTaskDownloadingVideo"
                class="home-download-progress"
                role="progressbar"
                aria-label="视频下载进度"
                :aria-valuenow="videoDownloadProgressValue"
                aria-valuemin="0"
                aria-valuemax="100"
              >
                <div class="translate-progress-track">
                  <span class="translate-progress-bar" :style="{ width: `${videoDownloadProgressValue}%` }" />
                </div>
                <span class="translate-progress-value">{{ videoDownloadProgressValue }}%</span>
              </div>

              <div v-if="videoError" class="translate-alert compact home-download-alert" role="alert">
                <CircleAlert :stroke-width="2.1" aria-hidden="true" />
                <span>{{ videoError }}</span>
              </div>
            </div>
          </section>

          <section class="settings-section" aria-labelledby="home-subtitle-options-title">
            <div id="home-subtitle-options-title" class="section-heading">
              <Captions aria-hidden="true" />
              <span>字幕文件</span>
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

                  <button
                    v-if="downloadedSubtitleForOption(option)"
                    class="settings-action youtube-monitor-action"
                    type="button"
                    :disabled="isSubtitleAddedToWorkbench(option) || isAddingSubtitleToWorkbench(option) || isWorkbenchRunning"
                    @click="addSubtitleOptionToWorkbench(option)"
                  >
                    <LoaderCircle
                      v-if="isAddingSubtitleToWorkbench(option)"
                      class="spinning"
                      :stroke-width="2.1"
                      aria-hidden="true"
                    />
                    <CheckCircle2
                      v-else-if="isSubtitleAddedToWorkbench(option)"
                      :stroke-width="2.1"
                      aria-hidden="true"
                    />
                    <Plus v-else :stroke-width="2.1" aria-hidden="true" />
                    <span>{{ isSubtitleAddedToWorkbench(option) ? '已加入工作台' : '添加到工作台' }}</span>
                  </button>

                  <div
                    v-if="isSubtitleDownloading(option)"
                    class="home-download-progress home-subtitle-progress"
                    role="progressbar"
                    :aria-label="`${option.name || option.language} 字幕下载进度`"
                    :aria-valuenow="subtitleDownloadProgressValue(option)"
                    aria-valuemin="0"
                    aria-valuemax="100"
                  >
                    <div class="home-download-progress-copy">
                      <span>{{ subtitleDownloadProgressMessage(option) }}</span>
                    </div>
                    <div class="translate-progress-track">
                      <span
                        class="translate-progress-bar"
                        :style="{ width: `${subtitleDownloadProgressValue(option)}%` }"
                      />
                    </div>
                    <span class="translate-progress-value">{{ subtitleDownloadProgressValue(option) }}%</span>
                  </div>
                </article>
              </div>

              <div v-if="subtitleError" class="translate-alert" role="alert">
                <CircleAlert :stroke-width="2.1" aria-hidden="true" />
                <span>{{ subtitleError }}</span>
              </div>
            </div>
          </section>

          <section class="settings-section" aria-labelledby="home-workbench-title">
            <div class="home-section-heading-line">
              <div id="home-workbench-title" class="section-heading">
                <Workflow aria-hidden="true" />
                <span>任务工作台</span>
              </div>

              <div class="home-info-strip inline" :class="workbenchMessageClass">
                <LoaderCircle v-if="isWorkbenchRunning" class="spinning" :stroke-width="2.1" aria-hidden="true" />
                <CircleAlert v-else-if="workbenchSnapshot?.errorMessage" :stroke-width="2.1" aria-hidden="true" />
                <CheckCircle2 v-else :stroke-width="2.1" aria-hidden="true" />
                <span>{{ workbenchStatusText }}</span>
              </div>
            </div>

            <div class="settings-panel home-workbench-panel">
              <div class="home-workbench-top">
                <div class="home-workbench-status">
                  <div class="translate-status">
                    <span class="translate-status-dot" :class="workbenchStatusDotClass" aria-hidden="true" />
                    <span>{{ workbenchMainMessage }}</span>
                  </div>
                </div>

                <button
                  class="settings-action youtube-monitor-action primary home-workbench-run"
                  type="button"
                  :disabled="!canStartWorkbench"
                  @click="startWorkbench"
                >
                  <LoaderCircle v-if="isWorkbenchRunning" class="spinning" :stroke-width="2.1" aria-hidden="true" />
                  <Play v-else :stroke-width="2.1" aria-hidden="true" />
                  <span>{{ workbenchRunLabel }}</span>
                </button>

                <button
                  v-if="exportedArtifacts.length > 0"
                  class="settings-action youtube-monitor-action"
                  type="button"
                  @click="openWorkbenchOutput"
                >
                  <FolderOpen :stroke-width="2.1" aria-hidden="true" />
                  <span>打开导出</span>
                </button>
              </div>

              <div class="home-workbench-config-list" aria-label="工作台参数">
                <button
                  class="home-workbench-config-button"
                  type="button"
                  :aria-label="`配置字幕参数，当前 ${workbenchTranscriptionModelLabel}`"
                  @click="openWorkbenchParameterPanel(WorkbenchParameterPanel.Subtitle)"
                >
                  <span class="home-workbench-config-icon">
                    <Captions :stroke-width="2.1" aria-hidden="true" />
                  </span>
                  <span class="home-workbench-config-copy">
                    <span class="home-workbench-config-title">字幕</span>
                    <span class="home-workbench-config-subtitle">
                      {{ workbenchTranscriptionModelLabel }} · {{ workbenchSourceLanguageLabel }}
                    </span>
                    <span class="home-workbench-config-meta">
                      <span>输出 {{ workbenchTranscriptionFormatLabel }}</span>
                      <span>断句 {{ workbenchOptions.isSmartSegmentationEnabled ? '开' : '关' }}</span>
                    </span>
                  </span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </button>

                <button
                  class="home-workbench-config-button"
                  type="button"
                  :aria-label="`配置翻译参数，当前 ${workbenchTargetLanguageLabel}`"
                  @click="openWorkbenchParameterPanel(WorkbenchParameterPanel.Translation)"
                >
                  <span class="home-workbench-config-icon">
                    <WandSparkles :stroke-width="2.1" aria-hidden="true" />
                  </span>
                  <span class="home-workbench-config-copy">
                    <span class="home-workbench-config-title">翻译</span>
                    <span class="home-workbench-config-subtitle">
                      {{ workbenchVideoContentTypeLabel }} · {{ workbenchTargetLanguageLabel }}
                    </span>
                    <span class="home-workbench-config-meta">
                      <span>{{ workbenchOutputModeLabel }}</span>
                      <span>输出 {{ workbenchTranslationFormatLabel }}</span>
                    </span>
                  </span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </button>

                <button
                  class="home-workbench-config-button"
                  type="button"
                  :aria-label="`配置配音参数，当前 ${workbenchOptions.dubbingTtsIntervalMs} 毫秒`"
                  @click="openWorkbenchParameterPanel(WorkbenchParameterPanel.Dubbing)"
                >
                  <span class="home-workbench-config-icon">
                    <MicVocal :stroke-width="2.1" aria-hidden="true" />
                  </span>
                  <span class="home-workbench-config-copy">
                    <span class="home-workbench-config-title">配音</span>
                    <span class="home-workbench-config-subtitle">
                      {{ workbenchReferenceAudioLabel }} · {{ workbenchOptions.dubbingTtsIntervalMs }} 毫秒
                    </span>
                    <span class="home-workbench-config-meta">
                      <span>背景音乐 {{ workbenchOptions.dubbingIsBackgroundMusicEnabled ? '开' : '关' }}</span>
                      <span>音量 {{ workbenchOptions.dubbingBackgroundMusicVolume.toFixed(1) }}</span>
                    </span>
                  </span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </button>

                <button
                  class="home-workbench-config-button"
                  type="button"
                  :aria-label="`配置导出参数，当前 ${workbenchExportDirLabel}`"
                  @click="openWorkbenchParameterPanel(WorkbenchParameterPanel.Export)"
                >
                  <span class="home-workbench-config-icon">
                    <FolderOpen :stroke-width="2.1" aria-hidden="true" />
                  </span>
                  <span class="home-workbench-config-copy">
                    <span class="home-workbench-config-title">导出</span>
                    <span class="home-workbench-config-subtitle">{{ workbenchExportDirLabel }}</span>
                    <span class="home-workbench-config-meta">
                      <span>视频与字幕最终产物</span>
                    </span>
                  </span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </button>
              </div>

              <div class="home-workbench-stage-list" aria-label="工作台执行步骤">
                <button
                  v-for="stage in workbenchStages"
                  :key="stage.key"
                  class="home-workbench-stage"
                  :class="[stage.status, { selected: selectedWorkbenchStage?.key === stage.key }]"
                  type="button"
                  :aria-pressed="selectedWorkbenchStage?.key === stage.key"
                  @click="selectWorkbenchStage(stage)"
                >
                  <span class="home-workbench-stage-mark">
                    <CheckCircle2 v-if="stage.status === 'done' || stage.status === 'skipped'" :stroke-width="2.1" aria-hidden="true" />
                    <CircleAlert v-else-if="stage.status === 'failed'" :stroke-width="2.1" aria-hidden="true" />
                    <LoaderCircle v-else-if="stage.status === 'active'" class="spinning" :stroke-width="2.1" aria-hidden="true" />
                    <span v-else aria-hidden="true">{{ stageOrderLabel(stage.key) }}</span>
                  </span>
                  <span class="home-workbench-stage-copy">
                    <span class="home-workbench-stage-title">{{ stage.label }}</span>
                    <span class="home-workbench-stage-message">{{ stage.message }}</span>
                    <span
                      class="home-workbench-stage-progress"
                      role="progressbar"
                      :aria-label="`${stage.label}进度`"
                      :aria-valuenow="stage.progress"
                      aria-valuemin="0"
                      aria-valuemax="100"
                    >
                      <span class="translate-progress-track">
                        <span class="translate-progress-bar" :style="{ width: `${stage.progress}%` }" />
                      </span>
                    </span>
                  </span>
                  <span class="home-workbench-stage-value">{{ stage.progress }}%</span>
                </button>
              </div>

              <div v-if="selectedWorkbenchStage" class="home-workbench-detail" aria-live="polite">
                <div class="home-workbench-detail-header">
                  <span class="home-workbench-detail-title">{{ selectedWorkbenchStage.label }}详情</span>
                  <span class="home-workbench-detail-status" :class="selectedWorkbenchStage.status">
                    {{ workbenchStageStatusLabel(selectedWorkbenchStage.status) }}
                  </span>
                </div>

                <div v-if="selectedWorkbenchStage.key === 'download-video'" class="home-workbench-detail-file">
                  <FileCheck2 :stroke-width="2.1" aria-hidden="true" />
                  <span class="home-workbench-artifact-copy">
                    <span class="home-workbench-artifact-title">
                      {{ isVideoAddedToWorkbench ? '工作台视频已就绪' : selectedWorkbenchStage.message }}
                    </span>
                    <span class="home-workbench-artifact-path">
                      {{ readStringValue(selectedWorkbenchStageSnapshot.path) || activeTask.downloadedVideo?.filePath || '等待视频下载或添加到工作台' }}
                    </span>
                  </span>
                </div>

                <template v-else-if="selectedWorkbenchStage.key === 'prepare-subtitle'">
                  <div v-if="workbenchPrepareSubtitleSteps.length > 0" class="home-workbench-detail-steps">
                    <div
                      v-for="step in workbenchPrepareSubtitleSteps"
                      :key="step.key"
                      class="dubbing-media-step"
                      :class="step.status"
                    >
                      <div class="dubbing-media-step-top">
                        <span class="dubbing-stage-mark" :class="step.status" aria-hidden="true" />
                        <span class="dubbing-media-step-title">{{ step.label }}</span>
                        <span class="dubbing-media-step-status">{{ workbenchStageStatusLabel(step.status) }}</span>
                      </div>
                      <span class="dubbing-media-step-subtitle">{{ step.message }}</span>
                      <div class="dubbing-media-step-progress">
                        <span class="dubbing-media-step-track" role="progressbar" :aria-valuenow="step.progress" aria-valuemin="0" aria-valuemax="100" :aria-label="`${step.label}进度`">
                          <span class="dubbing-media-step-bar" :style="{ width: `${step.progress}%` }" aria-hidden="true" />
                        </span>
                        <span class="dubbing-media-step-progress-value">{{ step.progress }}%</span>
                      </div>
                    </div>
                  </div>

                  <div v-if="workbenchSubtitleSegments.length > 0" class="translate-preview translate-subtitle-list home-workbench-subtitle-preview">
                    <article
                      v-for="(segment, index) in workbenchSubtitleSegments"
                      :key="segment.uid || `workbench-subtitle-${index}`"
                      class="translate-subtitle-row"
                    >
                      <span class="translate-subtitle-index">{{ index + 1 }}</span>
                      <span class="translate-subtitle-status" :class="`status-${normalizeSegmentStatus(segment.status)}`">
                        {{ segmentStatusLabel(segment.status) }}
                      </span>
                      <span class="translate-subtitle-time translate-subtitle-start">{{ formatSegmentTime(segment.startTime) }}</span>
                      <span class="translate-subtitle-time translate-subtitle-end">{{ formatSegmentTime(segment.endTime) }}</span>
                      <p>{{ segment.text }}</p>
                    </article>
                  </div>
                  <div v-else class="home-workbench-detail-file">
                    <Captions :stroke-width="2.1" aria-hidden="true" />
                    <span class="home-workbench-artifact-copy">
                      <span class="home-workbench-artifact-title">{{ selectedWorkbenchStage.message }}</span>
                      <span class="home-workbench-artifact-path">
                        {{ readStringValue(selectedWorkbenchStageSnapshot.path) || '等待转录或添加下载字幕' }}
                      </span>
                    </span>
                  </div>
                </template>

                <template v-else-if="selectedWorkbenchStage.key === 'translation'">
                  <div v-if="workbenchTranslationSteps.length > 0" class="home-workbench-detail-steps">
                    <div
                      v-for="step in workbenchTranslationSteps"
                      :key="step.key"
                      class="dubbing-media-step"
                      :class="step.status"
                    >
                      <div class="dubbing-media-step-top">
                        <span class="dubbing-stage-mark" :class="step.status" aria-hidden="true" />
                        <span class="dubbing-media-step-title">{{ step.label }}</span>
                        <span class="dubbing-media-step-status">{{ workbenchStageStatusLabel(step.status) }}</span>
                      </div>
                      <span class="dubbing-media-step-subtitle">{{ step.message }}</span>
                      <div class="dubbing-media-step-progress">
                        <span class="dubbing-media-step-track" role="progressbar" :aria-valuenow="step.progress" aria-valuemin="0" aria-valuemax="100" :aria-label="`${step.label}进度`">
                          <span class="dubbing-media-step-bar" :style="{ width: `${step.progress}%` }" aria-hidden="true" />
                        </span>
                        <span class="dubbing-media-step-progress-value">{{ step.progress }}%</span>
                      </div>
                    </div>
                  </div>

                  <div v-if="workbenchTranslationRows.length > 0" class="translate-preview translate-subtitle-list translate-translation-table home-workbench-subtitle-preview" role="table" aria-label="工作台翻译详情">
                    <article v-for="row in workbenchTranslationRows" :key="row.key" class="translate-translation-row" role="row">
                      <span class="translate-subtitle-index" role="cell">{{ row.index }}</span>
                      <span class="translate-subtitle-status" :class="`status-${normalizeSegmentStatus(row.status)}`" role="cell">
                        {{ segmentStatusLabel(row.status) }}
                      </span>
                      <span class="translate-subtitle-time translate-subtitle-start translate-translation-start" role="cell">{{ formatSegmentTime(row.startTime) }}</span>
                      <span class="translate-subtitle-time translate-subtitle-end translate-translation-end" role="cell">{{ formatSegmentTime(row.endTime) }}</span>
                      <p class="translate-translation-source" role="cell">{{ row.sourceText }}</p>
                      <p class="translate-translation-target" :class="{ empty: !row.targetText }" role="cell">
                        {{ row.targetText || '等待处理' }}
                      </p>
                    </article>
                  </div>
                  <div v-else class="home-workbench-detail-file">
                    <WandSparkles :stroke-width="2.1" aria-hidden="true" />
                    <span class="home-workbench-artifact-copy">
                      <span class="home-workbench-artifact-title">{{ selectedWorkbenchStage.message }}</span>
                      <span class="home-workbench-artifact-path">{{ readStringValue(selectedWorkbenchStageSnapshot.path) || '等待字幕翻译' }}</span>
                    </span>
                  </div>
                </template>

                <template v-else-if="selectedWorkbenchStage.key === 'dubbing'">
                  <div v-if="workbenchDubbingSteps.length > 0" class="home-workbench-detail-steps">
                    <div
                      v-for="step in workbenchDubbingSteps"
                      :key="step.key"
                      class="dubbing-media-step"
                      :class="step.status"
                    >
                      <div class="dubbing-media-step-top">
                        <span class="dubbing-stage-mark" :class="step.status" aria-hidden="true" />
                        <span class="dubbing-media-step-title">{{ step.label }}</span>
                        <span class="dubbing-media-step-status">{{ workbenchStageStatusLabel(step.status) }}</span>
                      </div>
                      <span class="dubbing-media-step-subtitle">{{ step.message }}</span>
                      <div class="dubbing-media-step-progress">
                        <span class="dubbing-media-step-track" role="progressbar" :aria-valuenow="step.progress" aria-valuemin="0" aria-valuemax="100" :aria-label="`${step.label}进度`">
                          <span class="dubbing-media-step-bar" :style="{ width: `${step.progress}%` }" aria-hidden="true" />
                        </span>
                        <span class="dubbing-media-step-progress-value">{{ step.progress }}%</span>
                      </div>
                    </div>
                  </div>
                  <div v-else class="home-workbench-detail-file">
                    <MicVocal :stroke-width="2.1" aria-hidden="true" />
                    <span class="home-workbench-artifact-copy">
                      <span class="home-workbench-artifact-title">{{ selectedWorkbenchStage.message }}</span>
                      <span class="home-workbench-artifact-path">
                        {{ readStringValue(selectedWorkbenchStageSnapshot.dubbingTaskId) || '等待配音流程' }}
                      </span>
                    </span>
                  </div>
                </template>

                <template v-else-if="selectedWorkbenchStage.key === 'export'">
                  <div v-if="workbenchExportRows.length > 0" class="home-workbench-artifacts inline">
                    <article v-for="row in workbenchExportRows" :key="row.kind" class="home-workbench-artifact">
                      <FileCheck2 :stroke-width="2.1" aria-hidden="true" />
                      <span class="home-workbench-artifact-copy">
                        <span class="home-workbench-artifact-title">{{ row.label }}</span>
                        <span class="home-workbench-artifact-path">{{ row.path }}</span>
                      </span>
                    </article>
                  </div>
                  <div v-else class="home-workbench-detail-file">
                    <FolderOpen :stroke-width="2.1" aria-hidden="true" />
                    <span class="home-workbench-artifact-copy">
                      <span class="home-workbench-artifact-title">{{ selectedWorkbenchStage.message }}</span>
                      <span class="home-workbench-artifact-path">等待导出最终产物</span>
                    </span>
                  </div>
                </template>
              </div>

              <div v-if="workbenchSnapshot?.errorMessage" class="translate-alert home-workbench-alert" role="alert">
                <CircleAlert :stroke-width="2.1" aria-hidden="true" />
                <span>{{ workbenchSnapshot.errorMessage }}</span>
              </div>

              <div v-if="exportedArtifacts.length > 0" class="home-workbench-artifacts">
                <article v-for="artifact in exportedArtifacts" :key="artifact.kind" class="home-workbench-artifact">
                  <FileCheck2 :stroke-width="2.1" aria-hidden="true" />
                  <span class="home-workbench-artifact-copy">
                    <span class="home-workbench-artifact-title">{{ artifactLabel(artifact.kind) }}</span>
                    <span class="home-workbench-artifact-path">{{ artifact.path }}</span>
                  </span>
                  <span class="home-file-size">{{ formatFileSize(artifact.fileSize) }}</span>
                </article>
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

      <div
        v-if="activeWorkbenchParameterPanel"
        class="dialog-backdrop"
        role="presentation"
        @click.self="closeWorkbenchParameterPanel"
      >
        <section
          class="settings-dialog home-workbench-parameter-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="home-workbench-parameter-dialog-title"
        >
          <h2 id="home-workbench-parameter-dialog-title" class="dialog-title">
            {{ workbenchParameterPanelTitle }}
          </h2>

          <div class="home-workbench-parameter-body">
            <div
              v-if="activeWorkbenchParameterPanel === WorkbenchParameterPanel.Subtitle"
              class="home-workbench-dialog-rows"
              aria-label="字幕参数"
            >
              <button class="setting-row setting-row-button" type="button" :disabled="isWorkbenchRunning" @click="openWorkbenchDialog(WorkbenchDialog.TranscriptionModel)">
                <Bot class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="setting-copy">
                  <span class="setting-title">转录模型</span>
                  <span class="setting-subtitle">选择用于语音识别的模型</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ workbenchTranscriptionModelLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>

              <button class="setting-row setting-row-button" type="button" :disabled="isWorkbenchRunning" @click="openWorkbenchDialog(WorkbenchDialog.SourceLanguage)">
                <Languages class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="setting-copy">
                  <span class="setting-title">源语言</span>
                  <span class="setting-subtitle">视频语音语言</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ workbenchSourceLanguageLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>

              <button class="setting-row setting-row-button" type="button" :disabled="isWorkbenchRunning" @click="openWorkbenchDialog(WorkbenchDialog.TranscriptionFormat)">
                <Captions class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="setting-copy">
                  <span class="setting-title">输出格式</span>
                  <span class="setting-subtitle">手动导出时使用的字幕格式</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ workbenchTranscriptionFormatLabel }}</span>
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
                  :class="{ active: workbenchOptions.isSmartSegmentationEnabled }"
                  type="button"
                  :aria-pressed="workbenchOptions.isSmartSegmentationEnabled"
                  :disabled="isWorkbenchRunning"
                  @click="updateWorkbenchOptions({ isSmartSegmentationEnabled: !workbenchOptions.isSmartSegmentationEnabled })"
                >
                  <span class="setting-toggle-label">{{ workbenchOptions.isSmartSegmentationEnabled ? '开' : '关' }}</span>
                  <span class="setting-toggle-track" aria-hidden="true"><span class="setting-toggle-thumb" /></span>
                </button>
              </div>
            </div>

            <div
              v-else-if="activeWorkbenchParameterPanel === WorkbenchParameterPanel.Translation"
              class="home-workbench-dialog-rows"
              aria-label="翻译参数"
            >
              <button class="setting-row setting-row-button" type="button" :disabled="isWorkbenchRunning" @click="openWorkbenchDialog(WorkbenchDialog.VideoContentType)">
                <Film class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="setting-copy">
                  <span class="setting-title">视频类型</span>
                  <span class="setting-subtitle">选择视频内容类型</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ workbenchVideoContentTypeLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>

              <button class="setting-row setting-row-button" type="button" :disabled="isWorkbenchRunning" @click="openWorkbenchDialog(WorkbenchDialog.TargetLanguage)">
                <Languages class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="setting-copy">
                  <span class="setting-title">目标语言</span>
                  <span class="setting-subtitle">翻译字幕的目标语言</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ workbenchTargetLanguageLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>

              <button class="setting-row setting-row-button" type="button" :disabled="isWorkbenchRunning" @click="openWorkbenchDialog(WorkbenchDialog.OutputMode)">
                <PanelTop class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="setting-copy">
                  <span class="setting-title">输出模式</span>
                  <span class="setting-subtitle">选择最终字幕的呈现方式</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ workbenchOutputModeLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>

              <button class="setting-row setting-row-button" type="button" :disabled="isWorkbenchRunning" @click="openWorkbenchDialog(WorkbenchDialog.TranslationFormat)">
                <Captions class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="setting-copy">
                  <span class="setting-title">输出格式</span>
                  <span class="setting-subtitle">处理后导出的字幕格式</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ workbenchTranslationFormatLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>
            </div>

            <div
              v-else-if="activeWorkbenchParameterPanel === WorkbenchParameterPanel.Dubbing"
              class="home-workbench-dialog-rows"
              aria-label="配音参数"
            >
              <div class="setting-row">
                <Timer class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <div class="setting-copy">
                  <div class="setting-title">TTS 间隔</div>
                  <div class="setting-subtitle">分段语音停顿时长</div>
                </div>
                <div class="setting-range-control home-workbench-dialog-range tts">
                  <span class="setting-range-value">{{ workbenchOptions.dubbingTtsIntervalMs }} 毫秒</span>
                  <input
                    class="setting-range"
                    type="range"
                    min="0"
                    max="1000"
                    step="10"
                    :value="workbenchOptions.dubbingTtsIntervalMs"
                    :disabled="isWorkbenchRunning"
                    aria-label="工作台 TTS 间隔"
                    @change="updateWorkbenchTtsInterval"
                  />
                </div>
              </div>

              <button class="setting-row setting-row-button" type="button" :disabled="isWorkbenchRunning" @click="openWorkbenchDialog(WorkbenchDialog.ReferenceAudio)">
                <FileMusic class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="setting-copy">
                  <span class="setting-title">参考音频</span>
                  <span class="setting-subtitle">选择参考音频来源</span>
                </span>
                <span class="setting-inline-action">
                  <span class="setting-value">{{ workbenchReferenceAudioLabel }}</span>
                  <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                </span>
              </button>

              <div class="setting-row">
                <Music class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <div class="setting-copy">
                  <div class="setting-title">背景音乐</div>
                  <div class="setting-subtitle">开启后分离源视频伴奏并跟随变速同步混入最终视频</div>
                </div>
                <button class="setting-toggle" :class="{ active: workbenchOptions.dubbingIsBackgroundMusicEnabled }" type="button" :aria-pressed="workbenchOptions.dubbingIsBackgroundMusicEnabled" :disabled="isWorkbenchRunning" @click="updateWorkbenchOptions({ dubbingIsBackgroundMusicEnabled: !workbenchOptions.dubbingIsBackgroundMusicEnabled })">
                  <span class="setting-toggle-label">{{ workbenchOptions.dubbingIsBackgroundMusicEnabled ? '开' : '关' }}</span>
                  <span class="setting-toggle-track" aria-hidden="true"><span class="setting-toggle-thumb" /></span>
                </button>
              </div>

              <div class="setting-row">
                <Volume2 class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <div class="setting-copy">
                  <div class="setting-title">背景音乐音量</div>
                  <div class="setting-subtitle">背景音乐混入音量</div>
                </div>
                <div class="setting-range-control home-workbench-dialog-range">
                  <span class="setting-range-value">{{ workbenchOptions.dubbingBackgroundMusicVolume.toFixed(1) }}</span>
                  <input
                    class="setting-range"
                    type="range"
                    min="0"
                    max="1"
                    step="0.1"
                    :value="workbenchOptions.dubbingBackgroundMusicVolume"
                    :disabled="isWorkbenchRunning || !workbenchOptions.dubbingIsBackgroundMusicEnabled"
                    aria-label="工作台背景音乐音量"
                    @change="updateWorkbenchBackgroundMusicVolume"
                  />
                </div>
              </div>
            </div>

            <div v-else class="home-workbench-dialog-rows" aria-label="导出参数">
              <div class="setting-row">
                <FolderOpen class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
                <div class="setting-copy">
                  <div class="setting-title">导出目录</div>
                  <div class="setting-subtitle">{{ workbenchExportDirLabel }}</div>
                </div>
                <button class="settings-action" type="button" :disabled="isWorkbenchRunning" @click="selectWorkbenchExportDir">
                  选择目录
                </button>
              </div>
            </div>
          </div>

          <div class="youtube-dialog-actions">
            <button class="settings-action youtube-monitor-action primary" type="button" @click="closeWorkbenchParameterPanel">
              完成
            </button>
          </div>
        </section>
      </div>

      <div v-if="activeWorkbenchDialog" class="dialog-backdrop" role="presentation" @click.self="closeWorkbenchDialog">
        <section
          class="settings-dialog"
          :class="{ 'language-dialog': isWorkbenchLanguageDialog }"
          role="dialog"
          aria-modal="true"
          :aria-labelledby="`${activeWorkbenchDialog}-dialog-title`"
        >
          <h2 :id="`${activeWorkbenchDialog}-dialog-title`" class="dialog-title">{{ workbenchDialogTitle }}</h2>

          <label v-if="isWorkbenchLanguageDialog" class="language-search-field">
            <Search class="language-search-icon" :stroke-width="2.1" aria-hidden="true" />
            <input
              v-model="workbenchDialogSearch"
              class="settings-input language-search-input"
              type="search"
              placeholder="搜索语言"
              aria-label="搜索语言"
            />
          </label>

          <div class="dialog-options" role="radiogroup" :aria-label="workbenchDialogTitle">
            <button
              v-for="option in filteredWorkbenchDialogOptions"
              :key="option.value"
              class="dialog-option"
              :class="{ active: workbenchDialogValue === option.value }"
              type="button"
              role="radio"
              :aria-checked="workbenchDialogValue === option.value"
              @click="selectWorkbenchDialogOption(option.value)"
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
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import {
  ArrowLeft,
  BadgeInfo,
  Bot,
  Captions,
  CheckCircle2,
  ChevronRight,
  CircleAlert,
  Clock,
  Download,
  FileMusic,
  FileCheck2,
  Film,
  FolderOpen,
  Link2,
  ListTodo,
  ListVideo,
  LoaderCircle,
  Languages,
  MicVocal,
  Music,
  PanelTop,
  Play,
  Plus,
  RefreshCw,
  Scissors,
  Search,
  Timer,
  Trash2,
  Video,
  Volume2,
  WandSparkles,
  Workflow,
} from 'lucide-vue-next'
import { computed, nextTick, onActivated, onBeforeUnmount, onMounted, ref, watch, type Ref } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import {
  getLanguageLabel,
  getOptionLabel,
  outputModeOptions,
  OutputMode,
  referenceAudioSourceOptions,
  ReferenceAudioSource,
  readOptionValue,
  sourceLanguageOptions,
  SubtitleFormat,
  subtitleFormatOptions,
  targetLanguageOptions,
  transcriptionModelOptions,
  TranscriptionModel,
  translationServiceOptions,
  TranslationService,
  VideoContentType,
  videoContentTypeOptions,
} from '../settingsModel'

defineOptions({ name: 'Home' })

type TaskStatusFilter = 'all' | 'pending' | 'ready' | 'failed'
type DownloadProgressStatus = 'active' | 'done' | 'failed'
type DownloadProgressKind = 'video' | 'subtitle'
type WorkbenchStatus = 'idle' | 'running' | 'done' | 'failed'
type WorkbenchStageStatus = 'pending' | 'active' | 'done' | 'skipped' | 'failed'
type WorkbenchSubtitleSource = 'transcribe' | 'downloaded'
type WorkbenchDetailStageStatus = WorkbenchStageStatus | 'interrupted'

const VIDEO_DESCRIPTION_FALLBACK_HEIGHT = 276
const HOME_DETAIL_NARROW_QUERY = '(max-width: 860px)'

enum WorkbenchDialog {
  SubtitleSource = 'workbench-subtitle-source',
  TranscriptionModel = 'workbench-transcription-model',
  SourceLanguage = 'workbench-source-language',
  TranscriptionFormat = 'workbench-transcription-format',
  TranslationService = 'workbench-translation-service',
  VideoContentType = 'workbench-video-content-type',
  TargetLanguage = 'workbench-target-language',
  OutputMode = 'workbench-output-mode',
  TranslationFormat = 'workbench-translation-format',
  ReferenceAudio = 'workbench-reference-audio',
}

enum WorkbenchParameterPanel {
  Subtitle = 'subtitle',
  Translation = 'translation',
  Dubbing = 'dubbing',
  Export = 'export',
}

type DialogOption = {
  value: string
  label: string
}

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

type HomeVideoDownload = {
  id: string
  taskId: string
  format: string
  filePath: string
  fileName: string
  fileSize: number
  createdAt: string
  updatedAt: string
}

type HomePartialVideoDownload = {
  fileCount: number
  fileSize: number
  updatedAt?: string | null
}

type HomeVideoDownloadProgress = {
  taskId: string
  kind: DownloadProgressKind
  key: string
  progress: number
  status: DownloadProgressStatus
  message: string
  downloadedBytes?: number | null
  totalBytes?: number | null
  language?: string | null
  sourceKind?: string | null
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
  downloadedVideo?: HomeVideoDownload | null
  partialVideoDownload?: HomePartialVideoDownload | null
}

type HomeWorkbenchOptions = {
  subtitleSource: WorkbenchSubtitleSource
  subtitleId: string
  translationEnabled: boolean
  dubbingEnabled: boolean
  exportDir: string
  transcriptionModel: TranscriptionModel
  sourceLanguage: string
  transcriptionFormat: SubtitleFormat
  isSmartSegmentationEnabled: boolean
  isSubtitleCorrectionEnabled: boolean
  translationFormat: SubtitleFormat
  translationService: TranslationService
  needsReflectionTranslation: boolean
  translationBatchSize: number
  translationThreadCount: number
  videoContentType: VideoContentType
  outputMode: OutputMode
  isSubtitleTranslationEnabled: boolean
  isPostTranslationOptimizationEnabled: boolean
  targetLanguage: string
  dubbingTtsIntervalMs: number
  dubbingReferenceAudioSource: ReferenceAudioSource
  dubbingCustomReferenceAudioPath: string
  dubbingIsBackgroundMusicEnabled: boolean
  dubbingBackgroundMusicVolume: number
}

type HomeWorkbenchStage = {
  key: string
  label: string
  progress: number
  status: WorkbenchStageStatus
  message: string
  snapshot?: Record<string, unknown>
}

type HomeWorkbenchArtifact = {
  kind: string
  path: string
  fileSize: number
  metadata: Record<string, unknown>
  createdAt: string
  updatedAt: string
}

type HomeWorkbenchSnapshot = {
  taskId: string
  status: WorkbenchStatus
  currentStage: string
  progress: number
  message: string
  stages: HomeWorkbenchStage[]
  options: HomeWorkbenchOptions
  artifacts: HomeWorkbenchArtifact[]
  warnings: string[]
  errorMessage: string
  revision: number
  createdAt: string
  updatedAt: string
}

type WorkbenchDetailStep = {
  key: string
  label: string
  progress: number
  status: WorkbenchDetailStageStatus
  message: string
}

type WorkbenchSubtitleSegment = {
  uid?: string
  text: string
  startTime: number
  endTime: number
  status?: string
}

type WorkbenchTranslationRow = {
  key: string
  index: number
  startTime: number
  endTime: number
  sourceText: string
  targetText: string
  status: string
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
const isAddingWorkbenchVideo = ref(false)
const downloadingVideoTaskIds = ref(new Set<string>())
const isAddDialogOpen = ref(false)
const isDeleteDialogOpen = ref(false)
const pageError = ref('')
const addError = ref('')
const deleteError = ref('')
const subtitleErrorsByTaskId = ref(new Map<string, string>())
const videoErrorsByTaskId = ref(new Map<string, string>())
const downloadingSubtitleKeys = ref(new Set<string>())
const addingWorkbenchSubtitleIds = ref(new Set<string>())
const downloadProgressByKey = ref(new Map<string, HomeVideoDownloadProgress>())
const videoSideRef = ref<HTMLElement | null>(null)
const videoCopyRef = ref<HTMLElement | null>(null)
const videoTitleLineRef = ref<HTMLElement | null>(null)
const videoUrlRef = ref<HTMLElement | null>(null)
const videoDescriptionMaxHeight = ref(VIDEO_DESCRIPTION_FALLBACK_HEIGHT)
const autoRefreshedTaskIds = ref(new Set<string>())
const taskPendingDelete = ref<HomeVideoTask | null>(null)
const workbenchSnapshot = ref<HomeWorkbenchSnapshot | null>(null)
const workbenchOptions = ref<HomeWorkbenchOptions>({
  subtitleSource: 'transcribe',
  subtitleId: '',
  translationEnabled: true,
  dubbingEnabled: false,
  exportDir: '',
  transcriptionModel: TranscriptionModel.Bilibili,
  sourceLanguage: 'auto',
  transcriptionFormat: SubtitleFormat.Srt,
  isSmartSegmentationEnabled: true,
  isSubtitleCorrectionEnabled: true,
  translationFormat: SubtitleFormat.Srt,
  translationService: TranslationService.Llm,
  needsReflectionTranslation: true,
  translationBatchSize: 30,
  translationThreadCount: 10,
  videoContentType: VideoContentType.General,
  outputMode: OutputMode.Bilingual,
  isSubtitleTranslationEnabled: true,
  isPostTranslationOptimizationEnabled: true,
  targetLanguage: 'zh-Hans',
  dubbingTtsIntervalMs: 150,
  dubbingReferenceAudioSource: ReferenceAudioSource.ExistingDubbing,
  dubbingCustomReferenceAudioPath: '',
  dubbingIsBackgroundMusicEnabled: true,
  dubbingBackgroundMusicVolume: 0.5,
})
const isWorkbenchLoading = ref(false)
const isWorkbenchStarting = ref(false)
const activeWorkbenchDialog = ref<WorkbenchDialog | null>(null)
const activeWorkbenchParameterPanel = ref<WorkbenchParameterPanel | null>(null)
const selectedWorkbenchStageKey = ref('')
const isWorkbenchStageSelectionPinned = ref(false)
const workbenchDialogSearch = ref('')
let unlistenHomeDownloadProgress: UnlistenFn | undefined
let unlistenHomeWorkbenchProgress: UnlistenFn | undefined
let videoLayoutObserver: ResizeObserver | undefined
let videoLayoutFrame = 0
let hasCompletedInitialLoad = false

const taskFilterOptions: { value: TaskStatusFilter; label: string }[] = [
  { value: 'all', label: '全部' },
  { value: 'pending', label: '待读取' },
  { value: 'ready', label: '已就绪' },
  { value: 'failed', label: '异常' },
]

const workbenchTranscribeOption: DialogOption = { value: 'transcribe', label: '自动转录' }
const workbenchDownloadedSubtitleOption: DialogOption = { value: 'downloaded', label: '使用最新已下载字幕' }

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

const videoDownloadMeta = computed(() => {
  const video = activeTask.value?.downloadedVideo
  if (!video) {
    const partial = partialVideoDownload.value
    if (partial) {
      const pieces = [
        '未完成',
        `已保留 ${formatFileSize(partial.fileSize)}`,
        partial.updatedAt ? formatDateTime(partial.updatedAt) : '',
      ].filter(Boolean)
      return pieces.join(' · ')
    }

    return '尚未下载'
  }

  const pieces = [
    video.format ? video.format.toUpperCase() : '视频',
    formatFileSize(video.fileSize),
    video.updatedAt ? formatDateTime(video.updatedAt) : '',
  ].filter(Boolean)
  return pieces.join(' · ')
})

const partialVideoDownload = computed(() => {
  if (activeTask.value?.downloadedVideo) {
    return null
  }

  return activeTask.value?.partialVideoDownload ?? null
})

const hasPartialVideoDownload = computed(() => Boolean(partialVideoDownload.value))

const isActiveTaskDownloadingVideo = computed(() => {
  const taskId = activeTask.value?.id
  return Boolean(taskId && isVideoTaskDownloading(taskId))
})

const videoActionLabel = computed(() => {
  if (isActiveTaskDownloadingVideo.value) {
    return '下载中'
  }
  if (hasPartialVideoDownload.value) {
    return '继续下载'
  }
  if (activeTask.value?.downloadedVideo) {
    return '重新下载'
  }
  return '下载视频'
})

const videoDownloadProgress = computed(() => {
  if (!activeTask.value) {
    return null
  }
  return downloadProgressByKey.value.get(downloadProgressKey(activeTask.value.id, 'video', 'video')) ?? null
})

const videoDownloadProgressValue = computed(() => {
  return clampProgress(videoDownloadProgress.value?.progress ?? (isActiveTaskDownloadingVideo.value ? 2 : 0))
})

const videoDownloadProgressMessage = computed(() => {
  return videoDownloadProgress.value?.message || (isActiveTaskDownloadingVideo.value ? '视频下载中' : '')
})

const subtitleError = computed(() => activeTaskScopedError(subtitleErrorsByTaskId))
const videoError = computed(() => activeTaskScopedError(videoErrorsByTaskId))

const videoDescriptionStyle = computed(() => ({
  maxHeight: `${videoDescriptionMaxHeight.value}px`,
}))

const deleteTargetLabel = computed(() => {
  const task = taskPendingDelete.value
  if (!task) {
    return '该待办任务'
  }

  return task.title || task.webpageUrl || task.url
})

const isWorkbenchRunning = computed(() => isWorkbenchStarting.value || workbenchSnapshot.value?.status === 'running')

const workbenchStages = computed(() => {
  return workbenchSnapshot.value?.stages?.length ? workbenchSnapshot.value.stages : createDefaultWorkbenchStages()
})

const selectedWorkbenchStage = computed(() => {
  return (
    workbenchStages.value.find((stage) => stage.key === selectedWorkbenchStageKey.value) ??
    preferredWorkbenchStage.value
  )
})

const selectedWorkbenchStageSnapshot = computed(() => {
  return selectedWorkbenchStage.value?.snapshot ?? {}
})

const preferredWorkbenchStage = computed(() => {
  return (
    workbenchStages.value.find((stage) => stage.status === 'active') ??
    workbenchStages.value.find((stage) => stage.status === 'failed') ??
    workbenchStages.value.find((stage) => stage.status === 'pending') ??
    workbenchStages.value[0] ??
    null
  )
})

const workbenchMainMessage = computed(() => {
  if (isWorkbenchLoading.value) {
    return '正在读取工作台'
  }
  return workbenchSnapshot.value?.message || '等待开始'
})

const workbenchStatusText = computed(() => {
  if (isWorkbenchRunning.value) {
    return '工作台执行中'
  }
  if (workbenchSnapshot.value?.errorMessage) {
    return workbenchSnapshot.value.errorMessage
  }
  if (workbenchSnapshot.value?.status === 'done') {
    return '工作台已完成'
  }
  return '等待执行'
})

const workbenchMessageClass = computed(() => ({
  ready: workbenchSnapshot.value?.status === 'done' && !workbenchSnapshot.value?.errorMessage,
  error: Boolean(workbenchSnapshot.value?.errorMessage),
}))

const workbenchStatusDotClass = computed(() => {
  if (workbenchSnapshot.value?.errorMessage || workbenchSnapshot.value?.status === 'failed') {
    return 'error'
  }
  if (isWorkbenchRunning.value) {
    return 'active'
  }
  if (workbenchSnapshot.value?.status === 'done') {
    return 'success'
  }
  return 'warning'
})

const canStartWorkbench = computed(() => {
  return Boolean(activeTask.value && ytdlpStatus.value.isAvailable && !isWorkbenchRunning.value && !isWorkbenchLoading.value)
})

const workbenchTranscriptionModelLabel = computed(() =>
  getOptionLabel(transcriptionModelOptions, workbenchOptions.value.transcriptionModel),
)
const workbenchSourceLanguageLabel = computed(() => getLanguageLabel(workbenchOptions.value.sourceLanguage))
const workbenchTranscriptionFormatLabel = computed(() =>
  getOptionLabel(subtitleFormatOptions, workbenchOptions.value.transcriptionFormat),
)
const workbenchVideoContentTypeLabel = computed(() =>
  getOptionLabel(videoContentTypeOptions, workbenchOptions.value.videoContentType),
)
const workbenchTargetLanguageLabel = computed(() => getLanguageLabel(workbenchOptions.value.targetLanguage))
const workbenchOutputModeLabel = computed(() => getOptionLabel(outputModeOptions, workbenchOptions.value.outputMode))
const workbenchTranslationFormatLabel = computed(() =>
  getOptionLabel(subtitleFormatOptions, workbenchOptions.value.translationFormat),
)
const workbenchReferenceAudioLabel = computed(() =>
  getOptionLabel(referenceAudioSourceOptions, workbenchOptions.value.dubbingReferenceAudioSource),
)
const workbenchExportDirLabel = computed(() => workbenchOptions.value.exportDir || '使用设置中的默认导出目录')

const exportedArtifacts = computed(() => {
  return (workbenchSnapshot.value?.artifacts ?? []).filter((artifact) =>
    ['exported-video', 'exported-subtitle'].includes(artifact.kind),
  )
})

const sourceVideoArtifact = computed(() => workbenchArtifactByKind('source-video'))

const selectedSubtitleArtifact = computed(() => workbenchArtifactByKind('selected-subtitle'))

const isVideoAddedToWorkbench = computed(() => Boolean(sourceVideoArtifact.value))

const registeredSubtitleIds = computed(() => {
  const subtitleId = readStringValue(selectedSubtitleArtifact.value?.metadata.subtitleId)
  return subtitleId ? new Set([subtitleId]) : new Set<string>()
})

const workbenchRunLabel = computed(() => {
  if (isWorkbenchRunning.value) {
    return '执行中'
  }
  if (workbenchSnapshot.value?.status === 'failed') {
    return '继续执行'
  }
  if (workbenchSnapshot.value?.stages.some((stage) => ['done', 'skipped'].includes(stage.status))) {
    return '继续执行'
  }
  return '开始执行'
})

const workbenchPrepareSubtitleSteps = computed<WorkbenchDetailStep[]>(() => {
  const snapshot = selectedWorkbenchStageSnapshot.value
  const mode = readStringValue(snapshot.mode)
  if (mode === 'downloaded') {
    return [
      {
        key: 'downloaded-subtitle',
        label: '下载字幕',
        progress: 100,
        status: 'done',
        message: '已添加到工作台',
      },
    ]
  }
  const stageProgress = readRecordValue(snapshot.stageProgress)
  return [
    { key: 'transcription', label: '转录', source: stageProgress.transcription },
    { key: 'smartSegmentation', label: '智能断句', source: stageProgress.smartSegmentation },
    { key: 'subtitleCorrection', label: '字幕校正', source: stageProgress.subtitleCorrection },
  ]
    .filter((item) => readRecordValue(item.source))
    .map((item) => detailStepFromRecord(item.key, item.label, readRecordValue(item.source)))
})

const workbenchSubtitleSegments = computed<WorkbenchSubtitleSegment[]>(() => {
  return readSubtitleSegments(selectedWorkbenchStageSnapshot.value.segments)
})

const workbenchTranslationSteps = computed<WorkbenchDetailStep[]>(() => {
  const stageProgress = readRecordValue(selectedWorkbenchStageSnapshot.value.stageProgress)
  return [
    { key: 'subtitleTranslation', label: '字幕翻译', source: stageProgress.subtitleTranslation },
    {
      key: 'postTranslationOptimization',
      label: '译后优化',
      source: stageProgress.postTranslationOptimization,
    },
  ]
    .filter((item) => readRecordValue(item.source))
    .map((item) => detailStepFromRecord(item.key, item.label, readRecordValue(item.source)))
})

const workbenchTranslationRows = computed<WorkbenchTranslationRow[]>(() => {
  const sourceSegments = readSubtitleSegments(selectedWorkbenchStageSnapshot.value.sourceSegments)
  const translatedSegments = readSubtitleSegments(selectedWorkbenchStageSnapshot.value.translatedSegments)
  const total = Math.max(sourceSegments.length, translatedSegments.length)
  return Array.from({ length: total }, (_, index) => {
    const source = sourceSegments[index]
    const translated = translatedSegments[index]
    return {
      key: translated?.uid || source?.uid || `workbench-translation-${index}`,
      index: index + 1,
      startTime: source?.startTime ?? translated?.startTime ?? 0,
      endTime: source?.endTime ?? translated?.endTime ?? 0,
      sourceText: source?.text ?? '',
      targetText: translated?.text ?? '',
      status: translated?.status || source?.status || 'raw',
    }
  })
})

const workbenchDubbingSnapshot = computed(() => {
  return readRecordValue(selectedWorkbenchStageSnapshot.value.dubbingSnapshot)
})

const workbenchDubbingSteps = computed<WorkbenchDetailStep[]>(() => {
  const stages = readRecordValue(workbenchDubbingSnapshot.value.stages)
  return [
    { key: 'material', label: '素材准备', source: stages.material },
    { key: 'subtitlePreprocess', label: '字幕预处理', source: stages.subtitlePreprocess },
    { key: 'mediaSeparation', label: '音视频分离', source: stages.mediaSeparation },
    { key: 'referenceAudio', label: '参考音频生成', source: stages.referenceAudio },
    { key: 'ttsSynthesis', label: 'TTS 配音', source: stages.ttsSynthesis },
    { key: 'audioVideoAlignment', label: '音视频对齐', source: stages.audioVideoAlignment },
    { key: 'videoCompose', label: '视频合成', source: stages.videoCompose },
  ]
    .filter((item) => readRecordValue(item.source))
    .map((item) => detailStepFromRecord(item.key, item.label, readRecordValue(item.source)))
})

const workbenchExportRows = computed(() => {
  const snapshot = selectedWorkbenchStageSnapshot.value
  return [
    { kind: 'video', label: '视频文件', path: readStringValue(snapshot.videoPath) },
    { kind: 'subtitle', label: '字幕文件', path: readStringValue(snapshot.subtitlePath) },
  ].filter((row) => row.path)
})

const isWorkbenchLanguageDialog = computed(() => {
  return (
    activeWorkbenchDialog.value === WorkbenchDialog.SourceLanguage ||
    activeWorkbenchDialog.value === WorkbenchDialog.TargetLanguage
  )
})

const workbenchParameterPanelTitle = computed(() => {
  switch (activeWorkbenchParameterPanel.value) {
    case WorkbenchParameterPanel.Subtitle:
      return '字幕参数'
    case WorkbenchParameterPanel.Translation:
      return '翻译参数'
    case WorkbenchParameterPanel.Dubbing:
      return '配音参数'
    case WorkbenchParameterPanel.Export:
      return '导出参数'
    default:
      return ''
  }
})

const workbenchDialogTitle = computed(() => {
  switch (activeWorkbenchDialog.value) {
    case WorkbenchDialog.SubtitleSource:
      return '字幕来源'
    case WorkbenchDialog.TranscriptionModel:
      return '转录模型'
    case WorkbenchDialog.SourceLanguage:
      return '源语言'
    case WorkbenchDialog.TranscriptionFormat:
      return '转录格式'
    case WorkbenchDialog.TranslationService:
      return '翻译服务'
    case WorkbenchDialog.VideoContentType:
      return '视频类型'
    case WorkbenchDialog.TargetLanguage:
      return '目标语言'
    case WorkbenchDialog.OutputMode:
      return '输出模式'
    case WorkbenchDialog.TranslationFormat:
      return '字幕格式'
    case WorkbenchDialog.ReferenceAudio:
      return '参考音频'
    default:
      return ''
  }
})

const workbenchDialogOptions = computed<DialogOption[]>(() => {
  switch (activeWorkbenchDialog.value) {
    case WorkbenchDialog.SubtitleSource:
      return workbenchSubtitleSourceOptions.value
    case WorkbenchDialog.TranscriptionModel:
      return [...transcriptionModelOptions]
    case WorkbenchDialog.SourceLanguage:
      return [...sourceLanguageOptions]
    case WorkbenchDialog.TranscriptionFormat:
      return [...subtitleFormatOptions]
    case WorkbenchDialog.TranslationService:
      return [...translationServiceOptions]
    case WorkbenchDialog.VideoContentType:
      return [...videoContentTypeOptions]
    case WorkbenchDialog.TargetLanguage:
      return [...targetLanguageOptions]
    case WorkbenchDialog.OutputMode:
      return [...outputModeOptions]
    case WorkbenchDialog.TranslationFormat:
      return [...subtitleFormatOptions]
    case WorkbenchDialog.ReferenceAudio:
      return [...referenceAudioSourceOptions]
    default:
      return []
  }
})

const workbenchSubtitleSourceOptions = computed<DialogOption[]>(() => {
  const subtitles = activeTask.value?.downloadedSubtitles ?? []
  const subtitleOptions = subtitles.map((subtitle) => ({
    value: `downloaded:${subtitle.id}`,
    label: subtitleSubtitleLabel(subtitle),
  }))
  return subtitles.length > 0
    ? [workbenchTranscribeOption, workbenchDownloadedSubtitleOption, ...subtitleOptions]
    : [workbenchTranscribeOption]
})

const filteredWorkbenchDialogOptions = computed<DialogOption[]>(() => {
  const query = workbenchDialogSearch.value.trim().toLowerCase()
  const options = workbenchDialogOptions.value
  if (!isWorkbenchLanguageDialog.value || !query) {
    return options
  }
  return options.filter((option) => option.label.toLowerCase().includes(query) || option.value.toLowerCase().includes(query))
})

const workbenchDialogValue = computed(() => {
  switch (activeWorkbenchDialog.value) {
    case WorkbenchDialog.SubtitleSource:
      return workbenchOptions.value.subtitleSource === 'downloaded' && workbenchOptions.value.subtitleId
        ? `downloaded:${workbenchOptions.value.subtitleId}`
        : workbenchOptions.value.subtitleSource
    case WorkbenchDialog.TranscriptionModel:
      return workbenchOptions.value.transcriptionModel
    case WorkbenchDialog.SourceLanguage:
      return workbenchOptions.value.sourceLanguage
    case WorkbenchDialog.TranscriptionFormat:
      return workbenchOptions.value.transcriptionFormat
    case WorkbenchDialog.TranslationService:
      return workbenchOptions.value.translationService
    case WorkbenchDialog.VideoContentType:
      return workbenchOptions.value.videoContentType
    case WorkbenchDialog.TargetLanguage:
      return workbenchOptions.value.targetLanguage
    case WorkbenchDialog.OutputMode:
      return workbenchOptions.value.outputMode
    case WorkbenchDialog.TranslationFormat:
      return workbenchOptions.value.translationFormat
    case WorkbenchDialog.ReferenceAudio:
      return workbenchOptions.value.dubbingReferenceAudioSource
    default:
      return ''
  }
})

watch(activeTaskId, async () => {
  selectedWorkbenchStageKey.value = ''
  isWorkbenchStageSelectionPinned.value = false
  if (!activeTaskId.value) {
    workbenchSnapshot.value = null
    return
  }
  await ensureActiveTaskLoaded()
  await loadWorkbenchSnapshot()
  await nextTick()
  refreshVideoDescriptionObserver()
  scheduleVideoDescriptionMeasure()
  maybeAutoRefreshActiveTask()
})

watch(activeTask, async () => {
  await nextTick()
  refreshVideoDescriptionObserver()
  scheduleVideoDescriptionMeasure()
})

watch(workbenchStages, () => {
  syncSelectedWorkbenchStage()
})

onMounted(async () => {
  window.addEventListener('keydown', handleKeydown)
  window.addEventListener('resize', scheduleVideoDescriptionMeasure)
  void registerHomeDownloadProgressListener()
  void registerHomeWorkbenchProgressListener()
  try {
    await refreshHomeView()
  } finally {
    hasCompletedInitialLoad = true
  }
})

onActivated(() => {
  if (hasCompletedInitialLoad) {
    void refreshHomeView()
  }
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown)
  window.removeEventListener('resize', scheduleVideoDescriptionMeasure)
  unlistenHomeDownloadProgress?.()
  unlistenHomeWorkbenchProgress?.()
  videoLayoutObserver?.disconnect()
  if (videoLayoutFrame) {
    window.cancelAnimationFrame(videoLayoutFrame)
  }
})

const loadAll = async () => {
  await Promise.all([loadYtdlpStatus(), loadTasks()])
}

const refreshHomeView = async () => {
  await loadAll()
  if (activeTaskId.value) {
    await loadWorkbenchSnapshot()
  }
  await nextTick()
  refreshVideoDescriptionObserver()
  scheduleVideoDescriptionMeasure()
  maybeAutoRefreshActiveTask()
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
    const loadedTasks = await invoke<HomeVideoTask[]>('list_home_video_tasks')
    tasks.value = sortTasksByCreatedAtDesc(loadedTasks)
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

const loadWorkbenchSnapshot = async () => {
  if (!activeTaskId.value || !isTauriRuntime()) {
    return
  }

  isWorkbenchLoading.value = true
  try {
    const snapshot = await invoke<HomeWorkbenchSnapshot>('get_home_workbench', {
      request: { taskId: activeTaskId.value },
    })
    applyWorkbenchSnapshot(snapshot)
  } catch (error) {
    pageError.value = stringifyError(error, '读取工作台失败')
  } finally {
    isWorkbenchLoading.value = false
  }
}

const saveWorkbenchOptions = async () => {
  const taskId = activeTaskId.value
  if (!taskId || !isTauriRuntime() || isWorkbenchRunning.value) {
    return
  }

  try {
    const snapshot = await invoke<HomeWorkbenchSnapshot>('save_home_workbench_options', {
      request: {
        taskId,
        options: workbenchOptions.value,
      },
    })
    applyWorkbenchSnapshot(snapshot)
  } catch (error) {
    pageError.value = stringifyError(error, '保存工作台参数失败')
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

  await reloadTask(activeTaskId.value)
}

const reloadTask = async (taskId: string) => {
  if (!taskId || !isTauriRuntime()) {
    return
  }

  try {
    const task = await invoke<HomeVideoTask>('get_home_video_task', {
      request: { taskId },
    })
    upsertTask(task)
    if (taskId === activeTaskId.value) {
      await loadWorkbenchSnapshot()
    }
  } catch {
    // 页面顶部已经展示了主错误，这里不覆盖上下文。
  }
}

const registerHomeDownloadProgressListener = async () => {
  if (!isTauriRuntime() || unlistenHomeDownloadProgress) {
    return
  }

  unlistenHomeDownloadProgress = await listen<HomeVideoDownloadProgress>('home-video-download-progress', (event) => {
    const payload = event.payload
    if (!payload.taskId || !payload.key || !['video', 'subtitle'].includes(payload.kind)) {
      return
    }

    setDownloadProgress(payload)
  })
}

const registerHomeWorkbenchProgressListener = async () => {
  if (!isTauriRuntime() || unlistenHomeWorkbenchProgress) {
    return
  }

  unlistenHomeWorkbenchProgress = await listen<HomeWorkbenchSnapshot>('home-workbench-progress', (event) => {
    const payload = event.payload
    if (!payload.taskId || payload.taskId !== activeTaskId.value) {
      return
    }
    applyWorkbenchSnapshot(payload)
  })
}

const setDownloadProgress = (payload: HomeVideoDownloadProgress, options: { reset?: boolean } = {}) => {
  const next = new Map(downloadProgressByKey.value)
  const key = downloadProgressKey(payload.taskId, payload.kind, payload.key)
  const previous = options.reset ? null : next.get(key)
  const incomingProgress = clampProgress(payload.progress)
  const shouldKeepPreviousProgress =
    payload.status === 'active' && previous?.status === 'active' && previous.progress > incomingProgress

  next.set(key, {
    ...payload,
    progress: shouldKeepPreviousProgress ? previous.progress : incomingProgress,
  })
  downloadProgressByKey.value = next
}

const refreshVideoDescriptionObserver = () => {
  videoLayoutObserver?.disconnect()
  videoLayoutObserver = undefined

  if (typeof ResizeObserver === 'undefined') {
    return
  }

  const targets = [videoSideRef.value, videoCopyRef.value, videoTitleLineRef.value, videoUrlRef.value].filter(
    (element): element is HTMLElement => Boolean(element),
  )
  if (targets.length === 0) {
    return
  }

  videoLayoutObserver = new ResizeObserver(scheduleVideoDescriptionMeasure)
  targets.forEach((element) => videoLayoutObserver?.observe(element))
}

const scheduleVideoDescriptionMeasure = () => {
  if (videoLayoutFrame) {
    window.cancelAnimationFrame(videoLayoutFrame)
  }

  videoLayoutFrame = window.requestAnimationFrame(() => {
    videoLayoutFrame = 0
    measureVideoDescriptionHeight()
  })
}

const measureVideoDescriptionHeight = () => {
  if (!activeTask.value || isHomeDetailNarrow()) {
    videoDescriptionMaxHeight.value = VIDEO_DESCRIPTION_FALLBACK_HEIGHT
    return
  }

  const sideHeight = videoSideRef.value?.offsetHeight ?? 0
  const titleHeight = videoTitleLineRef.value?.offsetHeight ?? 0
  const urlHeight = videoUrlRef.value?.offsetHeight ?? 0
  if (!sideHeight || !titleHeight || !urlHeight) {
    videoDescriptionMaxHeight.value = VIDEO_DESCRIPTION_FALLBACK_HEIGHT
    return
  }

  const gap = readRowGap(videoCopyRef.value, 10)
  const availableHeight = Math.floor(sideHeight - titleHeight - urlHeight - gap * 2)
  videoDescriptionMaxHeight.value = Math.max(96, availableHeight)
}

const isHomeDetailNarrow = () => {
  return window.matchMedia(HOME_DETAIL_NARROW_QUERY).matches
}

const readRowGap = (element: HTMLElement | null, fallback: number) => {
  if (!element) {
    return fallback
  }

  const value = Number.parseFloat(window.getComputedStyle(element).rowGap)
  return Number.isFinite(value) ? value : fallback
}

const downloadSubtitle = async (option: HomeVideoSubtitleOption) => {
  const task = activeTask.value
  if (!task || !isTauriRuntime()) {
    return
  }

  const key = subtitleKey(option)
  const scopedKey = downloadProgressKey(task.id, 'subtitle', key)
  if (downloadingSubtitleKeys.value.has(scopedKey)) {
    return
  }

  const next = new Set(downloadingSubtitleKeys.value)
  next.add(scopedKey)
  downloadingSubtitleKeys.value = next
  setDownloadProgress({
    taskId: task.id,
    kind: 'subtitle',
    key,
    progress: 2,
    status: 'active',
    message: '准备下载字幕',
    language: option.language,
    sourceKind: option.sourceKind,
  }, { reset: true })
  clearTaskError(subtitleErrorsByTaskId, task.id)

  try {
    const updatedTask = await invoke<HomeVideoTask>('download_home_video_task_subtitle', {
      request: {
        taskId: task.id,
        language: option.language,
        sourceKind: option.sourceKind,
      },
    })
    upsertTask(updatedTask)
  } catch (error) {
    const message = stringifyError(error, '下载字幕失败')
    setTaskError(subtitleErrorsByTaskId, task.id, message)
    setDownloadProgress({
      taskId: task.id,
      kind: 'subtitle',
      key,
      progress: 100,
      status: 'failed',
      message,
      language: option.language,
      sourceKind: option.sourceKind,
    })
  } finally {
    const cleared = new Set(downloadingSubtitleKeys.value)
    cleared.delete(scopedKey)
    downloadingSubtitleKeys.value = cleared
  }
}

const downloadVideo = async () => {
  const task = activeTask.value
  if (!task || isVideoTaskDownloading(task.id) || !isTauriRuntime()) {
    return
  }

  const isContinuing = hasPartialVideoDownload.value
  const taskId = task.id
  setVideoTaskDownloading(taskId, true)
  setDownloadProgress({
    taskId,
    kind: 'video',
    key: 'video',
    progress: 2,
    status: 'active',
    message: isContinuing ? '准备继续下载视频' : '准备下载视频',
  }, { reset: true })
  clearTaskError(videoErrorsByTaskId, taskId)

  try {
    const updatedTask = await invoke<HomeVideoTask>('download_home_video_task_video', {
      request: {
        taskId,
      },
    })
    upsertTask(updatedTask)
    await loadWorkbenchSnapshot()
  } catch (error) {
    const message = stringifyError(error, '下载视频失败')
    setTaskError(videoErrorsByTaskId, taskId, message)
    setDownloadProgress({
      taskId,
      kind: 'video',
      key: 'video',
      progress: 100,
      status: 'failed',
      message,
    })
    await reloadTask(taskId)
  } finally {
    setVideoTaskDownloading(taskId, false)
  }
}

const addVideoToWorkbench = async () => {
  const task = activeTask.value
  if (!task || isAddingWorkbenchVideo.value || isWorkbenchRunning.value || !isTauriRuntime()) {
    return
  }

  isAddingWorkbenchVideo.value = true
  pageError.value = ''
  try {
    const snapshot = await invoke<HomeWorkbenchSnapshot>('add_home_workbench_video_input', {
      request: { taskId: task.id },
    })
    applyWorkbenchSnapshot(snapshot)
    await reloadTask(task.id)
  } catch (error) {
    pageError.value = stringifyError(error, '添加视频到工作台失败')
  } finally {
    isAddingWorkbenchVideo.value = false
  }
}

const addSubtitleToWorkbench = async (subtitle: HomeVideoSubtitle) => {
  const task = activeTask.value
  if (!task || addingWorkbenchSubtitleIds.value.has(subtitle.id) || isWorkbenchRunning.value || !isTauriRuntime()) {
    return
  }

  addingWorkbenchSubtitleIds.value = new Set(addingWorkbenchSubtitleIds.value).add(subtitle.id)
  pageError.value = ''
  try {
    const snapshot = await invoke<HomeWorkbenchSnapshot>('add_home_workbench_subtitle_input', {
      request: { taskId: task.id, subtitleId: subtitle.id },
    })
    applyWorkbenchSnapshot(snapshot)
    await reloadTask(task.id)
  } catch (error) {
    pageError.value = stringifyError(error, '添加字幕到工作台失败')
  } finally {
    const next = new Set(addingWorkbenchSubtitleIds.value)
    next.delete(subtitle.id)
    addingWorkbenchSubtitleIds.value = next
  }
}

const startWorkbench = async () => {
  const taskId = activeTaskId.value
  if (!taskId || !canStartWorkbench.value || !isTauriRuntime()) {
    return
  }

  isWorkbenchStarting.value = true
  pageError.value = ''
  try {
    const snapshot = await invoke<HomeWorkbenchSnapshot>('start_home_workbench', {
      request: {
        taskId,
        options: workbenchOptions.value,
      },
    })
    applyWorkbenchSnapshot(snapshot)
    await reloadTask(taskId)
  } catch (error) {
    pageError.value = stringifyError(error, '工作台执行失败')
    await loadWorkbenchSnapshot()
    await reloadTask(taskId)
  } finally {
    isWorkbenchStarting.value = false
  }
}

const updateWorkbenchOptions = (patch: Partial<HomeWorkbenchOptions>) => {
  const next = normalizeWorkbenchOptions({
    ...workbenchOptions.value,
    ...patch,
  })
  workbenchOptions.value = next
  void saveWorkbenchOptions()
}

const updateWorkbenchTtsInterval = (event: Event) => {
  const value = Number((event.target as HTMLInputElement | null)?.value)
  updateWorkbenchOptions({ dubbingTtsIntervalMs: Number.isFinite(value) ? value : workbenchOptions.value.dubbingTtsIntervalMs })
}

const updateWorkbenchBackgroundMusicVolume = (event: Event) => {
  const value = Number((event.target as HTMLInputElement | null)?.value)
  updateWorkbenchOptions({
    dubbingBackgroundMusicVolume: Number.isFinite(value) ? value : workbenchOptions.value.dubbingBackgroundMusicVolume,
  })
}

const openWorkbenchParameterPanel = (panel: WorkbenchParameterPanel) => {
  activeWorkbenchParameterPanel.value = panel
}

const closeWorkbenchParameterPanel = () => {
  activeWorkbenchParameterPanel.value = null
  closeWorkbenchDialog()
}

const openWorkbenchDialog = (dialog: WorkbenchDialog) => {
  if (isWorkbenchRunning.value) {
    return
  }
  workbenchDialogSearch.value = ''
  activeWorkbenchDialog.value = dialog
}

const closeWorkbenchDialog = () => {
  activeWorkbenchDialog.value = null
  workbenchDialogSearch.value = ''
}

const selectWorkbenchDialogOption = (value: string) => {
  switch (activeWorkbenchDialog.value) {
    case WorkbenchDialog.SubtitleSource:
      if (value.startsWith('downloaded:')) {
        updateWorkbenchOptions({
          subtitleSource: 'downloaded',
          subtitleId: value.slice('downloaded:'.length),
        })
      } else {
        updateWorkbenchOptions({
          subtitleSource: value as WorkbenchSubtitleSource,
          subtitleId: '',
        })
      }
      break
    case WorkbenchDialog.TranscriptionModel:
      updateWorkbenchOptions({ transcriptionModel: value as TranscriptionModel })
      break
    case WorkbenchDialog.SourceLanguage:
      updateWorkbenchOptions({ sourceLanguage: value })
      break
    case WorkbenchDialog.TranscriptionFormat:
      updateWorkbenchOptions({ transcriptionFormat: value as SubtitleFormat })
      break
    case WorkbenchDialog.TranslationService:
      updateWorkbenchOptions({ translationService: value as TranslationService })
      break
    case WorkbenchDialog.VideoContentType:
      updateWorkbenchOptions({ videoContentType: value as VideoContentType })
      break
    case WorkbenchDialog.TargetLanguage:
      updateWorkbenchOptions({ targetLanguage: value })
      break
    case WorkbenchDialog.OutputMode:
      updateWorkbenchOptions({ outputMode: value as OutputMode })
      break
    case WorkbenchDialog.TranslationFormat:
      updateWorkbenchOptions({ translationFormat: value as SubtitleFormat })
      break
    case WorkbenchDialog.ReferenceAudio:
      updateWorkbenchOptions({ dubbingReferenceAudioSource: value as ReferenceAudioSource })
      break
    default:
      break
  }
  closeWorkbenchDialog()
}

const selectWorkbenchExportDir = async () => {
  if (!isTauriRuntime() || isWorkbenchRunning.value) {
    return
  }

  const selected = await open({
    title: '选择工作台导出目录',
    directory: true,
    multiple: false,
  })

  if (typeof selected === 'string') {
    updateWorkbenchOptions({ exportDir: selected })
  }
}

const openWorkbenchOutput = async () => {
  const artifact = exportedArtifacts.value[0]
  if (!artifact || !isTauriRuntime()) {
    return
  }

  try {
    await revealItemInDir(artifact.path)
  } catch (error) {
    pageError.value = stringifyError(error, '打开导出目录失败')
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
      workbenchSnapshot.value = null
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
    tasks.value = sortTasksByCreatedAtDesc(next)
    return
  }

  tasks.value = sortTasksByCreatedAtDesc([task, ...tasks.value])
}

const applyWorkbenchSnapshot = (snapshot: HomeWorkbenchSnapshot) => {
  workbenchSnapshot.value = snapshot
  workbenchOptions.value = normalizeWorkbenchOptions(snapshot.options)
  syncSelectedWorkbenchStage()
}

const syncSelectedWorkbenchStage = () => {
  const stages = workbenchStages.value
  if (
    isWorkbenchStageSelectionPinned.value &&
    selectedWorkbenchStageKey.value &&
    stages.some((stage) => stage.key === selectedWorkbenchStageKey.value)
  ) {
    return
  }
  selectedWorkbenchStageKey.value = preferredWorkbenchStage.value?.key ?? ''
}

const selectWorkbenchStage = (stage: HomeWorkbenchStage) => {
  selectedWorkbenchStageKey.value = stage.key
  isWorkbenchStageSelectionPinned.value = true
}

const workbenchArtifactByKind = (kind: string) => {
  return workbenchSnapshot.value?.artifacts.find((artifact) => artifact.kind === kind && artifact.path) ?? null
}

const readRecordValue = (value: unknown): Record<string, unknown> => {
  return value && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, unknown>) : {}
}

const readStringValue = (value: unknown) => {
  return typeof value === 'string' ? value : ''
}

const readNumberValue = (value: unknown) => {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined
}

const detailStepFromRecord = (key: string, label: string, value: Record<string, unknown>): WorkbenchDetailStep => ({
  key,
  label,
  progress: clampProgress(readNumberValue(value.progress) ?? 0),
  status: normalizeWorkbenchDetailStatus(readStringValue(value.status)),
  message: readStringValue(value.message) || '等待处理',
})

const readSubtitleSegments = (value: unknown): WorkbenchSubtitleSegment[] => {
  if (!Array.isArray(value)) {
    return []
  }
  return value.map((item, index) => {
    const record = readRecordValue(item)
    return {
      uid: readStringValue(record.uid) || `workbench-subtitle-${index}`,
      text: readStringValue(record.text),
      startTime: readNumberValue(record.startTime) ?? 0,
      endTime: readNumberValue(record.endTime) ?? 0,
      status: readStringValue(record.status) || 'raw',
    }
  })
}

const normalizeWorkbenchDetailStatus = (value: string): WorkbenchDetailStageStatus => {
  return ['pending', 'active', 'done', 'skipped', 'failed', 'interrupted'].includes(value)
    ? (value as WorkbenchDetailStageStatus)
    : 'pending'
}

const sortTasksByCreatedAtDesc = (items: HomeVideoTask[]) => {
  return [...items].sort((left, right) => {
    const timeDiff = taskCreatedTime(right) - taskCreatedTime(left)
    if (timeDiff !== 0) {
      return timeDiff
    }

    const createdAtDiff = right.createdAt.localeCompare(left.createdAt)
    return createdAtDiff !== 0 ? createdAtDiff : right.id.localeCompare(left.id)
  })
}

const taskCreatedTime = (task: HomeVideoTask) => {
  const timestamp = Date.parse(task.createdAt)
  return Number.isFinite(timestamp) ? timestamp : 0
}

const removeTask = (taskId: string) => {
  tasks.value = tasks.value.filter((task) => task.id !== taskId)
  const refreshed = new Set(autoRefreshedTaskIds.value)
  refreshed.delete(taskId)
  autoRefreshedTaskIds.value = refreshed
  clearTaskDownloadState(taskId)
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
  if (event.key !== 'Escape') {
    return
  }

  if (activeWorkbenchDialog.value) {
    closeWorkbenchDialog()
    return
  }

  if (activeWorkbenchParameterPanel.value) {
    closeWorkbenchParameterPanel()
    return
  }

  closeAddDialog()
  closeDeleteDialog()
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

const normalizeWorkbenchOptions = (options: HomeWorkbenchOptions): HomeWorkbenchOptions => {
  const next = { ...options }
  if (next.subtitleSource !== 'downloaded') {
    next.subtitleSource = 'transcribe'
    next.subtitleId = ''
  }
  next.transcriptionModel = readOptionValue(
    next.transcriptionModel,
    transcriptionModelOptions,
    TranscriptionModel.Bilibili,
  )
  if (!sourceLanguageOptions.some((option) => option.value === next.sourceLanguage)) {
    next.sourceLanguage = 'auto'
  }
  next.transcriptionFormat = readOptionValue(next.transcriptionFormat, subtitleFormatOptions, SubtitleFormat.Srt)
  next.translationFormat = readOptionValue(next.translationFormat, subtitleFormatOptions, SubtitleFormat.Srt)
  next.translationService = readOptionValue(next.translationService, translationServiceOptions, TranslationService.Llm)
  next.videoContentType = readOptionValue(next.videoContentType, videoContentTypeOptions, VideoContentType.General)
  next.outputMode = readOptionValue(next.outputMode, outputModeOptions, OutputMode.Bilingual)
  if (!targetLanguageOptions.some((option) => option.value === next.targetLanguage)) {
    next.targetLanguage = 'zh-Hans'
  }
  next.dubbingReferenceAudioSource = readOptionValue(
    next.dubbingReferenceAudioSource,
    referenceAudioSourceOptions,
    ReferenceAudioSource.ExistingDubbing,
  )
  if (next.dubbingReferenceAudioSource !== ReferenceAudioSource.CustomAudioFile) {
    next.dubbingCustomReferenceAudioPath = ''
  }
  if (next.dubbingEnabled) {
    next.translationEnabled = true
  }
  if (!next.translationEnabled) {
    next.dubbingEnabled = false
  }
  next.translationBatchSize = clampNumber(next.translationBatchSize, 10, 100)
  next.translationThreadCount = clampNumber(next.translationThreadCount, 1, 100)
  next.dubbingTtsIntervalMs = clampNumber(next.dubbingTtsIntervalMs, 0, 1000)
  next.dubbingBackgroundMusicVolume = Math.min(1, Math.max(0, next.dubbingBackgroundMusicVolume))
  return next
}

function createDefaultWorkbenchStages(): HomeWorkbenchStage[] {
  return [
    { key: 'download-video', label: '下载视频', progress: 0, status: 'pending', message: '等待下载视频' },
    { key: 'prepare-subtitle', label: '准备字幕', progress: 0, status: 'pending', message: '等待准备字幕' },
    { key: 'translation', label: '翻译', progress: 0, status: 'pending', message: '等待翻译' },
    { key: 'dubbing', label: '配音', progress: 0, status: 'pending', message: '等待配音' },
    { key: 'export', label: '导出', progress: 0, status: 'pending', message: '等待导出' },
  ]
}

const stageOrderLabel = (key: string) => {
  const index = createDefaultWorkbenchStages().findIndex((stage) => stage.key === key)
  return index >= 0 ? `${index + 1}` : ''
}

const artifactLabel = (kind: string) => {
  if (kind === 'exported-video') {
    return '导出视频'
  }
  if (kind === 'exported-subtitle') {
    return '导出字幕'
  }
  return '工作台产物'
}

const workbenchStageStatusLabel = (status: WorkbenchDetailStageStatus | string) => {
  switch (status) {
    case 'active':
      return '执行中'
    case 'done':
      return '完成'
    case 'skipped':
      return '跳过'
    case 'failed':
      return '失败'
    case 'interrupted':
      return '已中断'
    default:
      return '等待'
  }
}

const subtitleSubtitleLabel = (subtitle: HomeVideoSubtitle) => {
  const name = subtitle.languageName || subtitle.language || '字幕'
  return `${name} · ${subtitleSourceLabel(subtitle.sourceKind)}`
}

const clampNumber = (value: number, min: number, max: number) => {
  return Math.min(max, Math.max(min, Number.isFinite(value) ? Math.round(value) : min))
}

const subtitleKey = (option: HomeVideoSubtitleOption) => `${option.sourceKind}:${option.language}`

const downloadProgressKey = (taskId: string, kind: DownloadProgressKind, key: string) => `${taskId}:${kind}:${key}`

const isVideoTaskDownloading = (taskId: string) => {
  const progress = downloadProgressByKey.value.get(downloadProgressKey(taskId, 'video', 'video'))
  return downloadingVideoTaskIds.value.has(taskId) || progress?.status === 'active'
}

const setVideoTaskDownloading = (taskId: string, downloading: boolean) => {
  const next = new Set(downloadingVideoTaskIds.value)
  if (downloading) {
    next.add(taskId)
  } else {
    next.delete(taskId)
  }
  downloadingVideoTaskIds.value = next
}

const isSubtitleDownloading = (option: HomeVideoSubtitleOption) => {
  const taskId = activeTask.value?.id
  if (!taskId) {
    return false
  }

  const key = subtitleKey(option)
  const scopedKey = downloadProgressKey(taskId, 'subtitle', key)
  const progress = downloadProgressByKey.value.get(scopedKey)
  return downloadingSubtitleKeys.value.has(scopedKey) || progress?.status === 'active'
}

const subtitleDownloadProgress = (option: HomeVideoSubtitleOption) => {
  if (!activeTask.value) {
    return null
  }

  return (
    downloadProgressByKey.value.get(
      downloadProgressKey(activeTask.value.id, 'subtitle', subtitleKey(option)),
    ) ?? null
  )
}

const subtitleDownloadProgressValue = (option: HomeVideoSubtitleOption) => {
  return clampProgress(subtitleDownloadProgress(option)?.progress ?? (isSubtitleDownloading(option) ? 2 : 0))
}

const subtitleDownloadProgressMessage = (option: HomeVideoSubtitleOption) => {
  return subtitleDownloadProgress(option)?.message || '字幕下载中'
}

const downloadedSubtitleForOption = (option: HomeVideoSubtitleOption) => {
  return activeTask.value?.downloadedSubtitles.find((subtitle) => {
    return subtitle.language === option.language && subtitle.sourceKind === option.sourceKind
  }) ?? null
}

const isSubtitleAddedToWorkbench = (option: HomeVideoSubtitleOption) => {
  const subtitle = downloadedSubtitleForOption(option)
  return Boolean(subtitle && registeredSubtitleIds.value.has(subtitle.id))
}

const isAddingSubtitleToWorkbench = (option: HomeVideoSubtitleOption) => {
  const subtitle = downloadedSubtitleForOption(option)
  return Boolean(subtitle && addingWorkbenchSubtitleIds.value.has(subtitle.id))
}

const addSubtitleOptionToWorkbench = (option: HomeVideoSubtitleOption) => {
  const subtitle = downloadedSubtitleForOption(option)
  if (subtitle) {
    void addSubtitleToWorkbench(subtitle)
  }
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

const activeTaskScopedError = (errors: Ref<Map<string, string>>) => {
  const taskId = activeTask.value?.id
  return taskId ? (errors.value.get(taskId) ?? '') : ''
}

const setTaskError = (errors: Ref<Map<string, string>>, taskId: string, message: string) => {
  const next = new Map(errors.value)
  if (message) {
    next.set(taskId, message)
  } else {
    next.delete(taskId)
  }
  errors.value = next
}

const clearTaskError = (errors: Ref<Map<string, string>>, taskId: string) => {
  setTaskError(errors, taskId, '')
}

const clearTaskDownloadState = (taskId: string) => {
  setVideoTaskDownloading(taskId, false)
  if (activeTaskId.value === taskId) {
    isAddingWorkbenchVideo.value = false
    addingWorkbenchSubtitleIds.value = new Set()
  }

  const scopedPrefix = `${taskId}:`
  downloadingSubtitleKeys.value = new Set(
    [...downloadingSubtitleKeys.value].filter((key) => !key.startsWith(scopedPrefix)),
  )
  downloadProgressByKey.value = new Map(
    [...downloadProgressByKey.value].filter(([key]) => !key.startsWith(scopedPrefix)),
  )
  clearTaskError(subtitleErrorsByTaskId, taskId)
  clearTaskError(videoErrorsByTaskId, taskId)
}

const displayThumbnailUrl = (task: HomeVideoTask) => {
  return isInlineThumbnailUrl(task.thumbnailUrl) ? task.thumbnailUrl : ''
}

const hasRemoteThumbnail = (task: HomeVideoTask) => {
  return /^https?:\/\//i.test(task.thumbnailUrl)
}

const isInlineThumbnailUrl = (value: string) => {
  return value.startsWith('data:image/')
}

const clampProgress = (value: number) => {
  if (!Number.isFinite(value)) {
    return 0
  }

  return Math.min(100, Math.max(0, Math.round(value)))
}

const normalizeSegmentStatus = (status?: string) => {
  const value = status || 'raw'
  return ['raw', 'active', 'done', 'failed', 'translating', 'translated', 'optimizing', 'optimized', 'kept'].includes(value)
    ? value
    : 'raw'
}

const segmentStatusLabel = (status?: string) => {
  switch (normalizeSegmentStatus(status)) {
    case 'active':
      return '处理中'
    case 'done':
      return '完成'
    case 'failed':
      return '失败'
    case 'translating':
      return '翻译中'
    case 'translated':
      return '已翻译'
    case 'optimizing':
      return '优化中'
    case 'optimized':
      return '已优化'
    case 'kept':
      return '保留原文'
    default:
      return '原文'
  }
}

const formatSegmentTime = (ms: number) => {
  const safeMs = Number.isFinite(ms) ? Math.max(0, Math.round(ms)) : 0
  const totalSeconds = Math.floor(safeMs / 1000)
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  const millis = safeMs % 1000
  return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${seconds
    .toString()
    .padStart(2, '0')}.${millis.toString().padStart(3, '0')}`
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
