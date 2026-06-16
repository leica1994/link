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
              <div class="setting-subtitle" :class="llmConnectionSubtitleClass">{{ llmConnectionSubtitle }}</div>
            </div>
            <button class="settings-action" type="button" :disabled="isCheckingLlmConnection" @click="checkLlmConnection">
              {{ isCheckingLlmConnection ? '检查中' : '检查连接' }}
            </button>
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

      <section class="settings-section" aria-labelledby="translation-optimization-settings-title">
        <div id="translation-optimization-settings-title" class="section-heading">
          <WandSparkles aria-hidden="true" />
          <span>翻译与优化</span>
        </div>

        <div class="settings-panel">
          <button class="setting-row setting-row-button" type="button" @click="openVideoContentTypeDialog">
            <Film class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <span class="setting-copy">
              <span class="setting-title">视频类型</span>
              <span class="setting-subtitle">选择视频内容类型，不同类型使用不同的字幕处理策略和提示词</span>
            </span>
            <span class="setting-inline-action">
              <span class="setting-value">{{ videoContentTypeLabel }}</span>
              <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
            </span>
          </button>

          <div class="setting-row">
            <Pencil class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">字幕校正</div>
              <div class="setting-subtitle">字幕处理过程是否对生成的字幕进行校正</div>
            </div>
            <button
              class="setting-toggle"
              :class="{ active: isSubtitleCorrectionEnabled }"
              type="button"
              :aria-pressed="isSubtitleCorrectionEnabled"
              @click="isSubtitleCorrectionEnabled = !isSubtitleCorrectionEnabled"
            >
              <span class="setting-toggle-label">{{ isSubtitleCorrectionEnabled ? '开' : '关' }}</span>
              <span class="setting-toggle-track" aria-hidden="true">
                <span class="setting-toggle-thumb" />
              </span>
            </button>
          </div>

          <div class="setting-row">
            <Languages class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">字幕翻译</div>
              <div class="setting-subtitle">字幕处理过程是否对生成的字幕进行翻译</div>
            </div>
            <button
              class="setting-toggle"
              :class="{ active: isSubtitleTranslationEnabled }"
              type="button"
              :aria-pressed="isSubtitleTranslationEnabled"
              @click="isSubtitleTranslationEnabled = !isSubtitleTranslationEnabled"
            >
              <span class="setting-toggle-label">{{ isSubtitleTranslationEnabled ? '开' : '关' }}</span>
              <span class="setting-toggle-track" aria-hidden="true">
                <span class="setting-toggle-thumb" />
              </span>
            </button>
          </div>

          <div class="setting-row">
            <AlignJustify class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">AI审核</div>
              <div class="setting-subtitle">审核源文、译文、上下文连贯性和无关噪音行</div>
            </div>
            <button
              class="setting-toggle"
              :class="{ active: isAiSubtitleReviewEnabled }"
              type="button"
              :aria-pressed="isAiSubtitleReviewEnabled"
              @click="isAiSubtitleReviewEnabled = !isAiSubtitleReviewEnabled"
            >
              <span class="setting-toggle-label">{{ isAiSubtitleReviewEnabled ? '开' : '关' }}</span>
              <span class="setting-toggle-track" aria-hidden="true">
                <span class="setting-toggle-thumb" />
              </span>
            </button>
          </div>

          <button class="setting-row setting-row-button" type="button" @click="openAiSubtitleReviewModeDialog">
            <ListChecks class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <span class="setting-copy">
              <span class="setting-title">审核模式</span>
              <span class="setting-subtitle">控制 AI 审核的修正力度</span>
            </span>
            <span class="setting-inline-action">
              <span class="setting-value">{{ aiSubtitleReviewModeLabel }}</span>
              <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
            </span>
          </button>

          <button class="setting-row setting-row-button" type="button" @click="openTargetLanguageDialog">
            <Languages class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <span class="setting-copy">
              <span class="setting-title">目标语言</span>
              <span class="setting-subtitle">选择翻译字幕的目标语言</span>
            </span>
            <span class="setting-inline-action">
              <span class="setting-value">{{ targetLanguageLabel }}</span>
              <ChevronRight class="chevron-icon" :stroke-width="2.4" aria-hidden="true" />
            </span>
          </button>
        </div>
      </section>

      <section class="settings-section" aria-labelledby="dubbing-settings-title">
        <div id="dubbing-settings-title" class="section-heading">
          <MicVocal aria-hidden="true" />
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
              <span class="setting-range-value dubbing-range-value">{{ dubbingTtsIntervalMs }} 毫秒</span>
              <input
                v-model.number="dubbingTtsIntervalMs"
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
              :class="{ active: dubbingIsBackgroundMusicEnabled }"
              type="button"
              :aria-pressed="dubbingIsBackgroundMusicEnabled"
              @click="dubbingIsBackgroundMusicEnabled = !dubbingIsBackgroundMusicEnabled"
            >
              <span class="setting-toggle-label">{{ dubbingIsBackgroundMusicEnabled ? '开' : '关' }}</span>
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
              <span class="setting-range-value dubbing-range-value">{{ dubbingBackgroundMusicVolume.toFixed(1) }}</span>
              <input
                v-model.number="dubbingBackgroundMusicVolume"
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

      <section class="settings-section" aria-labelledby="home-workbench-settings-title">
        <div id="home-workbench-settings-title" class="section-heading">
          <Workflow aria-hidden="true" />
          <span>工作台</span>
        </div>

        <div class="settings-panel">
          <div class="setting-row">
            <Languages class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">翻译与优化</div>
              <div class="setting-subtitle">开启后待办工作台会默认执行字幕翻译与优化</div>
            </div>
            <button
              class="setting-toggle"
              :class="{ active: homeWorkbenchTranslationEnabled }"
              type="button"
              :aria-pressed="homeWorkbenchTranslationEnabled"
              @click="toggleHomeWorkbenchTranslation"
            >
              <span class="setting-toggle-label">{{ homeWorkbenchTranslationEnabled ? '开' : '关' }}</span>
              <span class="setting-toggle-track" aria-hidden="true">
                <span class="setting-toggle-thumb" />
              </span>
            </button>
          </div>

          <div class="setting-row">
            <MicVocal class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">配音</div>
              <div class="setting-subtitle">开启后待办工作台会在翻译后继续生成配音视频</div>
            </div>
            <button
              class="setting-toggle"
              :class="{ active: homeWorkbenchDubbingEnabled }"
              type="button"
              :aria-pressed="homeWorkbenchDubbingEnabled"
              :disabled="!homeWorkbenchTranslationEnabled"
              @click="toggleHomeWorkbenchDubbing"
            >
              <span class="setting-toggle-label">{{ homeWorkbenchDubbingEnabled ? '开' : '关' }}</span>
              <span class="setting-toggle-track" aria-hidden="true">
                <span class="setting-toggle-thumb" />
              </span>
            </button>
          </div>

          <div class="setting-row">
            <FolderOpen class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">导出目录</div>
              <div class="setting-subtitle">{{ homeWorkbenchExportDirLabel }}</div>
            </div>
            <button class="settings-action" type="button" @click="selectHomeWorkbenchExportDir">
              选择目录
            </button>
          </div>
        </div>
      </section>

      <section class="settings-section" aria-labelledby="ytdlp-settings-title">
        <div id="ytdlp-settings-title" class="section-heading">
          <Globe aria-hidden="true" />
          <span>yt-dlp 配置</span>
        </div>

        <div class="settings-panel">
          <div class="setting-row">
            <Globe class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">代理</div>
              <div class="setting-subtitle">留空直连；填写后监控、详情、字幕和视频下载会通过该代理访问 YouTube</div>
            </div>
            <input
              v-model="ytdlpProxy"
              class="setting-control settings-input"
              type="text"
              placeholder="http://127.0.0.1:7890"
              autocomplete="off"
              autocapitalize="off"
              autocorrect="off"
              spellcheck="false"
              aria-label="yt-dlp 代理"
            />
          </div>
        </div>
      </section>

      <section class="settings-section" aria-labelledby="log-settings-title">
        <div id="log-settings-title" class="section-heading">
          <FolderOpen aria-hidden="true" />
          <span>日志</span>
        </div>

        <div class="settings-panel">
          <div class="setting-row">
            <FolderOpen class="setting-icon" :stroke-width="2.1" aria-hidden="true" />
            <div class="setting-copy">
              <div class="setting-title">应用日志</div>
              <div class="setting-subtitle" :class="{ 'setting-subtitle-error': Boolean(logDirectoryError) }">
                {{ logDirectoryError || '查看转录、AI 处理和系统运行日志' }}
              </div>
            </div>
            <button class="settings-action" type="button" @click="openLogDirectory">
              打开日志
            </button>
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

      <div
        v-if="isVideoContentTypeDialogOpen"
        class="dialog-backdrop"
        role="presentation"
        @click.self="closeVideoContentTypeDialog"
      >
        <section
          class="settings-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="video-content-type-dialog-title"
        >
          <h2 id="video-content-type-dialog-title" class="dialog-title">视频类型</h2>
          <div class="dialog-options" role="radiogroup" aria-label="视频类型">
            <button
              v-for="option in videoContentTypeOptions"
              :key="option.value"
              class="dialog-option"
              :class="{ active: selectedVideoContentType === option.value }"
              type="button"
              role="radio"
              :aria-checked="selectedVideoContentType === option.value"
              @click="selectVideoContentType(option.value)"
            >
              <span class="dialog-radio" aria-hidden="true" />
              <span>{{ option.label }}</span>
            </button>
          </div>
        </section>
      </div>

      <div
        v-if="isAiSubtitleReviewModeDialogOpen"
        class="dialog-backdrop"
        role="presentation"
        @click.self="closeAiSubtitleReviewModeDialog"
      >
        <section
          class="settings-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="ai-subtitle-review-mode-title"
        >
          <h2 id="ai-subtitle-review-mode-title" class="dialog-title">审核模式</h2>
          <div class="dialog-options" role="radiogroup" aria-label="审核模式">
            <button
              v-for="option in aiSubtitleReviewModeOptions"
              :key="option.value"
              class="dialog-option"
              :class="{ active: selectedAiSubtitleReviewMode === option.value }"
              type="button"
              role="radio"
              :aria-checked="selectedAiSubtitleReviewMode === option.value"
              @click="selectAiSubtitleReviewMode(option.value)"
            >
              <span class="dialog-radio" aria-hidden="true" />
              <span>{{ option.label }}</span>
            </button>
          </div>
        </section>
      </div>

      <div
        v-if="isReferenceAudioDialogOpen"
        class="dialog-backdrop"
        role="presentation"
        @click.self="closeReferenceAudioDialog"
      >
        <section
          class="settings-dialog reference-audio-dialog"
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
              :class="{ active: draftReferenceAudioSource === option.value }"
              type="button"
              role="radio"
              :aria-checked="draftReferenceAudioSource === option.value"
              @click="selectReferenceAudioSource(option.value)"
            >
              <span class="dialog-radio" aria-hidden="true" />
              <span>{{ option.label }}</span>
            </button>
          </div>

          <div v-if="draftReferenceAudioSource === ReferenceAudioSource.CustomAudioFile" class="reference-audio-picker">
            <div class="reference-audio-file">
              <FileMusic :stroke-width="2.1" aria-hidden="true" />
              <span>{{ draftCustomReferenceAudioFileName }}</span>
              <button
                v-if="draftCustomReferenceAudioPath"
                class="reference-audio-clear-button"
                type="button"
                aria-label="取消选择参考音频"
                @click="clearCustomReferenceAudioFile"
              >
                <X :stroke-width="2.4" aria-hidden="true" />
              </button>
            </div>
            <button class="settings-action" type="button" @click="selectCustomReferenceAudioFile">
              选择音频
            </button>
          </div>

          <footer class="reference-audio-actions">
            <button class="settings-action" type="button" @click="closeReferenceAudioDialog">取消</button>
            <button class="settings-action" type="button" @click="confirmReferenceAudioDialog">确认</button>
          </footer>
        </section>
      </div>

      <div
        v-if="isTargetLanguageDialogOpen"
        class="dialog-backdrop"
        role="presentation"
        @click.self="closeTargetLanguageDialog"
      >
        <section
          class="settings-dialog language-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="target-language-dialog-title"
        >
          <h2 id="target-language-dialog-title" class="dialog-title">目标语言</h2>
          <label class="language-search-field">
            <Search class="language-search-icon" :stroke-width="2.1" aria-hidden="true" />
            <input
              v-model="targetLanguageSearch"
              class="settings-input language-search-input"
              type="search"
              placeholder="搜索语言"
              aria-label="搜索目标语言"
            />
          </label>
          <div class="language-options" role="radiogroup" aria-label="目标语言">
            <button
              v-for="option in filteredTargetLanguageOptions"
              :key="option.value"
              class="dialog-option language-option"
              :class="{ active: selectedTargetLanguage === option.value }"
              type="button"
              role="radio"
              :aria-checked="selectedTargetLanguage === option.value"
              @click="selectTargetLanguage(option.value)"
            >
              <span class="dialog-radio" aria-hidden="true" />
              <span class="language-option-label">{{ option.label }}</span>
            </button>
            <span v-if="filteredTargetLanguageOptions.length === 0" class="language-empty">未找到语言</span>
          </div>
        </section>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { onBeforeRouteLeave } from 'vue-router'
