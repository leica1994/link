<template>
  <div class="page youtube-monitor-page">
    <header class="translate-header youtube-monitor-header">
      <div class="youtube-monitor-title-block">
        <button
          v-if="isDetailView || isUnreadView"
          class="youtube-monitor-back"
          type="button"
          aria-label="返回监控列表"
          @click="goBackToList"
        >
          <ArrowLeft :stroke-width="2.2" aria-hidden="true" />
        </button>
        <div>
          <h1 class="page-title">{{ pageTitle }}</h1>
          <div class="youtube-monitor-status-line" :class="toolStatusClass">
            <span class="translate-status-dot" :class="toolStatusDotClass" aria-hidden="true" />
            <span :title="ytdlpStatus.resolvedPath">{{ ytdlpStatus.message }}</span>
          </div>
        </div>
      </div>

      <div v-if="isListView" class="translate-actions youtube-monitor-header-actions">
        <button
          class="settings-action youtube-monitor-action"
          type="button"
          :disabled="isPageRefreshing"
          @click="refreshPage"
        >
          <LoaderCircle v-if="isPageRefreshing" class="spinning" :stroke-width="2.1" aria-hidden="true" />
          <RefreshCw v-else :stroke-width="2.1" aria-hidden="true" />
          <span>{{ isPageRefreshing ? '检查中' : '刷新页面' }}</span>
        </button>
        <button
          class="settings-action youtube-monitor-action primary"
          type="button"
          @click="openAddDialog"
        >
          <Plus :stroke-width="2.1" aria-hidden="true" />
          <span>添加博主</span>
        </button>
      </div>
    </header>

    <main class="youtube-monitor-workspace">
      <section v-if="isListView" class="youtube-monitor-list-view">
        <div v-if="channelError" class="translate-alert" role="alert">
          <CircleAlert :stroke-width="2.1" aria-hidden="true" />
          <span>{{ channelError }}</span>
        </div>

        <section class="settings-section" aria-labelledby="youtube-monitor-overview-title">
          <div id="youtube-monitor-overview-title" class="section-heading">
            <Radar aria-hidden="true" />
            <span>监控概览</span>
          </div>

          <div class="settings-panel youtube-monitor-summary">
            <div class="youtube-monitor-summary-item">
              <span class="youtube-monitor-summary-value">{{ channels.length }}</span>
              <span class="youtube-monitor-summary-label">博主</span>
            </div>
            <div class="youtube-monitor-summary-item">
              <RouterLink
                v-if="totalUnreadCount > 0"
                class="youtube-monitor-summary-link"
                :to="{ name: 'YoutubeMonitorUnread' }"
                aria-label="查看全部未读更新"
              >
                <span class="youtube-monitor-summary-value">{{ totalUnreadCount }}</span>
                <span class="youtube-monitor-summary-label">未读更新</span>
              </RouterLink>
              <template v-else>
                <span class="youtube-monitor-summary-value">{{ totalUnreadCount }}</span>
                <span class="youtube-monitor-summary-label">未读更新</span>
              </template>
            </div>
            <div class="youtube-monitor-summary-item">
              <span class="youtube-monitor-summary-value">{{ totalVideoCount }}</span>
              <span class="youtube-monitor-summary-label">视频</span>
            </div>
            <div class="youtube-monitor-summary-item">
              <span class="youtube-monitor-summary-value">{{ checkingChannelCount }}</span>
              <span class="youtube-monitor-summary-label">检查中</span>
            </div>
          </div>
        </section>

        <section class="settings-section youtube-monitor-channel-section" aria-labelledby="youtube-monitor-list-title">
          <div id="youtube-monitor-list-title" class="section-heading">
            <ListVideo aria-hidden="true" />
            <span>博主列表</span>
          </div>

          <div class="settings-panel youtube-monitor-panel">
            <div class="youtube-monitor-toolbar">
              <label class="youtube-monitor-search">
                <Search :stroke-width="2.1" aria-hidden="true" />
                <input
                  v-model="channelQuery"
                  type="search"
                  placeholder="搜索博主或地址"
                  aria-label="搜索博主或地址"
                />
              </label>

              <div class="youtube-monitor-filter" role="group" aria-label="监控状态">
                <button
                  v-for="option in channelFilterOptions"
                  :key="option.value"
                  class="youtube-monitor-filter-button"
                  :class="{ active: channelFilter === option.value }"
                  type="button"
                  :aria-pressed="channelFilter === option.value"
                  @click="channelFilter = option.value"
                >
                  {{ option.label }}
                </button>
              </div>
            </div>

            <div v-if="isLoadingChannels" class="youtube-monitor-empty">
              <LoaderCircle class="youtube-monitor-empty-icon spinning" :stroke-width="2.1" aria-hidden="true" />
              <span class="translate-empty-title">正在读取监控列表</span>
            </div>

            <div v-else-if="channels.length === 0" class="youtube-monitor-empty">
              <Radar class="youtube-monitor-empty-icon" :stroke-width="2.1" aria-hidden="true" />
              <span class="translate-empty-title">暂无监控博主</span>
              <button class="settings-action youtube-monitor-action primary" type="button" @click="openAddDialog">
                <Plus :stroke-width="2.1" aria-hidden="true" />
                <span>添加博主</span>
              </button>
            </div>

            <div v-else-if="filteredChannels.length === 0" class="youtube-monitor-empty">
              <Search class="youtube-monitor-empty-icon" :stroke-width="2.1" aria-hidden="true" />
              <span class="translate-empty-title">没有匹配的博主</span>
            </div>

            <div v-else class="youtube-channel-list">
              <RouterLink
                v-for="channel in filteredChannels"
                :key="channel.id"
                class="youtube-channel-row"
                :to="{ name: 'YoutubeMonitorDetail', params: { channelId: channel.id } }"
              >
                <span class="youtube-channel-avatar" aria-hidden="true">
                  {{ channelInitial(channel) }}
                </span>

                <span class="youtube-channel-copy">
                  <span class="youtube-channel-title-line">
                    <span class="youtube-channel-title">{{ channel.title }}</span>
                    <span class="youtube-status-pill" :class="channelStatusClass(channel)">
                      {{ channelStatusLabel(channel) }}
                    </span>
                  </span>
                  <span class="youtube-channel-url">{{ channel.handle || channel.canonicalUrl || channel.url }}</span>
                </span>

                <span class="youtube-channel-latest">
                  <span class="youtube-channel-meta-label">最新视频</span>
                  <span class="youtube-channel-latest-title">{{ channel.latestVideoTitle || '等待检查' }}</span>
                </span>

                <span class="youtube-channel-stats">
                  <span class="youtube-channel-stat strong">{{ channel.unreadCount }}</span>
                  <span class="youtube-channel-stat-label">未读</span>
                  <span class="youtube-channel-stat">{{ channel.videoCount }}</span>
                  <span class="youtube-channel-stat-label">视频</span>
                </span>

                <span class="youtube-channel-time">
                  <span class="youtube-channel-meta-label">上次检查</span>
                  <span>{{ formatDateTime(channel.lastCheckedAt) }}</span>
                </span>

                <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
              </RouterLink>
            </div>
          </div>
        </section>
      </section>

      <section v-else-if="isUnreadView" class="youtube-monitor-unread-view">
        <div v-if="channelError" class="translate-alert" role="alert">
          <CircleAlert :stroke-width="2.1" aria-hidden="true" />
          <span>{{ channelError }}</span>
        </div>

        <section class="settings-section" aria-labelledby="youtube-monitor-unread-title">
          <div id="youtube-monitor-unread-title" class="section-heading">
            <ListVideo aria-hidden="true" />
            <span>未读视频</span>
          </div>

          <div class="settings-panel youtube-monitor-panel">
            <div class="youtube-monitor-toolbar">
              <label class="youtube-monitor-search">
                <Search :stroke-width="2.1" aria-hidden="true" />
                <input
                  v-model="unreadQuery"
                  type="search"
                  placeholder="搜索视频、博主或地址"
                  aria-label="搜索未读更新"
                />
              </label>

              <div class="youtube-unread-toolbar-actions">
                <span class="youtube-unread-total">未读 {{ unreadVideoTotal }} 条</span>
                <button
                  v-if="totalUnreadCount > 0"
                  class="settings-action youtube-monitor-action"
                  type="button"
                  :disabled="!canMarkAllUnreadSeen"
                  @click="markAllChannelsSeen"
                >
                  <LoaderCircle v-if="isMarkingAllSeen" class="spinning" :stroke-width="2.1" aria-hidden="true" />
                  <CheckCheck v-else :stroke-width="2.1" aria-hidden="true" />
                  <span>{{ isMarkingAllSeen ? '标记中' : '全部已读' }}</span>
                </button>
              </div>
            </div>

            <div v-if="unreadError" class="translate-alert" role="alert">
              <CircleAlert :stroke-width="2.1" aria-hidden="true" />
              <span>{{ unreadError }}</span>
            </div>

            <div v-if="taskAddError" class="translate-alert" role="alert">
              <CircleAlert :stroke-width="2.1" aria-hidden="true" />
              <span>{{ taskAddError }}</span>
            </div>

            <div v-if="isLoadingUnreadVideos && unreadVideos.length === 0" class="youtube-monitor-empty">
              <LoaderCircle class="youtube-monitor-empty-icon spinning" :stroke-width="2.1" aria-hidden="true" />
              <span class="translate-empty-title">正在读取未读更新</span>
            </div>

            <div v-else-if="unreadVideos.length === 0" class="youtube-monitor-empty">
              <Video class="youtube-monitor-empty-icon" :stroke-width="2.1" aria-hidden="true" />
              <span class="translate-empty-title">{{ emptyUnreadText }}</span>
            </div>

            <div v-else class="youtube-video-list youtube-unread-video-list" role="table" aria-label="未读更新视频列表">
              <article v-for="item in unreadVideos" :key="item.video.id" class="youtube-unread-video-row" role="row">
                <span class="youtube-video-status unread" role="cell">新</span>

                <span class="youtube-video-main" role="cell">
                  <span class="youtube-video-title">{{ item.video.title }}</span>
                  <a class="youtube-video-url" :href="item.video.url" target="_blank" rel="noreferrer">{{ item.video.url }}</a>
                </span>

                <RouterLink
                  class="youtube-unread-channel"
                  :to="{ name: 'YoutubeMonitorDetail', params: { channelId: item.channel.id } }"
                  role="cell"
                >
                  <span class="youtube-channel-avatar" aria-hidden="true">{{ channelInitial(item.channel) }}</span>
                  <span class="youtube-unread-channel-copy">
                    <span class="youtube-channel-title-line">
                      <span class="youtube-channel-title">{{ item.channel.title }}</span>
                      <span class="youtube-status-pill" :class="channelStatusClass(item.channel)">
                        {{ channelStatusLabel(item.channel) }}
                      </span>
                    </span>
                    <span class="youtube-channel-url">{{ item.channel.handle || item.channel.canonicalUrl || item.channel.url }}</span>
                  </span>
                </RouterLink>

                <span class="youtube-video-meta" role="cell">
                  <Clock :stroke-width="2.1" aria-hidden="true" />
                  {{ formatDuration(item.video.duration) }}
                </span>

                <span class="youtube-video-meta" role="cell">
                  {{ formatDateTime(item.video.firstSeenAt) }}
                </span>

                <button
                  class="youtube-video-task-button"
                  type="button"
                  :disabled="isVideoQueued(item.video) || isAddingVideoTask(item.video)"
                  :aria-label="isVideoQueued(item.video) ? '已加入主页待办' : '加入主页待办'"
                  @click="addVideoTask(item.video)"
                >
                  <LoaderCircle
                    v-if="isAddingVideoTask(item.video)"
                    class="spinning"
                    :stroke-width="2.1"
                    aria-hidden="true"
                  />
                  <Check v-else-if="isVideoQueued(item.video)" :stroke-width="2.1" aria-hidden="true" />
                  <ListPlus v-else :stroke-width="2.1" aria-hidden="true" />
                  <span>{{ isVideoQueued(item.video) ? '已加入' : '加入待办' }}</span>
                </button>

                <a class="youtube-video-open" :href="item.video.url" target="_blank" rel="noreferrer" aria-label="打开视频">
                  <ExternalLink :stroke-width="2.1" aria-hidden="true" />
                </a>
              </article>
            </div>

            <div v-if="unreadVideos.length > 0" class="youtube-monitor-loadbar">
              <span>已显示 {{ unreadVideos.length }} / {{ unreadVideoTotal }}</span>
              <button
                class="settings-action youtube-monitor-action"
                type="button"
                :disabled="!hasMoreUnreadVideos || isLoadingUnreadVideos"
                @click="loadMoreUnreadVideos"
              >
                <LoaderCircle v-if="isLoadingUnreadVideos" class="spinning" :stroke-width="2.1" aria-hidden="true" />
                <span>{{ hasMoreUnreadVideos ? '加载更多' : '已全部显示' }}</span>
              </button>
            </div>
          </div>
        </section>
      </section>

      <section v-else class="youtube-monitor-detail-view">
        <div v-if="isLoadingChannels && !activeChannel" class="youtube-monitor-empty">
          <LoaderCircle class="youtube-monitor-empty-icon spinning" :stroke-width="2.1" aria-hidden="true" />
          <span class="translate-empty-title">正在读取博主信息</span>
        </div>

        <div v-else-if="!activeChannel" class="youtube-monitor-empty">
          <CircleAlert class="youtube-monitor-empty-icon" :stroke-width="2.1" aria-hidden="true" />
          <span class="translate-empty-title">未找到该监控博主</span>
          <button class="settings-action youtube-monitor-action" type="button" @click="goBackToList">
            返回列表
          </button>
        </div>

        <template v-else>
          <section class="settings-section" aria-labelledby="youtube-monitor-channel-title">
            <div id="youtube-monitor-channel-title" class="section-heading">
              <Youtube aria-hidden="true" />
              <span>博主信息</span>
            </div>

            <div class="settings-panel youtube-channel-detail-panel">
              <div class="youtube-channel-detail-head">
                <span class="youtube-channel-avatar large" aria-hidden="true">{{ channelInitial(activeChannel) }}</span>
                <div class="youtube-channel-detail-copy">
                  <div class="youtube-channel-title-line">
                    <span class="youtube-channel-detail-title">{{ activeChannel.title }}</span>
                    <span class="youtube-status-pill" :class="channelStatusClass(activeChannel)">
                      {{ channelStatusLabel(activeChannel) }}
                    </span>
                  </div>
                  <a
                    class="youtube-channel-detail-url"
                    :href="activeChannel.canonicalUrl || activeChannel.url"
                    target="_blank"
                    rel="noreferrer"
                  >
                    {{ activeChannel.canonicalUrl || activeChannel.url }}
                  </a>
                </div>

                <div class="youtube-channel-detail-actions">
                  <button
                    class="settings-action youtube-monitor-action primary"
                    type="button"
                    :disabled="isChannelRefreshing(activeChannel.id) || !ytdlpStatus.isAvailable"
                    @click="refreshChannel(activeChannel.id)"
                  >
                    <LoaderCircle
                      v-if="isChannelRefreshing(activeChannel.id)"
                      class="spinning"
                      :stroke-width="2.1"
                      aria-hidden="true"
                    />
                    <RefreshCw v-else :stroke-width="2.1" aria-hidden="true" />
                    <span>{{ isChannelRefreshing(activeChannel.id) ? '检查中' : '检查更新' }}</span>
                  </button>
                  <button class="settings-action youtube-monitor-action" type="button" @click="markChannelSeen">
                    <CheckCheck :stroke-width="2.1" aria-hidden="true" />
                    <span>全部已读</span>
                  </button>
                  <button class="settings-action youtube-monitor-action danger" type="button" @click="openDeleteDialog">
                    <Trash2 :stroke-width="2.1" aria-hidden="true" />
                    <span>删除</span>
                  </button>
                </div>
              </div>

              <div class="youtube-channel-detail-stats">
                <div class="youtube-detail-stat">
                  <span class="youtube-monitor-summary-value">{{ activeChannel.videoCount }}</span>
                  <span class="youtube-monitor-summary-label">视频</span>
                </div>
                <div class="youtube-detail-stat">
                  <span class="youtube-monitor-summary-value">{{ activeChannel.unreadCount }}</span>
                  <span class="youtube-monitor-summary-label">未读更新</span>
                </div>
                <div class="youtube-detail-stat">
                  <span class="youtube-monitor-summary-value compact">{{ formatDateTime(activeChannel.lastSuccessAt) }}</span>
                  <span class="youtube-monitor-summary-label">最后成功</span>
                </div>
                <div class="youtube-detail-stat">
                  <span class="youtube-monitor-summary-value compact">{{ formatDateTime(activeChannel.lastCheckedAt) }}</span>
                  <span class="youtube-monitor-summary-label">最后检查</span>
                </div>
              </div>

              <div v-if="activeRefreshRun && activeRefreshRun.channelId === activeChannel.id" class="youtube-refresh-strip">
                <span class="translate-status-dot" :class="refreshRunDotClass(activeRefreshRun)" aria-hidden="true" />
                <span>{{ activeRefreshRun.message }}</span>
                <span>新增 {{ activeRefreshRun.insertedCount }} · 更新 {{ activeRefreshRun.updatedCount }}</span>
              </div>

              <div v-if="activeChannel.lastError" class="translate-alert" role="alert">
                <CircleAlert :stroke-width="2.1" aria-hidden="true" />
                <span>{{ activeChannel.lastError }}</span>
              </div>
            </div>
          </section>

          <section class="settings-section youtube-monitor-video-section" aria-labelledby="youtube-monitor-videos-title">
            <div id="youtube-monitor-videos-title" class="section-heading">
              <ListVideo aria-hidden="true" />
              <span>视频列表</span>
            </div>

            <div class="settings-panel youtube-monitor-panel">
              <div class="youtube-monitor-toolbar">
                <label class="youtube-monitor-search">
                  <Search :stroke-width="2.1" aria-hidden="true" />
                  <input
                    v-model="videoQuery"
                    type="search"
                    placeholder="搜索视频标题或地址"
                    aria-label="搜索视频标题或地址"
                  />
                </label>

                <button
                  class="youtube-monitor-filter-button"
                  :class="{ active: unreadOnly }"
                  type="button"
                  :aria-pressed="unreadOnly"
                  @click="unreadOnly = !unreadOnly"
                >
                  仅未读
                </button>
              </div>

              <div v-if="videosError" class="translate-alert" role="alert">
                <CircleAlert :stroke-width="2.1" aria-hidden="true" />
                <span>{{ videosError }}</span>
              </div>

              <div v-if="taskAddError" class="translate-alert" role="alert">
                <CircleAlert :stroke-width="2.1" aria-hidden="true" />
                <span>{{ taskAddError }}</span>
              </div>

              <div v-if="isLoadingVideos && videos.length === 0" class="youtube-monitor-empty">
                <LoaderCircle class="youtube-monitor-empty-icon spinning" :stroke-width="2.1" aria-hidden="true" />
                <span class="translate-empty-title">正在读取视频</span>
              </div>

              <div v-else-if="videos.length === 0" class="youtube-monitor-empty">
                <Video class="youtube-monitor-empty-icon" :stroke-width="2.1" aria-hidden="true" />
                <span class="translate-empty-title">{{ emptyVideoText }}</span>
              </div>

              <div v-else class="youtube-video-list" role="table" aria-label="监控视频列表">
                <article v-for="video in videos" :key="video.id" class="youtube-video-row" role="row">
                  <span class="youtube-video-status" :class="{ unread: video.isUnread }" role="cell">
                    {{ video.isUnread ? '新' : '已读' }}
                  </span>
                  <span class="youtube-video-main" role="cell">
                    <span class="youtube-video-title">{{ video.title }}</span>
                    <a class="youtube-video-url" :href="video.url" target="_blank" rel="noreferrer">{{ video.url }}</a>
                  </span>
                  <span class="youtube-video-meta" role="cell">
                    <Clock :stroke-width="2.1" aria-hidden="true" />
                    {{ formatDuration(video.duration) }}
                  </span>
                  <button
                    class="youtube-video-task-button"
                    type="button"
                    :disabled="isVideoQueued(video) || isAddingVideoTask(video)"
                    :aria-label="isVideoQueued(video) ? '已加入主页待办' : '加入主页待办'"
                    @click="addVideoTask(video)"
                  >
                    <LoaderCircle
                      v-if="isAddingVideoTask(video)"
                      class="spinning"
                      :stroke-width="2.1"
                      aria-hidden="true"
                    />
                    <Check v-else-if="isVideoQueued(video)" :stroke-width="2.1" aria-hidden="true" />
                    <ListPlus v-else :stroke-width="2.1" aria-hidden="true" />
                    <span>{{ isVideoQueued(video) ? '已加入' : '加入待办' }}</span>
                  </button>
                  <a class="youtube-video-open" :href="video.url" target="_blank" rel="noreferrer" aria-label="打开视频">
                    <ExternalLink :stroke-width="2.1" aria-hidden="true" />
                  </a>
                </article>
              </div>

              <div v-if="videos.length > 0" class="youtube-monitor-loadbar">
                <span>已显示 {{ videos.length }} / {{ videoTotal }}</span>
                <button
                  class="settings-action youtube-monitor-action"
                  type="button"
                  :disabled="!hasMoreVideos || isLoadingVideos"
                  @click="loadMoreVideos"
                >
                  <LoaderCircle v-if="isLoadingVideos" class="spinning" :stroke-width="2.1" aria-hidden="true" />
                  <span>{{ hasMoreVideos ? '加载更多' : '已全部显示' }}</span>
                </button>
              </div>
            </div>
          </section>
        </template>
      </section>
    </main>

    <Teleport to="body">
      <div v-if="isAddDialogOpen" class="dialog-backdrop" role="presentation" @click.self="closeAddDialog">
        <section class="settings-dialog youtube-monitor-dialog" role="dialog" aria-modal="true" aria-labelledby="add-youtube-channel-title">
          <h2 id="add-youtube-channel-title" class="dialog-title">添加博主</h2>
          <div class="youtube-dialog-field">
            <label class="youtube-dialog-label" for="youtube-channel-url">频道地址</label>
            <input
              id="youtube-channel-url"
              v-model="draftChannelUrl"
              class="settings-input youtube-dialog-input"
              type="text"
              placeholder="https://www.youtube.com/@handle"
              autocomplete="off"
              autocapitalize="off"
              autocorrect="off"
              spellcheck="false"
              aria-describedby="youtube-channel-url-hint"
              @keydown.enter.prevent="addChannel"
            />
            <span id="youtube-channel-url-hint" class="youtube-dialog-hint">
              支持 youtube.com/@handle、/channel/、/c/、/user/
            </span>
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
              :disabled="isAddingChannel || !draftChannelUrl.trim()"
              @click="addChannel"
            >
              <LoaderCircle v-if="isAddingChannel" class="spinning" :stroke-width="2.1" aria-hidden="true" />
              <span>{{ isAddingChannel ? '添加中' : '添加' }}</span>
            </button>
          </div>
        </section>
      </div>

      <div v-if="isDeleteDialogOpen" class="dialog-backdrop" role="presentation" @click.self="closeDeleteDialog">
        <section class="settings-dialog youtube-monitor-dialog" role="dialog" aria-modal="true" aria-labelledby="delete-youtube-channel-title">
          <h2 id="delete-youtube-channel-title" class="dialog-title">删除博主</h2>
          <p class="youtube-dialog-copy">删除后会移除该博主和已保存的视频记录。</p>
          <div v-if="deleteError" class="translate-alert" role="alert">
            <CircleAlert :stroke-width="2.1" aria-hidden="true" />
            <span>{{ deleteError }}</span>
          </div>
          <div class="youtube-dialog-actions">
            <button class="settings-action youtube-monitor-action" type="button" @click="closeDeleteDialog">取消</button>
            <button
              class="settings-action youtube-monitor-action danger"
              type="button"
              :disabled="isDeletingChannel"
              @click="deleteChannel"
            >
              <LoaderCircle v-if="isDeletingChannel" class="spinning" :stroke-width="2.1" aria-hidden="true" />
              <span>{{ isDeletingChannel ? '删除中' : '删除' }}</span>
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
import {
  ArrowLeft,
  Check,
  CheckCheck,
  ChevronRight,
  CircleAlert,
  Clock,
  ExternalLink,
  ListPlus,
  ListVideo,
  LoaderCircle,
  Plus,
  Radar,
  RefreshCw,
  Search,
  Trash2,
  Video,
  Youtube,
} from 'lucide-vue-next'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'

