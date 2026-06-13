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
                  <div class="home-workbench-progress" role="progressbar" aria-label="工作台总进度" :aria-valuenow="workbenchProgress" aria-valuemin="0" aria-valuemax="100">
                    <div class="translate-progress-track">
                      <span class="translate-progress-bar" :style="{ width: `${workbenchProgress}%` }" />
                    </div>
                    <span class="translate-progress-value">{{ workbenchProgress }}%</span>
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
                  <span>{{ isWorkbenchRunning ? '执行中' : '开始执行' }}</span>
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

              <div class="home-workbench-stage-list" aria-label="工作台执行步骤">
                <article
                  v-for="stage in workbenchStages"
                  :key="stage.key"
                  class="home-workbench-stage"
                  :class="stage.status"
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
                  </span>
                  <span class="home-workbench-stage-value">{{ stage.progress }}%</span>
                </article>
              </div>

              <div class="home-workbench-grid">
                <section class="home-workbench-block" aria-label="字幕参数">
                  <div class="home-workbench-block-title">
                    <Captions :stroke-width="2.1" aria-hidden="true" />
                    <span>字幕</span>
                  </div>
                  <button class="setting-row setting-row-button compact" type="button" :disabled="isWorkbenchRunning" @click="openWorkbenchDialog(WorkbenchDialog.SubtitleSource)">
                    <span class="setting-copy">
                      <span class="setting-title">字幕来源</span>
                      <span class="setting-subtitle">{{ workbenchSubtitleSourceSubtitle }}</span>
                    </span>
                    <span class="setting-inline-action">
                      <span class="setting-value">{{ workbenchSubtitleSourceLabel }}</span>
                      <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                    </span>
                  </button>
                  <button class="setting-row setting-row-button compact" type="button" :disabled="isWorkbenchRunning || workbenchOptions.subtitleSource === 'downloaded'" @click="openWorkbenchDialog(WorkbenchDialog.TranscriptionModel)">
                    <span class="setting-copy">
                      <span class="setting-title">转录模型</span>
                      <span class="setting-subtitle">自动转录时使用</span>
                    </span>
                    <span class="setting-inline-action">
                      <span class="setting-value">{{ workbenchTranscriptionModelLabel }}</span>
                      <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                    </span>
                  </button>
                  <button class="setting-row setting-row-button compact" type="button" :disabled="isWorkbenchRunning" @click="openWorkbenchDialog(WorkbenchDialog.SourceLanguage)">
                    <span class="setting-copy">
                      <span class="setting-title">源语言</span>
                      <span class="setting-subtitle">转录时使用</span>
                    </span>
                    <span class="setting-inline-action">
                      <span class="setting-value">{{ workbenchSourceLanguageLabel }}</span>
                      <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                    </span>
                  </button>
                  <button class="setting-row setting-row-button compact" type="button" :disabled="isWorkbenchRunning" @click="openWorkbenchDialog(WorkbenchDialog.TranscriptionFormat)">
                    <span class="setting-copy">
                      <span class="setting-title">转录格式</span>
                      <span class="setting-subtitle">自动转录字幕格式</span>
                    </span>
                    <span class="setting-inline-action">
                      <span class="setting-value">{{ workbenchTranscriptionFormatLabel }}</span>
                      <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                    </span>
                  </button>
                  <div class="setting-row compact">
                    <span class="setting-copy">
                      <span class="setting-title">智能断句</span>
                      <span class="setting-subtitle">转录后优化字幕断句</span>
                    </span>
                    <button class="setting-toggle" :class="{ active: workbenchOptions.isSmartSegmentationEnabled }" type="button" :aria-pressed="workbenchOptions.isSmartSegmentationEnabled" :disabled="isWorkbenchRunning" @click="updateWorkbenchOptions({ isSmartSegmentationEnabled: !workbenchOptions.isSmartSegmentationEnabled })">
                      <span class="setting-toggle-label">{{ workbenchOptions.isSmartSegmentationEnabled ? '开' : '关' }}</span>
                      <span class="setting-toggle-track" aria-hidden="true"><span class="setting-toggle-thumb" /></span>
                    </button>
                  </div>
                  <div class="setting-row compact">
                    <span class="setting-copy">
                      <span class="setting-title">字幕校正</span>
                      <span class="setting-subtitle">转录后校正字幕内容</span>
                    </span>
                    <button class="setting-toggle" :class="{ active: workbenchOptions.isSubtitleCorrectionEnabled }" type="button" :aria-pressed="workbenchOptions.isSubtitleCorrectionEnabled" :disabled="isWorkbenchRunning || workbenchOptions.subtitleSource === 'downloaded'" @click="updateWorkbenchOptions({ isSubtitleCorrectionEnabled: !workbenchOptions.isSubtitleCorrectionEnabled })">
                      <span class="setting-toggle-label">{{ workbenchOptions.isSubtitleCorrectionEnabled ? '开' : '关' }}</span>
                      <span class="setting-toggle-track" aria-hidden="true"><span class="setting-toggle-thumb" /></span>
                    </button>
                  </div>
                </section>

                <section class="home-workbench-block" aria-label="翻译参数">
                  <div class="home-workbench-block-title">
                    <Languages :stroke-width="2.1" aria-hidden="true" />
                    <span>翻译</span>
                  </div>
                  <div class="setting-row compact">
                    <span class="setting-copy">
                      <span class="setting-title">翻译与优化</span>
                      <span class="setting-subtitle">开启后翻译字幕并生成最终字幕</span>
                    </span>
                    <button class="setting-toggle" :class="{ active: workbenchOptions.translationEnabled }" type="button" :aria-pressed="workbenchOptions.translationEnabled" :disabled="isWorkbenchRunning" @click="toggleWorkbenchTranslation">
                      <span class="setting-toggle-label">{{ workbenchOptions.translationEnabled ? '开' : '关' }}</span>
                      <span class="setting-toggle-track" aria-hidden="true"><span class="setting-toggle-thumb" /></span>
                    </button>
                  </div>
                  <button class="setting-row setting-row-button compact" type="button" :disabled="isWorkbenchRunning || !workbenchOptions.translationEnabled" @click="openWorkbenchDialog(WorkbenchDialog.TranslationService)">
                    <span class="setting-copy">
                      <span class="setting-title">翻译服务</span>
                      <span class="setting-subtitle">字幕翻译使用的服务</span>
                    </span>
                    <span class="setting-inline-action">
                      <span class="setting-value">{{ workbenchTranslationServiceLabel }}</span>
                      <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                    </span>
                  </button>
                  <button class="setting-row setting-row-button compact" type="button" :disabled="isWorkbenchRunning || !workbenchOptions.translationEnabled" @click="openWorkbenchDialog(WorkbenchDialog.VideoContentType)">
                    <span class="setting-copy">
                      <span class="setting-title">视频类型</span>
                      <span class="setting-subtitle">影响翻译提示词</span>
                    </span>
                    <span class="setting-inline-action">
                      <span class="setting-value">{{ workbenchVideoContentTypeLabel }}</span>
                      <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                    </span>
                  </button>
                  <button class="setting-row setting-row-button compact" type="button" :disabled="isWorkbenchRunning || !workbenchOptions.translationEnabled" @click="openWorkbenchDialog(WorkbenchDialog.TargetLanguage)">
                    <span class="setting-copy">
                      <span class="setting-title">目标语言</span>
                      <span class="setting-subtitle">最终字幕语言</span>
                    </span>
                    <span class="setting-inline-action">
                      <span class="setting-value">{{ workbenchTargetLanguageLabel }}</span>
                      <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                    </span>
                  </button>
                  <button class="setting-row setting-row-button compact" type="button" :disabled="isWorkbenchRunning || !workbenchOptions.translationEnabled" @click="openWorkbenchDialog(WorkbenchDialog.OutputMode)">
                    <span class="setting-copy">
                      <span class="setting-title">输出模式</span>
                      <span class="setting-subtitle">最终字幕呈现方式</span>
                    </span>
                    <span class="setting-inline-action">
                      <span class="setting-value">{{ workbenchOutputModeLabel }}</span>
                      <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                    </span>
                  </button>
                  <button class="setting-row setting-row-button compact" type="button" :disabled="isWorkbenchRunning || !workbenchOptions.translationEnabled" @click="openWorkbenchDialog(WorkbenchDialog.TranslationFormat)">
                    <span class="setting-copy">
                      <span class="setting-title">字幕格式</span>
                      <span class="setting-subtitle">翻译后导出的字幕格式</span>
                    </span>
                    <span class="setting-inline-action">
                      <span class="setting-value">{{ workbenchTranslationFormatLabel }}</span>
                      <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                    </span>
                  </button>
                  <div class="setting-row compact">
                    <span class="setting-copy">
                      <span class="setting-title">字幕翻译</span>
                      <span class="setting-subtitle">关闭后只保留优化处理</span>
                    </span>
                    <button class="setting-toggle" :class="{ active: workbenchOptions.isSubtitleTranslationEnabled }" type="button" :aria-pressed="workbenchOptions.isSubtitleTranslationEnabled" :disabled="isWorkbenchRunning || !workbenchOptions.translationEnabled" @click="updateWorkbenchOptions({ isSubtitleTranslationEnabled: !workbenchOptions.isSubtitleTranslationEnabled })">
                      <span class="setting-toggle-label">{{ workbenchOptions.isSubtitleTranslationEnabled ? '开' : '关' }}</span>
                      <span class="setting-toggle-track" aria-hidden="true"><span class="setting-toggle-thumb" /></span>
                    </button>
                  </div>
                  <div class="setting-row compact">
                    <span class="setting-copy">
                      <span class="setting-title">译后优化</span>
                      <span class="setting-subtitle">翻译后继续优化译文</span>
                    </span>
                    <button class="setting-toggle" :class="{ active: workbenchOptions.isPostTranslationOptimizationEnabled }" type="button" :aria-pressed="workbenchOptions.isPostTranslationOptimizationEnabled" :disabled="isWorkbenchRunning || !workbenchOptions.translationEnabled" @click="updateWorkbenchOptions({ isPostTranslationOptimizationEnabled: !workbenchOptions.isPostTranslationOptimizationEnabled })">
                      <span class="setting-toggle-label">{{ workbenchOptions.isPostTranslationOptimizationEnabled ? '开' : '关' }}</span>
                      <span class="setting-toggle-track" aria-hidden="true"><span class="setting-toggle-thumb" /></span>
                    </button>
                  </div>
                  <div class="setting-row compact">
                    <span class="setting-copy">
                      <span class="setting-title">反思翻译</span>
                      <span class="setting-subtitle">提升译文质量但会增加耗时</span>
                    </span>
                    <button class="setting-toggle" :class="{ active: workbenchOptions.needsReflectionTranslation }" type="button" :aria-pressed="workbenchOptions.needsReflectionTranslation" :disabled="isWorkbenchRunning || !workbenchOptions.translationEnabled" @click="updateWorkbenchOptions({ needsReflectionTranslation: !workbenchOptions.needsReflectionTranslation })">
                      <span class="setting-toggle-label">{{ workbenchOptions.needsReflectionTranslation ? '开' : '关' }}</span>
                      <span class="setting-toggle-track" aria-hidden="true"><span class="setting-toggle-thumb" /></span>
                    </button>
                  </div>
                  <div class="setting-row compact range">
                    <span class="setting-copy">
                      <span class="setting-title">批处理大小</span>
                      <span class="setting-subtitle">每批处理字幕数量</span>
                    </span>
                    <div class="setting-range-control dubbing-range-control">
                      <span class="setting-range-value dubbing-range-value">{{ workbenchOptions.translationBatchSize }}</span>
                      <input
                        class="setting-range"
                        type="range"
                        min="10"
                        max="100"
                        step="1"
                        :value="workbenchOptions.translationBatchSize"
                        :disabled="isWorkbenchRunning || !workbenchOptions.translationEnabled"
                        aria-label="工作台翻译批处理大小"
                        @change="updateWorkbenchBatchSize"
                      />
                    </div>
                  </div>
                  <div class="setting-row compact range">
                    <span class="setting-copy">
                      <span class="setting-title">线程数</span>
                      <span class="setting-subtitle">AI 请求并行数量</span>
                    </span>
                    <div class="setting-range-control dubbing-range-control">
                      <span class="setting-range-value dubbing-range-value">{{ workbenchOptions.translationThreadCount }}</span>
                      <input
                        class="setting-range"
                        type="range"
                        min="1"
                        max="100"
                        step="1"
                        :value="workbenchOptions.translationThreadCount"
                        :disabled="isWorkbenchRunning || !workbenchOptions.translationEnabled"
                        aria-label="工作台翻译线程数"
                        @change="updateWorkbenchThreadCount"
                      />
                    </div>
                  </div>
                </section>

                <section class="home-workbench-block" aria-label="配音和导出参数">
                  <div class="home-workbench-block-title">
                    <MicVocal :stroke-width="2.1" aria-hidden="true" />
                    <span>配音与导出</span>
                  </div>
                  <div class="setting-row compact">
                    <span class="setting-copy">
                      <span class="setting-title">配音</span>
                      <span class="setting-subtitle">开启后根据翻译字幕生成配音视频</span>
                    </span>
                    <button class="setting-toggle" :class="{ active: workbenchOptions.dubbingEnabled }" type="button" :aria-pressed="workbenchOptions.dubbingEnabled" :disabled="isWorkbenchRunning || !workbenchOptions.translationEnabled" @click="toggleWorkbenchDubbing">
                      <span class="setting-toggle-label">{{ workbenchOptions.dubbingEnabled ? '开' : '关' }}</span>
                      <span class="setting-toggle-track" aria-hidden="true"><span class="setting-toggle-thumb" /></span>
                    </button>
                  </div>
                  <button class="setting-row setting-row-button compact" type="button" :disabled="isWorkbenchRunning || !workbenchOptions.dubbingEnabled" @click="openWorkbenchDialog(WorkbenchDialog.ReferenceAudio)">
                    <span class="setting-copy">
                      <span class="setting-title">参考音频</span>
                      <span class="setting-subtitle">配音克隆来源</span>
                    </span>
                    <span class="setting-inline-action">
                      <span class="setting-value">{{ workbenchReferenceAudioLabel }}</span>
                      <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
                    </span>
                  </button>
                  <div class="setting-row compact">
                    <span class="setting-copy">
                      <span class="setting-title">自定义音频</span>
                      <span class="setting-subtitle">{{ workbenchCustomReferenceAudioLabel }}</span>
                    </span>
                    <button
                      class="settings-action"
                      type="button"
                      :disabled="isWorkbenchRunning || !workbenchOptions.dubbingEnabled || workbenchOptions.dubbingReferenceAudioSource !== ReferenceAudioSource.CustomAudioFile"
                      @click="selectWorkbenchCustomReferenceAudio"
                    >
                      选择音频
                    </button>
                  </div>
                  <div class="setting-row compact range">
                    <span class="setting-copy">
                      <span class="setting-title">TTS 间隔</span>
                      <span class="setting-subtitle">分段语音停顿时长</span>
                    </span>
                    <div class="setting-range-control dubbing-range-control">
                      <span class="setting-range-value dubbing-range-value">{{ workbenchOptions.dubbingTtsIntervalMs }} 毫秒</span>
                      <input
                        class="setting-range"
                        type="range"
                        min="0"
                        max="1000"
                        step="10"
                        :value="workbenchOptions.dubbingTtsIntervalMs"
                        :disabled="isWorkbenchRunning || !workbenchOptions.dubbingEnabled"
                        aria-label="工作台 TTS 间隔"
                        @change="updateWorkbenchTtsInterval"
                      />
                    </div>
                  </div>
                  <div class="setting-row compact">
                    <span class="setting-copy">
                      <span class="setting-title">背景音乐</span>
                      <span class="setting-subtitle">保留并混入源视频伴奏</span>
                    </span>
                    <button class="setting-toggle" :class="{ active: workbenchOptions.dubbingIsBackgroundMusicEnabled }" type="button" :aria-pressed="workbenchOptions.dubbingIsBackgroundMusicEnabled" :disabled="isWorkbenchRunning || !workbenchOptions.dubbingEnabled" @click="updateWorkbenchOptions({ dubbingIsBackgroundMusicEnabled: !workbenchOptions.dubbingIsBackgroundMusicEnabled })">
                      <span class="setting-toggle-label">{{ workbenchOptions.dubbingIsBackgroundMusicEnabled ? '开' : '关' }}</span>
                      <span class="setting-toggle-track" aria-hidden="true"><span class="setting-toggle-thumb" /></span>
                    </button>
                  </div>
                  <div class="setting-row compact range">
                    <span class="setting-copy">
                      <span class="setting-title">背景音量</span>
                      <span class="setting-subtitle">最终视频背景音乐音量</span>
                    </span>
                    <div class="setting-range-control dubbing-range-control">
                      <span class="setting-range-value dubbing-range-value">{{ workbenchOptions.dubbingBackgroundMusicVolume.toFixed(1) }}</span>
                      <input
                        class="setting-range"
                        type="range"
                        min="0"
                        max="1"
                        step="0.1"
                        :value="workbenchOptions.dubbingBackgroundMusicVolume"
                        :disabled="isWorkbenchRunning || !workbenchOptions.dubbingEnabled || !workbenchOptions.dubbingIsBackgroundMusicEnabled"
                        aria-label="工作台背景音乐音量"
                        @change="updateWorkbenchBackgroundMusicVolume"
                      />
                    </div>
                  </div>
                  <div class="setting-row compact">
                    <span class="setting-copy">
                      <span class="setting-title">导出目录</span>
                      <span class="setting-subtitle">{{ workbenchExportDirLabel }}</span>
                    </span>
                    <button class="settings-action" type="button" :disabled="isWorkbenchRunning" @click="selectWorkbenchExportDir">
                      选择目录
                    </button>
                  </div>
                </section>
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
  Captions,
  CheckCircle2,
  ChevronRight,
  CircleAlert,
  Clock,
  Download,
  FileCheck2,
  FolderOpen,
  Link2,
  ListTodo,
  ListVideo,
  LoaderCircle,
  Languages,
  MicVocal,
  Play,
  Plus,
  RefreshCw,
  Search,
  Trash2,
  Video,
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
const downloadingVideoTaskIds = ref(new Set<string>())
const isAddDialogOpen = ref(false)
const isDeleteDialogOpen = ref(false)
const pageError = ref('')
const addError = ref('')
const deleteError = ref('')
const subtitleErrorsByTaskId = ref(new Map<string, string>())
const videoErrorsByTaskId = ref(new Map<string, string>())
const downloadingSubtitleKeys = ref(new Set<string>())
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
const referenceAudioExtensions = ['wav', 'mp3', 'm4a', 'aac', 'flac', 'ogg', 'opus', 'wma']

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

