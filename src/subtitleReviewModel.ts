export type ReviewSubtitleFormat = 'srt' | 'vtt' | 'ass'

export type ReviewTextMode = 'plain' | 'raw'

export type ReviewCue = {
  id: string
  startTime: number
  endTime: number
  text: string
  rawText: string
  textMode: ReviewTextMode
  styleName: string
  layer: string
  actor: string
  marginL: string
  marginR: string
  marginV: string
  effect: string
  hasInlineTags: boolean
  sourceOrder: number
  isNew: boolean
}

export type ReviewVideoMetadata = {
  path: string
  previewPath: string
  fileName: string
  durationMs: number
  width: number
  height: number
  videoCodec: string
  audioCodec: string
  fileSize: number
}

export type ReviewValidationIssue = {
  cueId?: string
  level: 'error' | 'warning'
  code: string
  message: string
}

export type ReviewValidation = {
  canExport: boolean
  issues: ReviewValidationIssue[]
}

export type PrepareSubtitleReviewResult = {
  sessionId: string
  video: ReviewVideoMetadata
  subtitleFormat: ReviewSubtitleFormat
  styleName: string
  cues: ReviewCue[]
  revision: number
  assContent: string
  validation: ReviewValidation
  warnings: string[]
}

export type UpdateSubtitleReviewResult = {
  revision: number
  assContent: string
  validation: ReviewValidation
}

export type CueLane = {
  cue: ReviewCue
  lane: number
}

export const MIN_CUE_DURATION_MS = 100
export const DEFAULT_CUE_DURATION_MS = 2_000
export const SNAP_GRID_MS = 100
export const SNAP_THRESHOLD_MS = 80

export const cloneReviewCues = (cues: readonly ReviewCue[]): ReviewCue[] => cues.map((cue) => ({ ...cue }))

export const sortReviewCues = (cues: readonly ReviewCue[]): ReviewCue[] =>
  [...cues].sort(
    (left, right) =>
      left.startTime - right.startTime || left.endTime - right.endTime || left.sourceOrder - right.sourceOrder,
  )

export const formatReviewTime = (value: number): string => {
  const safeValue = Math.max(0, Math.round(value))
  const milliseconds = safeValue % 1_000
  const totalSeconds = Math.floor(safeValue / 1_000)
  const seconds = totalSeconds % 60
  const totalMinutes = Math.floor(totalSeconds / 60)
  const minutes = totalMinutes % 60
  const hours = Math.floor(totalMinutes / 60)
  return `${padNumber(hours, 2)}:${padNumber(minutes, 2)}:${padNumber(seconds, 2)}.${padNumber(milliseconds, 3)}`
}

export const formatCompactTime = (value: number): string => {
  const safeValue = Math.max(0, Math.round(value))
  const totalSeconds = Math.floor(safeValue / 1_000)
  const seconds = totalSeconds % 60
  const totalMinutes = Math.floor(totalSeconds / 60)
  const minutes = totalMinutes % 60
  const hours = Math.floor(totalMinutes / 60)
  return hours > 0
    ? `${padNumber(hours, 2)}:${padNumber(minutes, 2)}:${padNumber(seconds, 2)}`
    : `${padNumber(minutes, 2)}:${padNumber(seconds, 2)}`
}

export const parseReviewTime = (value: string): number | null => {
  const normalized = value.trim().replace(',', '.')
  const match = normalized.match(/^(?:(\d+):)?(\d{1,2}):(\d{1,2})(?:\.(\d{1,3}))?$/)
  if (!match) {
    return null
  }
  const hours = Number(match[1] ?? 0)
  const minutes = Number(match[2])
  const seconds = Number(match[3])
  if (!Number.isFinite(hours) || (match[1] !== undefined && minutes >= 60) || seconds >= 60) {
    return null
  }
  const milliseconds = Number((match[4] ?? '').padEnd(3, '0'))
  return ((hours * 60 + minutes) * 60 + seconds) * 1_000 + milliseconds
}