import {
  AlignJustify,
  Bot,
  Brain,
  Captions,
  ChevronRight,
  CircleHelp,
  Eye,
  EyeOff,
  FileMusic,
  Film,
  FolderOpen,
  Gauge,
  Globe,
  KeyRound,
  Languages,
  Link as LinkIcon,
  ListChecks,
  MicVocal,
  Moon,
  Music,
  Pencil,
  Plug,
  RefreshCw,
  Search,
  Server,
  SlidersHorizontal,
  Sun,
  Timer,
  Volume2,
  WandSparkles,
  Workflow,
  X,
} from 'lucide-vue-next'
import { useTheme } from '../composables/useTheme'

const { currentTheme, setTheme, themeLabel } = useTheme()

type ThemeMode = 'light' | 'dark'

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

enum VideoContentType {
  General = 'general',
  Trading = 'trading',
}

enum AiSubtitleReviewMode {
  Expert = 'expert',
  Conservative = 'conservative',
}

enum SubtitleFormat {
  Srt = 'srt',
  Vtt = 'vtt',
  Ass = 'ass',
}

enum OutputMode {
  TargetOnly = 'target-only',
  Bilingual = 'bilingual',
  SourceAndTargetFiles = 'source-and-target-files',
}

enum ReferenceAudioSource {
  ExistingDubbing = 'existing-dubbing',
  CustomAudioFile = 'custom-audio-file',
}