const workbenchProgress = computed(() => clampProgress(workbenchSnapshot.value?.progress ?? 0))

const workbenchStages = computed(() => {
  return workbenchSnapshot.value?.stages?.length ? workbenchSnapshot.value.stages : createDefaultWorkbenchStages()
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

const workbenchSubtitleSourceLabel = computed(() => {
  if (workbenchOptions.value.subtitleSource === 'downloaded') {
    const subtitle = selectedWorkbenchSubtitle.value
    return subtitle ? subtitleSubtitleLabel(subtitle) : '已下载字幕'
  }
  return '自动转录'
})

const workbenchSubtitleSourceSubtitle = computed(() => {
  if (workbenchOptions.value.subtitleSource === 'downloaded') {
    return selectedWorkbenchSubtitle.value?.filePath || '使用任务中已下载的字幕'
  }
  return '未选择字幕时从视频自动转录'
})

const selectedWorkbenchSubtitle = computed(() => {
  const task = activeTask.value
  if (!task) {
    return null
  }
  return task.downloadedSubtitles.find((subtitle) => subtitle.id === workbenchOptions.value.subtitleId) ?? null
})

const workbenchTranscriptionModelLabel = computed(() =>
  getOptionLabel(transcriptionModelOptions, workbenchOptions.value.transcriptionModel),
)
const workbenchSourceLanguageLabel = computed(() => getLanguageLabel(workbenchOptions.value.sourceLanguage))
const workbenchTranscriptionFormatLabel = computed(() =>
  getOptionLabel(subtitleFormatOptions, workbenchOptions.value.transcriptionFormat),
)
const workbenchTranslationServiceLabel = computed(() =>
  getOptionLabel(translationServiceOptions, workbenchOptions.value.translationService),
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
const workbenchCustomReferenceAudioLabel = computed(() => {
  if (workbenchOptions.value.dubbingReferenceAudioSource !== ReferenceAudioSource.CustomAudioFile) {
    return '参考音频来源为自定义时使用'
  }
  return workbenchOptions.value.dubbingCustomReferenceAudioPath
    ? fileNameFromPath(workbenchOptions.value.dubbingCustomReferenceAudioPath)
    : '未选择音频'
})
const workbenchExportDirLabel = computed(() => workbenchOptions.value.exportDir || '使用设置中的默认导出目录')

const exportedArtifacts = computed(() => {
  return (workbenchSnapshot.value?.artifacts ?? []).filter((artifact) =>
    ['exported-video', 'exported-subtitle'].includes(artifact.kind),
  )
})

const isWorkbenchLanguageDialog = computed(() => {
  return (
    activeWorkbenchDialog.value === WorkbenchDialog.SourceLanguage ||
    activeWorkbenchDialog.value === WorkbenchDialog.TargetLanguage
  )
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

const setDownloadProgress = (payload: HomeVideoDownloadProgress) => {
  const next = new Map(downloadProgressByKey.value)
  next.set(downloadProgressKey(payload.taskId, payload.kind, payload.key), {
    ...payload,
    progress: clampProgress(payload.progress),
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
  })
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
  })
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

const toggleWorkbenchTranslation = () => {
  updateWorkbenchOptions({
    translationEnabled: !workbenchOptions.value.translationEnabled,
    dubbingEnabled: workbenchOptions.value.translationEnabled ? false : workbenchOptions.value.dubbingEnabled,
  })
}

const toggleWorkbenchDubbing = () => {
  if (!workbenchOptions.value.translationEnabled) {
    return
  }
  updateWorkbenchOptions({ dubbingEnabled: !workbenchOptions.value.dubbingEnabled })
}

const updateWorkbenchTtsInterval = (event: Event) => {
  const value = Number((event.target as HTMLInputElement | null)?.value)
  updateWorkbenchOptions({ dubbingTtsIntervalMs: Number.isFinite(value) ? value : workbenchOptions.value.dubbingTtsIntervalMs })
}

const updateWorkbenchBatchSize = (event: Event) => {
  const value = Number((event.target as HTMLInputElement | null)?.value)
  updateWorkbenchOptions({ translationBatchSize: Number.isFinite(value) ? value : workbenchOptions.value.translationBatchSize })
}

const updateWorkbenchThreadCount = (event: Event) => {
  const value = Number((event.target as HTMLInputElement | null)?.value)
  updateWorkbenchOptions({ translationThreadCount: Number.isFinite(value) ? value : workbenchOptions.value.translationThreadCount })
}

const updateWorkbenchBackgroundMusicVolume = (event: Event) => {
  const value = Number((event.target as HTMLInputElement | null)?.value)
  updateWorkbenchOptions({
    dubbingBackgroundMusicVolume: Number.isFinite(value) ? value : workbenchOptions.value.dubbingBackgroundMusicVolume,
  })
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

const selectWorkbenchCustomReferenceAudio = async () => {
  if (!isTauriRuntime() || isWorkbenchRunning.value) {
    return
  }

  const selected = await open({
    title: '选择工作台参考音频',
    multiple: false,
    filters: [
      {
        name: '音频文件',
        extensions: referenceAudioExtensions,
      },
    ],
  })

  if (typeof selected === 'string') {
    updateWorkbenchOptions({
      dubbingReferenceAudioSource: ReferenceAudioSource.CustomAudioFile,
      dubbingCustomReferenceAudioPath: selected,
    })
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
  if (event.key === 'Escape') {
    closeAddDialog()
    closeDeleteDialog()
    closeWorkbenchDialog()
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

const createDefaultWorkbenchStages = (): HomeWorkbenchStage[] => [
  { key: 'download-video', label: '下载视频', progress: 0, status: 'pending', message: '等待下载视频' },
  { key: 'prepare-subtitle', label: '准备字幕', progress: 0, status: 'pending', message: '等待准备字幕' },
  { key: 'translation', label: '翻译与优化', progress: 0, status: 'pending', message: '等待翻译与优化' },
  { key: 'dubbing', label: '配音', progress: 0, status: 'pending', message: '等待配音' },
  { key: 'export', label: '导出', progress: 0, status: 'pending', message: '等待导出' },
]

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

const subtitleSubtitleLabel = (subtitle: HomeVideoSubtitle) => {
  const name = subtitle.languageName || subtitle.language || '字幕'
  return `${name} · ${subtitleSourceLabel(subtitle.sourceKind)}`
}

const fileNameFromPath = (path: string) => {
  const normalized = path.replace(/\\/g, '/')
  return normalized.split('/').filter(Boolean).pop() ?? path
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