defineOptions({ name: 'YoutubeMonitor' })

type ChannelStatusFilter = 'all' | 'updated' | 'checking' | 'failed' | 'idle'

type YtdlpStatus = {
  isAvailable: boolean
  version: string
  message: string
  resolvedPath: string
  configPolicy: 'ignoreConfig'
}

type YoutubeChannel = {
  id: string
  url: string
  canonicalUrl: string
  externalId: string
  title: string
  handle: string
  description: string
  thumbnailUrl: string
  status: string
  lastCheckedAt?: string | null
  lastSuccessAt?: string | null
  lastError: string
  videoCount: number
  unreadCount: number
  createdAt: string
  updatedAt: string
  latestVideoTitle: string
  latestVideoUrl: string
  latestVideoSeenAt?: string | null
}

type YoutubeVideo = {
  id: string
  channelId: string
  externalId: string
  title: string
  url: string
  duration?: number | null
  isUnread: boolean
  firstSeenAt: string
  lastSeenAt: string
  metadata: Record<string, unknown>
}

type YoutubeChannelLike = {
  id: string
  title: string
  handle: string
  url: string
  canonicalUrl: string
  status: string
  unreadCount: number
}

type HomeVideoTask = {
  id: string
  url: string
  sourceChannelId: string
  sourceVideoId: string
  title: string
}