type TargetLanguageOption = {
  value: string
  label: string
}

type LlmConfig = {
  baseUrl: string
  apiKey: string
  model: string
  reasoningEffort: ReasoningEffort
  isStreaming: boolean
}

type LlmConfigs = Record<LlmService, LlmConfig>

type AppSettings = {
  theme: ThemeMode
  transcriptionModel: TranscriptionModel
  sourceLanguage: string
  transcriptionFormat: SubtitleFormat
  translationFormat: SubtitleFormat
  selectedSubtitleStyleId: string
  isSmartSegmentationEnabled: boolean
  selectedLlmService: LlmService
  llmConfigs: LlmConfigs
  translationService: TranslationService
  needsReflectionTranslation: boolean
  translationBatchSize: number
  translationThreadCount: number
  videoContentType: VideoContentType
  outputMode: OutputMode
  isSubtitleCorrectionEnabled: boolean
  isSubtitleTranslationEnabled: boolean
  isAiSubtitleReviewEnabled: boolean
  aiSubtitleReviewMode: AiSubtitleReviewMode
  targetLanguage: string
  dubbingTtsIntervalMs: number
  dubbingReferenceAudioSource: ReferenceAudioSource
  dubbingCustomReferenceAudioPath: string
  dubbingIsBackgroundMusicEnabled: boolean
  dubbingBackgroundMusicVolume: number
  homeWorkbenchTranslationEnabled: boolean
  homeWorkbenchDubbingEnabled: boolean
  homeWorkbenchExportDir: string
  ytdlpProxy: string
}

type LlmConnectionCheckResult = {
  service: string
  model: string
  latencyMs: number
  message: string
}

type LlmConnectionStatus = 'idle' | 'success' | 'error'

const languageDisplayNames = new Intl.DisplayNames(['zh-Hans'], { type: 'language' })

const targetLanguageLabelOverrides: Record<string, string> = {
  aa: '阿法尔语',
  ab: '阿布哈兹语',
  ae: '阿维斯陀语',
  ak: '阿坎语',
  av: '阿瓦尔语',
  ba: '巴什基尔语',
  bi: '比斯拉马语',
  bo: '藏语',
  ce: '车臣语',
  ch: '查莫罗语',
  cr: '克里语',
  cu: '教会斯拉夫语',
  cv: '楚瓦什语',
  dz: '宗喀语',
  ff: '富拉语',
  fj: '斐济语',
  gv: '马恩岛语',
  ho: '希里莫图语',
  hz: '赫雷罗语',
  ie: '国际语（E）',
  ii: '四川彝语',
  ik: '伊努皮克语',
  io: '伊多语',
  iu: '因纽特语',
  kg: '刚果语',
  ki: '吉库尤语',
  kj: '宽亚玛语',
  kl: '格陵兰语',
  kr: '卡努里语',
  ks: '克什米尔语',
  kv: '科米语',
  kw: '康沃尔语',
  li: '林堡语',
  lu: '鲁巴加丹加语',
  mh: '马绍尔语',
  na: '瑙鲁语',
  nd: '北恩德贝莱语',
  ng: '恩敦加语',
  nr: '南恩德贝莱语',
  nv: '纳瓦霍语',
  oj: '奥吉布瓦语',
  os: '奥塞梯语',
  pi: '巴利语',
  rn: '基隆迪语',
  sc: '撒丁语',
  se: '北萨米语',
  sg: '桑戈语',
  ss: '斯瓦蒂语',
  tw: '特威语',
  ty: '塔希提语',
  ve: '文达语',
  vo: '沃拉普克语',
  za: '壮语',
  'zh-Hans': '简体中文',
  'zh-Hant': '繁体中文',
}

