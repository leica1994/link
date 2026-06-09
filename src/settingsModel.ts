export type ThemeMode = 'light' | 'dark'

export enum TranscriptionModel {
  Bilibili = 'bilibili',
}

export enum LlmService {
  OpenAI = 'openai',
  OpenAIResponses = 'openai-responses',
  Anthropic = 'anthropic',
}

export enum ReasoningEffort {
  UltraHigh = 'ultra-high',
  High = 'high',
  Medium = 'medium',
  Low = 'low',
  Off = 'off',
}

export enum TranslationService {
  Llm = 'llm',
  DeepLx = 'deeplx',
  Microsoft = 'microsoft',
  Google = 'google',
}

export enum VideoContentType {
  General = 'general',
  Trading = 'trading',
}

export enum SubtitleFormat {
  Srt = 'srt',
  Vtt = 'vtt',
  Ass = 'ass',
}

export enum OutputMode {
  TargetOnly = 'target-only',
  Bilingual = 'bilingual',
  SourceAndTargetFiles = 'source-and-target-files',
}

export enum ReferenceAudioSource {
  ExistingDubbing = 'existing-dubbing',
  CustomAudioFile = 'custom-audio-file',
}

export type TargetLanguageOption = {
  value: string
  label: string
}

export type LlmConfig = {
  baseUrl: string
  apiKey: string
  model: string
  reasoningEffort: ReasoningEffort
  isStreaming: boolean
}

export type LlmConfigs = Record<LlmService, LlmConfig>

export type AppSettings = {
  theme: ThemeMode
  transcriptionModel: TranscriptionModel
  sourceLanguage: string
  transcriptionFormat: SubtitleFormat
  translationFormat: SubtitleFormat
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
  isPostTranslationOptimizationEnabled: boolean
  targetLanguage: string
  dubbingTtsIntervalMs: number
  dubbingReferenceAudioSource: ReferenceAudioSource
  dubbingIsBackgroundMusicEnabled: boolean
  dubbingBackgroundMusicVolume: number
}

export const transcriptionModelOptions = [
  { value: TranscriptionModel.Bilibili, label: 'B站转录' },
] as const

export const llmServiceOptions = [
  { value: LlmService.OpenAI, label: 'OpenAI' },
  { value: LlmService.OpenAIResponses, label: 'OpenAI Responses' },
  { value: LlmService.Anthropic, label: 'Anthropic' },
] as const

export const reasoningEffortOptions = [
  { value: ReasoningEffort.UltraHigh, label: '超高' },
  { value: ReasoningEffort.High, label: '高' },
  { value: ReasoningEffort.Medium, label: '中' },
  { value: ReasoningEffort.Low, label: '低' },
  { value: ReasoningEffort.Off, label: '关闭' },
] as const

export const translationServiceOptions = [
  { value: TranslationService.Llm, label: 'LLM 大模型翻译' },
  { value: TranslationService.DeepLx, label: 'DeepLx 翻译' },
  { value: TranslationService.Microsoft, label: '微软翻译' },
  { value: TranslationService.Google, label: '谷歌翻译' },
] as const

export const videoContentTypeOptions = [
  { value: VideoContentType.General, label: '通用' },
  { value: VideoContentType.Trading, label: '交易' },
] as const

export const subtitleFormatOptions = [
  { value: SubtitleFormat.Srt, label: 'SRT' },
  { value: SubtitleFormat.Vtt, label: 'VTT' },
  { value: SubtitleFormat.Ass, label: 'ASS' },
] as const

export const outputModeOptions = [
  { value: OutputMode.TargetOnly, label: '仅译文字幕' },
  { value: OutputMode.Bilingual, label: '双语字幕' },
  { value: OutputMode.SourceAndTargetFiles, label: '原文与译文文件' },
] as const

export const referenceAudioSourceOptions = [
  { value: ReferenceAudioSource.ExistingDubbing, label: '克隆现有配音' },
  { value: ReferenceAudioSource.CustomAudioFile, label: '自定义音频文件' },
] as const

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

export const getLanguageLabel = (value: string) => {
  if (value === 'auto') {
    return '自动识别'
  }

  if (targetLanguageLabelOverrides[value]) {
    return targetLanguageLabelOverrides[value]
  }

  try {
    return languageDisplayNames.of(value) ?? value
  } catch {
    return value
  }
}

export const targetLanguageOptions: TargetLanguageOption[] = [
  { value: 'zh-Hans', label: '简体中文' },
  { value: 'zh-Hant', label: '繁体中文' },
  ...isoLanguageCodes
    .map((value) => ({
      value,
      label: getLanguageLabel(value),
    }))
    .sort((a, b) => a.label.localeCompare(b.label, 'zh-Hans')),
]

export const sourceLanguageOptions: TargetLanguageOption[] = [
  { value: 'auto', label: '自动识别' },
  ...targetLanguageOptions,
]

export const createDefaultLlmConfig = (): LlmConfig => ({
  baseUrl: '',
  apiKey: '',
  model: '',
  reasoningEffort: ReasoningEffort.Off,
  isStreaming: true,
})

export const createDefaultLlmConfigs = (): LlmConfigs => ({
  [LlmService.OpenAI]: createDefaultLlmConfig(),
  [LlmService.OpenAIResponses]: createDefaultLlmConfig(),
  [LlmService.Anthropic]: createDefaultLlmConfig(),
})

export const readOptionValue = <T extends string>(
  value: unknown,
  options: readonly { readonly value: T }[],
  fallback: T,
) => {
  return typeof value === 'string' && options.some((option) => option.value === value) ? (value as T) : fallback
}

export const readTheme = (value: unknown): ThemeMode => {
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

export const normalizeSettings = (settings: Partial<AppSettings>): AppSettings => ({
  theme: readTheme(settings.theme),
  transcriptionModel: readOptionValue(
    settings.transcriptionModel,
    transcriptionModelOptions,
    TranscriptionModel.Bilibili,
  ),
  sourceLanguage:
    typeof settings.sourceLanguage === 'string' &&
    sourceLanguageOptions.some((option) => option.value === settings.sourceLanguage)
      ? settings.sourceLanguage
      : 'auto',
  transcriptionFormat: readOptionValue(settings.transcriptionFormat, subtitleFormatOptions, SubtitleFormat.Srt),
  translationFormat: readOptionValue(settings.translationFormat, subtitleFormatOptions, SubtitleFormat.Srt),
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
  isPostTranslationOptimizationEnabled:
    typeof settings.isPostTranslationOptimizationEnabled === 'boolean'
      ? settings.isPostTranslationOptimizationEnabled
      : true,
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
  dubbingIsBackgroundMusicEnabled:
    typeof settings.dubbingIsBackgroundMusicEnabled === 'boolean'
      ? settings.dubbingIsBackgroundMusicEnabled
      : true,
  dubbingBackgroundMusicVolume: readNumberSetting(settings.dubbingBackgroundMusicVolume, 0.5, 0, 1),
})

export const getOptionLabel = <T extends string>(
  options: readonly { readonly value: T; readonly label: string }[],
  value: T,
) => {
  return options.find((option) => option.value === value)?.label ?? ''
}