type YoutubeVideoPage = {
  items: YoutubeVideo[]
  total: number
  hasMore: boolean
}

type YoutubeUnreadVideoChannel = {
  id: string
  title: string
  handle: string
  url: string
  canonicalUrl: string
  status: string
  unreadCount: number
  videoCount: number
  lastCheckedAt?: string | null
}

type YoutubeUnreadVideoItem = {
  video: YoutubeVideo
  channel: YoutubeUnreadVideoChannel
}

type YoutubeUnreadVideoPage = {
  items: YoutubeUnreadVideoItem[]
  total: number
  hasMore: boolean
}

type YoutubeRefreshRun = {
  id: string
  channelId: string
  status: string
  processedCount: number
  insertedCount: number
  updatedCount: number
  message: string
  errorMessage: string
  startedAt: string
  finishedAt?: string | null
}

const VIDEO_PAGE_SIZE = 100

const route = useRoute()
const router = useRouter()

const channels = ref<YoutubeChannel[]>([])
const videos = ref<YoutubeVideo[]>([])
const unreadVideos = ref<YoutubeUnreadVideoItem[]>([])
const videoTotal = ref(0)
const unreadVideoTotal = ref(0)
const hasMoreVideos = ref(false)
const hasMoreUnreadVideos = ref(false)
const ytdlpStatus = ref<YtdlpStatus>({
  isAvailable: false,
  version: '',
  message: '正在检测 yt-dlp',
  resolvedPath: '',
  configPolicy: 'ignoreConfig',
})
const channelQuery = ref('')
const channelFilter = ref<ChannelStatusFilter>('all')
const videoQuery = ref('')
const unreadQuery = ref('')
const unreadOnly = ref(false)
const isLoadingChannels = ref(false)
const isLoadingVideos = ref(false)
const isLoadingUnreadVideos = ref(false)
const isPageRefreshing = ref(false)
const isMarkingAllSeen = ref(false)
const isAddDialogOpen = ref(false)
const isDeleteDialogOpen = ref(false)
const draftChannelUrl = ref('')
const addError = ref('')
const deleteError = ref('')
const channelError = ref('')
const videosError = ref('')
const unreadError = ref('')
const taskAddError = ref('')
const isAddingChannel = ref(false)
const isDeletingChannel = ref(false)
const activeRefreshRun = ref<YoutubeRefreshRun | null>(null)
const refreshingChannelIds = ref(new Set<string>())
const queuedVideoUrls = ref(new Set<string>())
const addingVideoIds = ref(new Set<string>())
const refreshWaiters = new Map<string, Array<(run: YoutubeRefreshRun) => void>>()
let unlistenRefresh: UnlistenFn | undefined