const isoLanguageCodes = [
  'aa',
  'ab',
  'ae',
  'af',
  'ak',
  'am',
  'an',
  'ar',
  'as',
  'av',
  'ay',
  'az',
  'ba',
  'be',
  'bg',
  'bh',
  'bi',
  'bm',
  'bn',
  'bo',
  'br',
  'bs',
  'ca',
  'ce',
  'ch',
  'co',
  'cr',
  'cs',
  'cu',
  'cv',
  'cy',
  'da',
  'de',
  'dv',
  'dz',
  'ee',
  'el',
  'en',
  'eo',
  'es',
  'et',
  'eu',
  'fa',
  'ff',
  'fi',
  'fj',
  'fo',
  'fr',
  'fy',
  'ga',
  'gd',
  'gl',
  'gn',
  'gu',
  'gv',
  'ha',
  'he',
  'hi',
  'ho',
  'hr',
  'ht',
  'hu',
  'hy',
  'hz',
  'ia',
  'id',
  'ie',
  'ig',
  'ii',
  'ik',
  'io',
  'is',
  'it',
  'iu',
  'ja',
  'jv',
  'ka',
  'kg',
  'ki',
  'kj',
  'kk',
  'kl',
  'km',
  'kn',
  'ko',
  'kr',
  'ks',
  'ku',
  'kv',
  'kw',
  'ky',
  'la',
  'lb',
  'lg',
  'li',
  'ln',
  'lo',
  'lt',
  'lu',
  'lv',
  'mg',
  'mh',
  'mi',
  'mk',
  'ml',
  'mn',
  'mr',
  'ms',
  'mt',
  'my',
  'na',
  'nb',
  'nd',
  'ne',
  'ng',
  'nl',
  'nn',
  'no',
  'nr',
  'nv',
  'ny',
  'oc',
  'oj',
  'om',
  'or',
  'os',
  'pa',
  'pi',
  'pl',
  'ps',
  'pt',
  'qu',
  'rm',
  'rn',
  'ro',
  'ru',
  'rw',
  'sa',
  'sc',
  'sd',
  'se',
  'sg',
  'si',
  'sk',
  'sl',
  'sm',
  'sn',
  'so',
  'sq',
  'sr',
  'ss',
  'st',
  'su',
  'sv',
  'sw',
  'ta',
  'te',
  'tg',
  'th',
  'ti',
  'tk',
  'tl',
  'tn',
  'to',
  'tr',
  'ts',
  'tt',
  'tw',
  'ty',
  'ug',
  'uk',
  'ur',
  'uz',
  've',
  'vi',
  'vo',
  'wa',
  'wo',
  'xh',
  'yi',
  'yo',
  'za',
  'zu',
] as const