export const findActiveCueIds = (cues: readonly ReviewCue[], currentTimeMs: number): string[] =>
  cues
    .filter((cue) => cue.startTime <= currentTimeMs && cue.endTime > currentTimeMs)
    .map((cue) => cue.id)

export const findPlaybackCueId = (
  cues: readonly ReviewCue[],
  currentTimeMs: number,
  selectedCueId = '',
): string => {
  const activeCues = sortReviewCues(cues).filter(
    (cue) => cue.startTime <= currentTimeMs && cue.endTime > currentTimeMs,
  )
  if (activeCues.some((cue) => cue.id === selectedCueId)) {
    return selectedCueId
  }
  return activeCues[0]?.id ?? ''
}

export const assignCueLanes = (cues: readonly ReviewCue[]): CueLane[] => {
  const laneEnds: number[] = []
  return sortReviewCues(cues).map((cue) => {
    let lane = laneEnds.findIndex((endTime) => endTime <= cue.startTime)
    if (lane < 0) {
      lane = laneEnds.length
      laneEnds.push(cue.endTime)
    } else {
      laneEnds[lane] = cue.endTime
    }
    return { cue, lane }
  })
}

export const snapReviewTime = (
  value: number,
  candidates: readonly number[],
  disableSnap = false,
  gridMs = SNAP_GRID_MS,
  thresholdMs = SNAP_THRESHOLD_MS,
): number => {
  const safeValue = Math.max(0, Math.round(value))
  if (disableSnap) {
    return safeValue
  }
  let snapped = Math.round(safeValue / gridMs) * gridMs
  let distance = Math.abs(snapped - safeValue)
  for (const candidate of candidates) {
    const candidateDistance = Math.abs(candidate - safeValue)
    if (candidateDistance <= thresholdMs && candidateDistance < distance) {
      snapped = candidate
      distance = candidateDistance
    }
  }
  return Math.max(0, snapped)
}

export const moveReviewCue = (
  cue: ReviewCue,
  deltaMs: number,
  durationMs: number,
  candidates: readonly number[],
  disableSnap: boolean,
): ReviewCue => {
  const cueDuration = Math.max(MIN_CUE_DURATION_MS, cue.endTime - cue.startTime)
  const maxStart = Math.max(0, durationMs - cueDuration)
  const rawStart = clampNumber(cue.startTime + deltaMs, 0, maxStart)
  const startTime = clampNumber(snapReviewTime(rawStart, candidates, disableSnap), 0, maxStart)
  return { ...cue, startTime, endTime: startTime + cueDuration }
}

export const resizeReviewCue = (
  cue: ReviewCue,
  edge: 'start' | 'end',
  deltaMs: number,
  durationMs: number,
  candidates: readonly number[],
  disableSnap: boolean,
): ReviewCue => {
  if (edge === 'start') {
    const maximum = Math.max(0, cue.endTime - MIN_CUE_DURATION_MS)
    const startTime = clampNumber(
      snapReviewTime(cue.startTime + deltaMs, candidates, disableSnap),
      0,
      maximum,
    )
    return { ...cue, startTime }
  }
  const minimum = cue.startTime + MIN_CUE_DURATION_MS
  const endTime = clampNumber(
    snapReviewTime(cue.endTime + deltaMs, candidates, disableSnap),
    minimum,
    Math.max(minimum, durationMs),
  )
  return { ...cue, endTime }
}

export const reviewCuesEqual = (left: readonly ReviewCue[], right: readonly ReviewCue[]): boolean =>
  JSON.stringify(left) === JSON.stringify(right)

export const clampNumber = (value: number, minimum: number, maximum: number): number =>
  Math.min(Math.max(value, minimum), maximum)

const padNumber = (value: number, length: number) => value.toString().padStart(length, '0')