const channelFilterOptions: { value: ChannelStatusFilter; label: string }[] = [
  { value: 'all', label: '全部' },
  { value: 'updated', label: '有更新' },
  { value: 'checking', label: '检查中' },
  { value: 'failed', label: '异常' },
  { value: 'idle', label: '已同步' },
]

const isTauriRuntime = () => '__TAURI_INTERNALS__' in window

const activeChannelId = computed(() => {
  const value = route.params.channelId
  return typeof value === 'string' ? value : ''
})

const isUnreadView = computed(() => route.name === 'YoutubeMonitorUnread')
const isDetailView = computed(() => Boolean(activeChannelId.value))
const isListView = computed(() => !isUnreadView.value && !isDetailView.value)

const activeChannel = computed(() => {
  return channels.value.find((channel) => channel.id === activeChannelId.value) ?? null
})

const pageTitle = computed(() => {
  if (isUnreadView.value) {
    return '未读更新'
  }
  if (isListView.value) {
    return '监控'
  }

  return activeChannel.value?.title || '监控详情'
})

const filteredChannels = computed(() => {
  const query = channelQuery.value.trim().toLowerCase()
  return channels.value.filter((channel) => {
    const matchesQuery = !query ||
      channel.title.toLowerCase().includes(query) ||
      channel.handle.toLowerCase().includes(query) ||
      channel.url.toLowerCase().includes(query) ||
      channel.canonicalUrl.toLowerCase().includes(query)
    const matchesFilter =
      channelFilter.value === 'all' ||
      (channelFilter.value === 'updated' && channel.unreadCount > 0) ||
      (channelFilter.value === 'checking' && channel.status === 'checking') ||
      (channelFilter.value === 'failed' && channel.status === 'failed') ||
      (channelFilter.value === 'idle' && channel.status !== 'checking' && channel.status !== 'failed' && channel.unreadCount === 0)

    return matchesQuery && matchesFilter
  })
})