const getTargetLanguageLabel = (value: string) => {
  if (targetLanguageLabelOverrides[value]) {
    return targetLanguageLabelOverrides[value]
  }

  try {
    return languageDisplayNames.of(value) ?? value
  } catch {
    return value
  }
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

const videoContentTypeOptions = [
  { value: VideoContentType.General, label: '通用' },
  { value: VideoContentType.Trading, label: '交易' },
] as const

const aiSubtitleReviewModeOptions = [
  { value: AiSubtitleReviewMode.Expert, label: '专家审核' },
  { value: AiSubtitleReviewMode.Conservative, label: '保守审核' },
] as const

const subtitleFormatOptions = [
  { value: SubtitleFormat.Srt, label: 'SRT' },
  { value: SubtitleFormat.Vtt, label: 'VTT' },
  { value: SubtitleFormat.Ass, label: 'ASS' },
] as const

const outputModeOptions = [
  { value: OutputMode.TargetOnly, label: '仅译文字幕' },
  { value: OutputMode.Bilingual, label: '双语字幕' },
  { value: OutputMode.SourceAndTargetFiles, label: '原文与译文文件' },
] as const

const referenceAudioSourceOptions = [
  { value: ReferenceAudioSource.ExistingDubbing, label: '克隆现有配音' },
  { value: ReferenceAudioSource.CustomAudioFile, label: '自定义音频文件' },
] as const

const targetLanguageOptions: TargetLanguageOption[] = [
  { value: 'zh-Hans', label: '简体中文' },
  { value: 'zh-Hant', label: '繁体中文' },
  ...isoLanguageCodes
    .map((value) => ({
      value,
      label: getTargetLanguageLabel(value),
    }))
    .sort((a, b) => a.label.localeCompare(b.label, 'zh-Hans')),
]

const createDefaultLlmConfig = (): LlmConfig => ({
  baseUrl: '',
  apiKey: '',
  model: '',
  reasoningEffort: ReasoningEffort.Off,
  isStreaming: true,
})

const createDefaultLlmConfigs = (): LlmConfigs => ({
  [LlmService.OpenAI]: createDefaultLlmConfig(),
  [LlmService.OpenAIResponses]: createDefaultLlmConfig(),
  [LlmService.Anthropic]: createDefaultLlmConfig(),
})

const readOptionValue = <T extends string>(
  value: unknown,
  options: readonly { readonly value: T }[],
  fallback: T,
) => {
  return typeof value === 'string' && options.some((option) => option.value === value) ? (value as T) : fallback
}

const readTheme = (value: unknown): ThemeMode => {
  return value === 'dark' || value === 'light' ? value : 'light'
}

const normalizeLlmConfig = (config?: Partial<LlmConfig>): LlmConfig => ({
  baseUrl: typeof config?.baseUrl === 'string' ? config.baseUrl : '',
  apiKey: typeof config?.apiKey === 'string' ? config.apiKey : '',
  model: typeof config?.model === 'string' ? config.model : '',
  reasoningEffort: readOptionValue(config?.reasoningEffort, reasoningEffortOptions, ReasoningEffort.Off),
  isStreaming: typeof config?.isStreaming === 'boolean' ? config.isStreaming : true,
})

const normalizeLlmConfigs = (configs?: Partial<Record<LlmService, Partial<LlmConfig>>>): LlmConfigs => ({
  [LlmService.OpenAI]: normalizeLlmConfig(configs?.[LlmService.OpenAI]),
  [LlmService.OpenAIResponses]: normalizeLlmConfig(configs?.[LlmService.OpenAIResponses]),
  [LlmService.Anthropic]: normalizeLlmConfig(configs?.[LlmService.Anthropic]),
})

const readNumberSetting = (value: unknown, fallback: number, min: number, max: number) => {
  const numberValue = typeof value === 'number' && Number.isFinite(value) ? value : fallback

  return Math.min(Math.max(numberValue, min), max)
}

const normalizeSettings = (settings: Partial<AppSettings>): AppSettings => ({
  theme: readTheme(settings.theme),
  transcriptionModel: readOptionValue(
    settings.transcriptionModel,
    transcriptionModelOptions,
    TranscriptionModel.Bilibili,
  ),
  sourceLanguage: typeof settings.sourceLanguage === 'string' ? settings.sourceLanguage : 'auto',
  transcriptionFormat: readOptionValue(settings.transcriptionFormat, subtitleFormatOptions, SubtitleFormat.Srt),
  translationFormat: readOptionValue(settings.translationFormat, subtitleFormatOptions, SubtitleFormat.Ass),
  selectedSubtitleStyleId:
    typeof settings.selectedSubtitleStyleId === 'string' && settings.selectedSubtitleStyleId.trim()
      ? settings.selectedSubtitleStyleId
      : 'default',
  isSmartSegmentationEnabled:
    typeof settings.isSmartSegmentationEnabled === 'boolean' ? settings.isSmartSegmentationEnabled : true,
  selectedLlmService: readOptionValue(settings.selectedLlmService, llmServiceOptions, LlmService.OpenAI),
  llmConfigs: normalizeLlmConfigs(settings.llmConfigs),
  translationService: readOptionValue(settings.translationService, translationServiceOptions, TranslationService.Llm),
  needsReflectionTranslation:
    typeof settings.needsReflectionTranslation === 'boolean' ? settings.needsReflectionTranslation : true,
  translationBatchSize: readNumberSetting(settings.translationBatchSize, 30, 10, 100),
  translationThreadCount: readNumberSetting(settings.translationThreadCount, 10, 1, 100),
  videoContentType: readOptionValue(settings.videoContentType, videoContentTypeOptions, VideoContentType.General),
  outputMode: readOptionValue(settings.outputMode, outputModeOptions, OutputMode.Bilingual),
  isSubtitleCorrectionEnabled:
    typeof settings.isSubtitleCorrectionEnabled === 'boolean' ? settings.isSubtitleCorrectionEnabled : true,
  isSubtitleTranslationEnabled:
    typeof settings.isSubtitleTranslationEnabled === 'boolean' ? settings.isSubtitleTranslationEnabled : true,
  isAiSubtitleReviewEnabled:
    typeof settings.isAiSubtitleReviewEnabled === 'boolean' ? settings.isAiSubtitleReviewEnabled : true,
  aiSubtitleReviewMode: readOptionValue(
    settings.aiSubtitleReviewMode,
    aiSubtitleReviewModeOptions,
    AiSubtitleReviewMode.Expert,
  ),
  targetLanguage:
    typeof settings.targetLanguage === 'string' &&
    targetLanguageOptions.some((option) => option.value === settings.targetLanguage)
      ? settings.targetLanguage
      : 'zh-Hans',
  dubbingTtsIntervalMs: readNumberSetting(settings.dubbingTtsIntervalMs, 150, 0, 1000),
  dubbingReferenceAudioSource: readOptionValue(
    settings.dubbingReferenceAudioSource,
    referenceAudioSourceOptions,
    ReferenceAudioSource.ExistingDubbing,
  ),
  dubbingCustomReferenceAudioPath:
    typeof settings.dubbingCustomReferenceAudioPath === 'string' ? settings.dubbingCustomReferenceAudioPath : '',
  dubbingIsBackgroundMusicEnabled:
    typeof settings.dubbingIsBackgroundMusicEnabled === 'boolean'
      ? settings.dubbingIsBackgroundMusicEnabled
      : true,
  dubbingBackgroundMusicVolume: readNumberSetting(settings.dubbingBackgroundMusicVolume, 0.5, 0, 1),
  homeWorkbenchTranslationEnabled:
    typeof settings.homeWorkbenchTranslationEnabled === 'boolean' ? settings.homeWorkbenchTranslationEnabled : true,
  homeWorkbenchDubbingEnabled:
    typeof settings.homeWorkbenchDubbingEnabled === 'boolean'
      ? settings.homeWorkbenchDubbingEnabled && (settings.homeWorkbenchTranslationEnabled ?? true)
      : false,
  homeWorkbenchExportDir:
    typeof settings.homeWorkbenchExportDir === 'string' ? settings.homeWorkbenchExportDir : '',
  ytdlpProxy: typeof settings.ytdlpProxy === 'string' ? settings.ytdlpProxy : '',
})

const selectedTranscriptionModel = ref<TranscriptionModel>(TranscriptionModel.Bilibili)
const selectedSourceLanguage = ref('auto')
const selectedTranscriptionFormat = ref<SubtitleFormat>(SubtitleFormat.Srt)
const selectedTranslationFormat = ref<SubtitleFormat>(SubtitleFormat.Ass)
const selectedSubtitleStyleId = ref('default')
const isSmartSegmentationEnabled = ref(true)
const isTranscriptionModelDialogOpen = ref(false)
const selectedLlmService = ref<LlmService>(LlmService.OpenAI)
const isLlmServiceDialogOpen = ref(false)
const isLlmApiKeyVisible = ref(false)
const llmConfigs = ref<LlmConfigs>(createDefaultLlmConfigs())
const isCheckingLlmConnection = ref(false)
const llmConnectionStatus = ref<LlmConnectionStatus>('idle')
const llmConnectionMessage = ref('')
const isReasoningEffortDialogOpen = ref(false)
const selectedTranslationService = ref<TranslationService>(TranslationService.Llm)
const isTranslationServiceDialogOpen = ref(false)
const needsReflectionTranslation = ref(true)
const translationBatchSize = ref(30)
const translationThreadCount = ref(10)
const selectedVideoContentType = ref<VideoContentType>(VideoContentType.General)
const selectedOutputMode = ref<OutputMode>(OutputMode.Bilingual)
const isVideoContentTypeDialogOpen = ref(false)
const isSubtitleCorrectionEnabled = ref(true)
const isSubtitleTranslationEnabled = ref(true)
const isAiSubtitleReviewEnabled = ref(true)
const selectedAiSubtitleReviewMode = ref<AiSubtitleReviewMode>(AiSubtitleReviewMode.Expert)
const isAiSubtitleReviewModeDialogOpen = ref(false)
const selectedTargetLanguage = ref('zh-Hans')
const isTargetLanguageDialogOpen = ref(false)
const targetLanguageSearch = ref('')
const dubbingTtsIntervalMs = ref(150)
const selectedReferenceAudioSource = ref<ReferenceAudioSource>(ReferenceAudioSource.ExistingDubbing)
const dubbingCustomReferenceAudioPath = ref('')
const draftReferenceAudioSource = ref<ReferenceAudioSource>(ReferenceAudioSource.ExistingDubbing)
const draftCustomReferenceAudioPath = ref('')
const isReferenceAudioDialogOpen = ref(false)
const dubbingIsBackgroundMusicEnabled = ref(true)
const dubbingBackgroundMusicVolume = ref(0.5)
const homeWorkbenchTranslationEnabled = ref(true)
const homeWorkbenchDubbingEnabled = ref(false)
const homeWorkbenchExportDir = ref('')
const ytdlpProxy = ref('')
const logDirectoryError = ref('')
const isSettingsLoaded = ref(false)
let isApplyingStoredSettings = false
let saveSettingsTimer: ReturnType<typeof window.setTimeout> | undefined

const isTauriRuntime = () => '__TAURI_INTERNALS__' in window
const referenceAudioExtensions = ['wav', 'mp3', 'm4a', 'aac', 'flac', 'ogg', 'opus', 'wma']

const getCurrentLlmConfig = () => {
  return llmConfigs.value[selectedLlmService.value]
}

const resetLlmConnectionStatus = () => {
  llmConnectionStatus.value = 'idle'
  llmConnectionMessage.value = ''
}

const updateCurrentLlmConfig = (patch: Partial<LlmConfig>) => {
  const service = selectedLlmService.value
  llmConfigs.value = {
    ...llmConfigs.value,
    [service]: {
      ...getCurrentLlmConfig(),
      ...patch,
    },
  }
  resetLlmConnectionStatus()
}

const llmBaseUrl = computed({
  get: () => getCurrentLlmConfig().baseUrl,
  set: (baseUrl: string) => updateCurrentLlmConfig({ baseUrl }),
})

const llmApiKey = computed({
  get: () => getCurrentLlmConfig().apiKey,
  set: (apiKey: string) => updateCurrentLlmConfig({ apiKey }),
})

const llmModel = computed({
  get: () => getCurrentLlmConfig().model,
  set: (model: string) => updateCurrentLlmConfig({ model }),
})

const selectedReasoningEffort = computed({
  get: () => getCurrentLlmConfig().reasoningEffort,
  set: (reasoningEffort: ReasoningEffort) => updateCurrentLlmConfig({ reasoningEffort }),
})

const isLlmStreaming = computed({
  get: () => getCurrentLlmConfig().isStreaming,
  set: (isStreaming: boolean) => updateCurrentLlmConfig({ isStreaming }),
})

const transcriptionModelLabel = computed(() => {
  return transcriptionModelOptions.find((option) => option.value === selectedTranscriptionModel.value)?.label ?? ''
})

const llmServiceLabel = computed(() => {
  return llmServiceOptions.find((option) => option.value === selectedLlmService.value)?.label ?? ''
})

const llmConnectionSubtitle = computed(() => {
  if (isCheckingLlmConnection.value) {
    return '正在发送最小测试请求'
  }

  return llmConnectionMessage.value || '发送最小测试请求验证当前服务和模型'
})

const llmConnectionSubtitleClass = computed(() => ({
  'setting-subtitle-success': llmConnectionStatus.value === 'success',
  'setting-subtitle-error': llmConnectionStatus.value === 'error',
}))

const reasoningEffortLabel = computed(() => {
  return reasoningEffortOptions.find((option) => option.value === selectedReasoningEffort.value)?.label ?? ''
})

const translationServiceLabel = computed(() => {
  return translationServiceOptions.find((option) => option.value === selectedTranslationService.value)?.label ?? ''
})

const videoContentTypeLabel = computed(() => {
  return videoContentTypeOptions.find((option) => option.value === selectedVideoContentType.value)?.label ?? ''
})

const aiSubtitleReviewModeLabel = computed(() => {
  return aiSubtitleReviewModeOptions.find((option) => option.value === selectedAiSubtitleReviewMode.value)?.label ?? ''
})

const targetLanguageLabel = computed(() => {
  return targetLanguageOptions.find((option) => option.value === selectedTargetLanguage.value)?.label ?? ''
})

const referenceAudioSourceLabel = computed(() => {
  return referenceAudioSourceOptions.find((option) => option.value === selectedReferenceAudioSource.value)?.label ?? ''
})

const draftCustomReferenceAudioFileName = computed(() => {
  return draftCustomReferenceAudioPath.value ? fileNameFromPath(draftCustomReferenceAudioPath.value) : '未选择音频'
})

const homeWorkbenchExportDirLabel = computed(() => {
  return homeWorkbenchExportDir.value || '未选择时使用应用默认导出目录'
})

const filteredTargetLanguageOptions = computed(() => {
  const query = targetLanguageSearch.value.trim().toLowerCase()

  if (!query) {
    return targetLanguageOptions
  }

  return targetLanguageOptions.filter((option) => {
    return option.label.toLowerCase().includes(query) || option.value.toLowerCase().includes(query)
  })
})

const createSettingsSnapshot = (): AppSettings => ({
  theme: currentTheme.value,
  transcriptionModel: selectedTranscriptionModel.value,
  sourceLanguage: selectedSourceLanguage.value,
  transcriptionFormat: selectedTranscriptionFormat.value,
  translationFormat: selectedTranslationFormat.value,
  selectedSubtitleStyleId: selectedSubtitleStyleId.value,
  isSmartSegmentationEnabled: isSmartSegmentationEnabled.value,
  selectedLlmService: selectedLlmService.value,
  llmConfigs: llmConfigs.value,
  translationService: selectedTranslationService.value,
  needsReflectionTranslation: needsReflectionTranslation.value,
  translationBatchSize: translationBatchSize.value,
  translationThreadCount: translationThreadCount.value,
  videoContentType: selectedVideoContentType.value,
  outputMode: selectedOutputMode.value,
  isSubtitleCorrectionEnabled: isSubtitleCorrectionEnabled.value,
  isSubtitleTranslationEnabled: isSubtitleTranslationEnabled.value,
  isAiSubtitleReviewEnabled: isAiSubtitleReviewEnabled.value,
  aiSubtitleReviewMode: selectedAiSubtitleReviewMode.value,
  targetLanguage: selectedTargetLanguage.value,
  dubbingTtsIntervalMs: dubbingTtsIntervalMs.value,
  dubbingReferenceAudioSource: selectedReferenceAudioSource.value,
  dubbingCustomReferenceAudioPath: dubbingCustomReferenceAudioPath.value,
  dubbingIsBackgroundMusicEnabled: dubbingIsBackgroundMusicEnabled.value,
  dubbingBackgroundMusicVolume: dubbingBackgroundMusicVolume.value,
  homeWorkbenchTranslationEnabled: homeWorkbenchTranslationEnabled.value,
  homeWorkbenchDubbingEnabled: homeWorkbenchDubbingEnabled.value,
  homeWorkbenchExportDir: homeWorkbenchExportDir.value.trim(),
  ytdlpProxy: ytdlpProxy.value.trim(),
})

const applySettings = (settings: AppSettings) => {
  isApplyingStoredSettings = true

  setTheme(settings.theme)
  selectedTranscriptionModel.value = settings.transcriptionModel
  selectedSourceLanguage.value = settings.sourceLanguage
  selectedTranscriptionFormat.value = settings.transcriptionFormat
  selectedTranslationFormat.value = settings.translationFormat
  selectedSubtitleStyleId.value = settings.selectedSubtitleStyleId
  isSmartSegmentationEnabled.value = settings.isSmartSegmentationEnabled
  selectedLlmService.value = settings.selectedLlmService
  llmConfigs.value = settings.llmConfigs
  selectedTranslationService.value = settings.translationService
  needsReflectionTranslation.value = settings.needsReflectionTranslation
  translationBatchSize.value = settings.translationBatchSize
  translationThreadCount.value = settings.translationThreadCount
  selectedVideoContentType.value = settings.videoContentType
  selectedOutputMode.value = settings.outputMode
  isSubtitleCorrectionEnabled.value = settings.isSubtitleCorrectionEnabled
  isSubtitleTranslationEnabled.value = settings.isSubtitleTranslationEnabled
  isAiSubtitleReviewEnabled.value = settings.isAiSubtitleReviewEnabled
  selectedAiSubtitleReviewMode.value = settings.aiSubtitleReviewMode
  selectedTargetLanguage.value = settings.targetLanguage
  dubbingTtsIntervalMs.value = settings.dubbingTtsIntervalMs
  selectedReferenceAudioSource.value = settings.dubbingReferenceAudioSource
  dubbingCustomReferenceAudioPath.value = settings.dubbingCustomReferenceAudioPath
  dubbingIsBackgroundMusicEnabled.value = settings.dubbingIsBackgroundMusicEnabled
  dubbingBackgroundMusicVolume.value = settings.dubbingBackgroundMusicVolume
  homeWorkbenchTranslationEnabled.value = settings.homeWorkbenchTranslationEnabled
  homeWorkbenchDubbingEnabled.value =
    settings.homeWorkbenchDubbingEnabled && settings.homeWorkbenchTranslationEnabled
  homeWorkbenchExportDir.value = settings.homeWorkbenchExportDir
  ytdlpProxy.value = settings.ytdlpProxy
  resetLlmConnectionStatus()

  nextTick(() => {
    isApplyingStoredSettings = false
  })
}

const saveSettingsNow = async () => {
  if (!isSettingsLoaded.value || isApplyingStoredSettings) {
    return
  }

  if (!isTauriRuntime()) {
    return
  }

  try {
    await invoke('save_settings', { settings: createSettingsSnapshot() })
  } catch (error) {
    console.error('保存设置失败', error)
  }
}

const flushPendingSave = async () => {
  if (saveSettingsTimer !== undefined) {
    window.clearTimeout(saveSettingsTimer)
    saveSettingsTimer = undefined
  }

  await saveSettingsNow()
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

const checkLlmConnection = async () => {
  if (isCheckingLlmConnection.value) {
    return
  }

  llmConnectionStatus.value = 'idle'
  llmConnectionMessage.value = ''

  if (!isTauriRuntime()) {
    llmConnectionStatus.value = 'error'
    llmConnectionMessage.value = '请在桌面应用中检查连接'
    return
  }

  isCheckingLlmConnection.value = true

  try {
    await flushPendingSave()
    const result = await invoke<LlmConnectionCheckResult>('check_llm_connection')
    llmConnectionStatus.value = 'success'
    llmConnectionMessage.value = `连接正常 · ${result.model} · ${result.latencyMs}ms`
  } catch (error) {
    llmConnectionStatus.value = 'error'
    llmConnectionMessage.value = stringifyError(error)
  } finally {
    isCheckingLlmConnection.value = false
  }
}

const openLogDirectory = async () => {
  logDirectoryError.value = ''

  if (!isTauriRuntime()) {
    logDirectoryError.value = '请在桌面应用中打开日志'
    return
  }

  try {
    await invoke<string>('open_log_directory')
  } catch (error) {
    logDirectoryError.value = stringifyError(error)
  }
}

const loadStoredSettings = async () => {
  if (!isTauriRuntime()) {
    applySettings(normalizeSettings({}))
    await nextTick()
    isSettingsLoaded.value = true
    return
  }

  try {
    const storedSettings = await invoke<Partial<AppSettings>>('load_settings')
    applySettings(normalizeSettings(storedSettings))
  } catch (error) {
    console.error('加载设置失败', error)
  } finally {
    await nextTick()
    isSettingsLoaded.value = true
    void saveSettingsNow()
  }
}

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
  resetLlmConnectionStatus()
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

const openVideoContentTypeDialog = () => {
  isVideoContentTypeDialogOpen.value = true
}

const closeVideoContentTypeDialog = () => {
  isVideoContentTypeDialogOpen.value = false
}

const selectVideoContentType = (type: VideoContentType) => {
  selectedVideoContentType.value = type
  closeVideoContentTypeDialog()
}

const openAiSubtitleReviewModeDialog = () => {
  isAiSubtitleReviewModeDialogOpen.value = true
}

const closeAiSubtitleReviewModeDialog = () => {
  isAiSubtitleReviewModeDialogOpen.value = false
}

const selectAiSubtitleReviewMode = (mode: AiSubtitleReviewMode) => {
  selectedAiSubtitleReviewMode.value = mode
  closeAiSubtitleReviewModeDialog()
}

const openReferenceAudioDialog = () => {
  draftReferenceAudioSource.value = selectedReferenceAudioSource.value
  draftCustomReferenceAudioPath.value = dubbingCustomReferenceAudioPath.value
  isReferenceAudioDialogOpen.value = true
}

const closeReferenceAudioDialog = () => {
  isReferenceAudioDialogOpen.value = false
}

const confirmReferenceAudioDialog = () => {
  selectedReferenceAudioSource.value = draftReferenceAudioSource.value
  dubbingCustomReferenceAudioPath.value = draftCustomReferenceAudioPath.value
  closeReferenceAudioDialog()
}

const selectReferenceAudioSource = (source: ReferenceAudioSource) => {
  draftReferenceAudioSource.value = source
}

const selectCustomReferenceAudioFile = async () => {
  if (!isTauriRuntime()) {
    return
  }

  const selected = await open({
    title: '选择参考音频',
    multiple: false,
    filters: [
      {
        name: '音频文件',
        extensions: referenceAudioExtensions,
      },
    ],
  })

  if (typeof selected === 'string') {
    draftCustomReferenceAudioPath.value = selected
    draftReferenceAudioSource.value = ReferenceAudioSource.CustomAudioFile
  }
}

const clearCustomReferenceAudioFile = () => {
  draftCustomReferenceAudioPath.value = ''
}

const toggleHomeWorkbenchTranslation = () => {
  homeWorkbenchTranslationEnabled.value = !homeWorkbenchTranslationEnabled.value
  if (!homeWorkbenchTranslationEnabled.value) {
    homeWorkbenchDubbingEnabled.value = false
  }
}

const toggleHomeWorkbenchDubbing = () => {
  if (!homeWorkbenchTranslationEnabled.value) {
    return
  }
  homeWorkbenchDubbingEnabled.value = !homeWorkbenchDubbingEnabled.value
}

const selectHomeWorkbenchExportDir = async () => {
  if (!isTauriRuntime()) {
    return
  }

  const selected = await open({
    title: '选择默认导出目录',
    directory: true,
    multiple: false,
  })

  if (typeof selected === 'string') {
    homeWorkbenchExportDir.value = selected
  }
}

const openTargetLanguageDialog = () => {
  targetLanguageSearch.value = ''
  isTargetLanguageDialogOpen.value = true
}

const closeTargetLanguageDialog = () => {
  isTargetLanguageDialogOpen.value = false
}

const selectTargetLanguage = (language: string) => {
  selectedTargetLanguage.value = language
  closeTargetLanguageDialog()
}

const fileNameFromPath = (path: string) => {
  const normalizedPath = path.replace(/\\/g, '/')
  return normalizedPath.split('/').filter(Boolean).pop() ?? path
}

const handleKeydown = (event: KeyboardEvent) => {
  if (event.key === 'Escape') {
    closeTranscriptionModelDialog()
    closeLlmServiceDialog()
    closeReasoningEffortDialog()
    closeTranslationServiceDialog()
    closeVideoContentTypeDialog()
    closeAiSubtitleReviewModeDialog()
    closeReferenceAudioDialog()
    closeTargetLanguageDialog()
  }
}

const stringifyError = (error: unknown, fallback = '检查连接失败') => {
  if (typeof error === 'string') {
    return error
  }

  if (error instanceof Error) {
    return error.message
  }

  return fallback
}

watch(createSettingsSnapshot, scheduleSaveSettings, { deep: true })

window.addEventListener('keydown', handleKeydown)

onMounted(() => {
  void loadStoredSettings()
})

onBeforeRouteLeave(async () => {
  await flushPendingSave()
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