const totalUnreadCount = computed(() => channels.value.reduce((total, channel) => total + channel.unreadCount, 0))
const totalVideoCount = computed(() => channels.value.reduce((total, channel) => total + channel.videoCount, 0))
const checkingChannelCount = computed(() => {
  return channels.value.filter((channel) => channel.status === 'checking' || isChannelRefreshing(channel.id)).length
})
const canMarkAllUnreadSeen = computed(() => {
  return isTauriRuntime() &&
    totalUnreadCount.value > 0 &&
    !isLoadingChannels.value &&
    !isLoadingUnreadVideos.value &&
    !isPageRefreshing.value &&
    !isMarkingAllSeen.value
})

const toolStatusClass = computed(() => ({
  ready: ytdlpStatus.value.isAvailable,
  error: !ytdlpStatus.value.isAvailable,
}))

const toolStatusDotClass = computed(() => (ytdlpStatus.value.isAvailable ? 'success' : 'error'))

const emptyVideoText = computed(() => {
  if (videoQuery.value.trim() || unreadOnly.value) {
    return '没有匹配的视频'
  }

  return activeChannel.value?.lastCheckedAt ? '暂无视频记录' : '检查更新后会显示视频'
})

const emptyUnreadText = computed(() => {
  if (unreadQuery.value.trim()) {
    return '没有匹配的未读更新'
  }

  return '暂无未读更新'
})

watch(activeChannelId, () => {
  resetVideos()
  if (activeChannelId.value) {
    void loadVideos(true)
  }
})

watch(isUnreadView, () => {
  resetUnreadVideos()
  if (isUnreadView.value) {
    void loadUnreadVideos(true)
  }
})

watch([videoQuery, unreadOnly], () => {
  if (activeChannelId.value) {
    resetVideos()
    void loadVideos(true)
  }
})

watch(unreadQuery, () => {
  if (isUnreadView.value) {
    resetUnreadVideos()
    void loadUnreadVideos(true)
  }
})

const loadAll = async () => {
  await Promise.all([loadYtdlpStatus(), loadChannels(), loadQueuedVideoUrls()])
  if (activeChannelId.value) {
    resetVideos()
    await loadVideos(true)
  }
  if (isUnreadView.value) {
    resetUnreadVideos()
    await loadUnreadVideos(true)
  }
}

const refreshPage = async () => {
  if (!isTauriRuntime() || isPageRefreshing.value) {
    return
  }

  isPageRefreshing.value = true
  channelError.value = ''
  videosError.value = ''
  let failedCount = 0

  try {
    await loadYtdlpStatus()
    if (!ytdlpStatus.value.isAvailable) {
      return
    }

    const channelIds = channels.value.map((channel) => channel.id)
    for (const channelId of channelIds) {
      if (isChannelRefreshing(channelId)) {
        continue
      }

      try {
        const run = await refreshChannelAndWait(channelId, false)
        if (run.status === 'failed') {
          failedCount += 1
        }
      } catch {
        failedCount += 1
      }
    }
  } finally {
    await loadAll()
    if (failedCount > 0) {
      channelError.value = '部分博主检查失败，请查看异常状态'
    }
    isPageRefreshing.value = false
  }
}

const loadYtdlpStatus = async () => {
  if (!isTauriRuntime()) {
    ytdlpStatus.value = {
      isAvailable: false,
      version: '',
      message: '请在桌面应用中使用监控',
      resolvedPath: '',
      configPolicy: 'ignoreConfig',
    }
    return
  }

  ytdlpStatus.value = await invoke<YtdlpStatus>('get_ytdlp_status')
}

const loadChannels = async () => {
  channelError.value = ''

  if (!isTauriRuntime()) {
    channels.value = []
    channelError.value = '请在桌面应用中读取监控列表'
    return
  }

  isLoadingChannels.value = true
  try {
    channels.value = await invoke<YoutubeChannel[]>('list_youtube_channels')
  } catch (error) {
    channelError.value = stringifyError(error, '读取监控列表失败')
  } finally {
    isLoadingChannels.value = false
  }
}

const resetVideos = () => {
  videos.value = []
  videoTotal.value = 0
  hasMoreVideos.value = false
  videosError.value = ''
  taskAddError.value = ''
}

const resetUnreadVideos = () => {
  unreadVideos.value = []
  unreadVideoTotal.value = 0
  hasMoreUnreadVideos.value = false
  unreadError.value = ''
  taskAddError.value = ''
}

const loadVideos = async (replace = false) => {
  if (!activeChannelId.value || isLoadingVideos.value) {
    return
  }

  if (!isTauriRuntime()) {
    videosError.value = '请在桌面应用中读取视频列表'
    return
  }

  isLoadingVideos.value = true
  videosError.value = ''
  const offset = replace ? 0 : videos.value.length

  try {
    const page = await invoke<YoutubeVideoPage>('list_youtube_videos', {
      request: {
        channelId: activeChannelId.value,
        query: videoQuery.value,
        unreadOnly: unreadOnly.value,
        limit: VIDEO_PAGE_SIZE,
        offset,
      },
    })
    videos.value = replace ? page.items : [...videos.value, ...page.items]
    videoTotal.value = page.total
    hasMoreVideos.value = page.hasMore
  } catch (error) {
    videosError.value = stringifyError(error, '读取视频列表失败')
  } finally {
    isLoadingVideos.value = false
  }
}

const loadMoreVideos = async () => {
  await loadVideos(false)
}

const loadUnreadVideos = async (replace = false) => {
  if (!isUnreadView.value || isLoadingUnreadVideos.value) {
    return
  }

  if (!isTauriRuntime()) {
    unreadError.value = '请在桌面应用中读取未读更新'
    return
  }

  isLoadingUnreadVideos.value = true
  unreadError.value = ''
  const offset = replace ? 0 : unreadVideos.value.length

  try {
    const page = await invoke<YoutubeUnreadVideoPage>('list_youtube_unread_videos', {
      request: {
        query: unreadQuery.value,
        limit: VIDEO_PAGE_SIZE,
        offset,
      },
    })
    unreadVideos.value = replace ? page.items : [...unreadVideos.value, ...page.items]
    unreadVideoTotal.value = page.total
    hasMoreUnreadVideos.value = page.hasMore
  } catch (error) {
    unreadError.value = stringifyError(error, '读取未读更新失败')
  } finally {
    isLoadingUnreadVideos.value = false
  }
}

const loadMoreUnreadVideos = async () => {
  await loadUnreadVideos(false)
}

const refreshChannel = async (channelId: string) => {
  if (!isTauriRuntime() || isChannelRefreshing(channelId)) {
    return
  }

  videosError.value = ''

  try {
    await refreshChannelAndWait(channelId, true)
  } catch (error) {
    videosError.value = stringifyError(error, '检查更新失败')
    setChannelRefreshing(channelId, false)
  }
}

const refreshChannelAndWait = async (channelId: string, reloadAfterDone: boolean) => {
  setChannelRefreshing(channelId, true)
  activeRefreshRun.value = {
    id: '',
    channelId,
    status: 'running',
    processedCount: 0,
    insertedCount: 0,
    updatedCount: 0,
    message: '准备检查更新',
    errorMessage: '',
    startedAt: new Date().toISOString(),
    finishedAt: null,
  }

  try {
    const finishedRun = waitForChannelRefresh(channelId)
    const run = await invoke<YoutubeRefreshRun>('refresh_youtube_channel', {
      request: { channelId },
    })
    activeRefreshRun.value = run
    if (run.status === 'done' || run.status === 'failed') {
      setChannelRefreshing(channelId, false)
      resolveRefreshWaiters(run)
      if (reloadAfterDone) {
        await reloadAfterChannelRefresh(channelId)
      }
      return run
    }

    const completedRun = await finishedRun
    if (reloadAfterDone) {
      await reloadAfterChannelRefresh(channelId)
    }
    return completedRun
  } catch (error) {
    setChannelRefreshing(channelId, false)
    resolveRefreshWaiters({
      id: '',
      channelId,
      status: 'failed',
      processedCount: 0,
      insertedCount: 0,
      updatedCount: 0,
      message: '检查失败',
      errorMessage: stringifyError(error, '检查更新失败'),
      startedAt: new Date().toISOString(),
      finishedAt: new Date().toISOString(),
    })
    throw error
  }
}

const waitForChannelRefresh = (channelId: string) => {
  return new Promise<YoutubeRefreshRun>((resolve) => {
    const waiters = refreshWaiters.get(channelId) ?? []
    waiters.push(resolve)
    refreshWaiters.set(channelId, waiters)
  })
}

const resolveRefreshWaiters = (run: YoutubeRefreshRun) => {
  const waiters = refreshWaiters.get(run.channelId)
  if (!waiters) {
    return false
  }

  refreshWaiters.delete(run.channelId)
  waiters.forEach((resolve) => resolve(run))
  return true
}

const reloadAfterChannelRefresh = async (channelId: string) => {
  await loadChannels()
  if (activeChannelId.value === channelId) {
    resetVideos()
    await loadVideos(true)
  }
  if (isUnreadView.value) {
    resetUnreadVideos()
    await loadUnreadVideos(true)
  }
}

const markChannelSeen = async () => {
  if (!activeChannel.value || !isTauriRuntime()) {
    return
  }

  try {
    const updated = await invoke<YoutubeChannel>('mark_youtube_channel_seen', {
      request: { channelId: activeChannel.value.id },
    })
    updateChannel(updated)
    videos.value = videos.value.map((video) => ({ ...video, isUnread: false }))
  } catch (error) {
    videosError.value = stringifyError(error, '标记已读失败')
  }
}

const markAllChannelsSeen = async () => {
  if (!canMarkAllUnreadSeen.value) {
    return
  }

  isMarkingAllSeen.value = true
  unreadError.value = ''
  channelError.value = ''

  try {
    channels.value = await invoke<YoutubeChannel[]>('mark_all_youtube_channels_seen')
    resetUnreadVideos()
    if (isUnreadView.value) {
      await loadUnreadVideos(true)
    }
    if (activeChannelId.value) {
      videos.value = videos.value.map((video) => ({ ...video, isUnread: false }))
    }
  } catch (error) {
    const message = stringifyError(error, '标记全部已读失败')
    if (isUnreadView.value) {
      unreadError.value = message
    } else {
      channelError.value = message
    }
  } finally {
    isMarkingAllSeen.value = false
  }
}

const addVideoTask = async (video: YoutubeVideo) => {
  if (!isTauriRuntime() || isVideoQueued(video) || isAddingVideoTask(video)) {
    return
  }

  const next = new Set(addingVideoIds.value)
  next.add(video.id)
  addingVideoIds.value = next
  taskAddError.value = ''

  try {
    const task = await invoke<HomeVideoTask>('add_home_video_task', {
      request: {
        url: video.url,
        title: video.title,
        sourceChannelId: video.channelId,
        sourceVideoId: video.id,
      },
    })
    const queued = new Set(queuedVideoUrls.value)
    queued.add(task.url)
    queued.add(video.url)
    queuedVideoUrls.value = queued
  } catch (error) {
    taskAddError.value = stringifyError(error, '加入主页待办失败')
  } finally {
    const cleared = new Set(addingVideoIds.value)
    cleared.delete(video.id)
    addingVideoIds.value = cleared
  }
}

const isVideoQueued = (video: YoutubeVideo) => queuedVideoUrls.value.has(normalizeVideoQueueUrl(video.url))

const isAddingVideoTask = (video: YoutubeVideo) => addingVideoIds.value.has(video.id)

const normalizeVideoQueueUrl = (url: string) => {
  const videoId = youtubeVideoIdFromUrl(url)
  return videoId ? `https://www.youtube.com/watch?v=${videoId}` : url
}

const youtubeVideoIdFromUrl = (url: string) => {
  const trimmed = url.trim()
  const lower = trimmed.toLowerCase()
  const directMarkers = ['youtu.be/', 'youtube.com/shorts/', 'youtube.com/embed/']
  for (const marker of directMarkers) {
    const index = lower.indexOf(marker)
    if (index >= 0) {
      return cleanVideoId(trimmed.slice(index + marker.length))
    }
  }

  const queryIndex = trimmed.indexOf('?')
  if (queryIndex < 0 || !lower.includes('youtube.com/watch')) {
    return ''
  }

  const params = new URLSearchParams(trimmed.slice(queryIndex + 1))
  return cleanVideoId(params.get('v') ?? '')
}

const cleanVideoId = (value: string) => {
  const id = value.split(/[/?&#]/)[0]?.trim() ?? ''
  return id && !id.includes('.') ? id : ''
}

const addChannel = async () => {
  const url = draftChannelUrl.value.trim()
  if (!url || isAddingChannel.value || !isTauriRuntime()) {
    return
  }

  isAddingChannel.value = true
  addError.value = ''

  try {
    const channel = await invoke<YoutubeChannel>('add_youtube_channel', {
      request: { url },
    })
    closeAddDialog()
    await loadChannels()
    await router.push({ name: 'YoutubeMonitorDetail', params: { channelId: channel.id } })
  } catch (error) {
    addError.value = stringifyError(error, '添加博主失败')
  } finally {
    isAddingChannel.value = false
  }
}

const deleteChannel = async () => {
  if (!activeChannel.value || isDeletingChannel.value || !isTauriRuntime()) {
    return
  }

  isDeletingChannel.value = true
  deleteError.value = ''

  try {
    await invoke('delete_youtube_channel', {
      request: { channelId: activeChannel.value.id },
    })
    closeDeleteDialog()
    await router.push({ name: 'YoutubeMonitor' })
    await loadChannels()
  } catch (error) {
    deleteError.value = stringifyError(error, '删除博主失败')
  } finally {
    isDeletingChannel.value = false
  }
}

const registerRefreshListener = async () => {
  if (!isTauriRuntime()) {
    return
  }

  unlistenRefresh = await listen<YoutubeRefreshRun>('youtube-monitor-refresh', async (event) => {
    const run = event.payload
    activeRefreshRun.value = run

    if (run.status === 'done' || run.status === 'failed') {
      const hadWaiters = resolveRefreshWaiters(run)
      setChannelRefreshing(run.channelId, false)
      if (!hadWaiters && !isPageRefreshing.value) {
        await reloadAfterChannelRefresh(run.channelId)
      }
    } else {
      setChannelRefreshing(run.channelId, true)
    }
  })
}

const updateChannel = (updated: YoutubeChannel) => {
  channels.value = channels.value.map((channel) => (channel.id === updated.id ? updated : channel))
}

const setChannelRefreshing = (channelId: string, refreshing: boolean) => {
  const next = new Set(refreshingChannelIds.value)
  if (refreshing) {
    next.add(channelId)
  } else {
    next.delete(channelId)
  }
  refreshingChannelIds.value = next
}

const isChannelRefreshing = (channelId: string) => refreshingChannelIds.value.has(channelId)

const openAddDialog = () => {
  draftChannelUrl.value = ''
  addError.value = ''
  isAddDialogOpen.value = true
}

const closeAddDialog = () => {
  isAddDialogOpen.value = false
}

const openDeleteDialog = () => {
  deleteError.value = ''
  isDeleteDialogOpen.value = true
}

const closeDeleteDialog = () => {
  isDeleteDialogOpen.value = false
}

const goBackToList = async () => {
  await router.push({ name: 'YoutubeMonitor' })
}

const channelInitial = (channel: YoutubeChannelLike) => {
  const value = channel.title || channel.handle || '监'
  return value.trim().slice(0, 1).toUpperCase()
}

const channelStatusLabel = (channel: YoutubeChannelLike) => {
  if (channel.status === 'checking' || isChannelRefreshing(channel.id)) {
    return '检查中'
  }
  if (channel.status === 'failed') {
    return '异常'
  }
  if (channel.unreadCount > 0) {
    return '有更新'
  }
  return '已同步'
}

const channelStatusClass = (channel: YoutubeChannelLike) => ({
  updated: channel.unreadCount > 0 && channel.status !== 'checking',
  checking: channel.status === 'checking' || isChannelRefreshing(channel.id),
  failed: channel.status === 'failed',
})

const refreshRunDotClass = (run: YoutubeRefreshRun) => {
  if (run.status === 'done') {
    return 'success'
  }
  if (run.status === 'failed') {
    return 'error'
  }
  return 'active'
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

const formatDuration = (duration?: number | null) => {
  if (!duration || !Number.isFinite(duration)) {
    return '--'
  }

  const totalSeconds = Math.max(0, Math.round(duration))
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
  }

  return `${minutes}:${String(seconds).padStart(2, '0')}`
}

const loadQueuedVideoUrls = async () => {
  if (!isTauriRuntime()) {
    queuedVideoUrls.value = new Set()
    return
  }

  try {
    const tasks = await invoke<HomeVideoTask[]>('list_home_video_tasks')
    queuedVideoUrls.value = new Set(tasks.map((task) => task.url))
  } catch {
    queuedVideoUrls.value = new Set()
  }
}

const handleKeydown = (event: KeyboardEvent) => {
  if (event.key === 'Escape') {
    closeAddDialog()
    closeDeleteDialog()
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
  void loadAll()
  void registerRefreshListener()
  window.addEventListener('keydown', handleKeydown)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown)
  unlistenRefresh?.()
})
</script>
