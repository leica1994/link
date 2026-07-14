<template>
  <div class="review-timeline-shell">
    <div class="review-timeline-toolbar">
      <span class="review-timeline-title">时间线</span>
      <span class="review-timeline-meta">{{ cueCountLabel }} · {{ zoomLabel }}</span>
      <span class="review-timeline-spacer" />
      <button
        class="review-timeline-tool"
        type="button"
        :class="{ active: followPlayhead }"
        :aria-pressed="followPlayhead"
        aria-label="跟随播放头"
        @click="followPlayhead = !followPlayhead"
      >
        <LocateFixed :stroke-width="2.1" aria-hidden="true" />
      </button>
      <button class="review-timeline-tool" type="button" aria-label="缩小时间线" @click="changeZoom(-1)">
        <ZoomOut :stroke-width="2.1" aria-hidden="true" />
      </button>
      <button class="review-timeline-tool" type="button" aria-label="适应完整视频" @click="fitTimeline">
        <Scan :stroke-width="2.1" aria-hidden="true" />
      </button>
      <button class="review-timeline-tool" type="button" aria-label="放大时间线" @click="changeZoom(1)">
        <ZoomIn :stroke-width="2.1" aria-hidden="true" />
      </button>
    </div>

    <div
      ref="viewportRef"
      class="review-timeline-viewport"
      tabindex="0"
      aria-label="字幕时间线，点击空白处定位视频"
      @scroll="handleScroll"
    >
      <div
        class="review-timeline-track"
        :style="trackStyle"
        @pointerdown="handleTrackPointerDown"
      >
        <div class="review-timeline-ruler" aria-hidden="true">
          <span
            v-for="tick in visibleTicks"
            :key="tick.time"
            class="review-timeline-tick"
            :style="{ left: `${timeToPixel(tick.time)}px` }"
          >
            <i />
            <b>{{ tick.label }}</b>
          </span>
        </div>

        <button
          v-for="item in visibleCueLanes"
          :key="item.cue.id"
          class="review-timeline-cue"
          :class="{
            selected: item.cue.id === selectedCueId,
            active: activeCueIds.has(item.cue.id),
            dragging: dragState?.cueId === item.cue.id,
          }"
          :style="cueStyle(item.cue, item.lane)"
          type="button"
          :title="item.cue.text || '空字幕'"
          @pointerdown.stop="startCueDrag($event, item.cue, 'move')"
          @click.stop="$emit('select', item.cue.id)"
        >
          <span
            class="review-timeline-handle start"
            aria-hidden="true"
            @pointerdown.stop="startCueDrag($event, item.cue, 'start')"
          />
          <span class="review-timeline-cue-label">{{ compactCueText(item.cue.text) }}</span>
          <span
            class="review-timeline-handle end"
            aria-hidden="true"
            @pointerdown.stop="startCueDrag($event, item.cue, 'end')"
          />
        </button>

        <span
          class="review-timeline-playhead"
          :style="{ left: `${timeToPixel(currentTimeMs)}px` }"
          aria-hidden="true"
        >
          <i />
        </span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch, type CSSProperties } from 'vue'
import { LocateFixed, Scan, ZoomIn, ZoomOut } from 'lucide-vue-next'
import {
  assignCueLanes,
  clampNumber,
  formatCompactTime,
  moveReviewCue,
  resizeReviewCue,
  type ReviewCue,
} from '../subtitleReviewModel'

type DragMode = 'move' | 'start' | 'end'

type DragState = {
  cueId: string
  mode: DragMode
  originX: number
  before: ReviewCue
  current: ReviewCue
}

type TimelineTick = {
  time: number
  label: string
}

const props = defineProps<{
  cues: ReviewCue[]
  durationMs: number
  currentTimeMs: number
  selectedCueId: string
  disabled?: boolean
}>()

const emit = defineEmits<{
  select: [cueId: string]
  seek: [timeMs: number]
  previewChange: [cue: ReviewCue]
  commitChange: [change: { before: ReviewCue; after: ReviewCue }]
}>()

const viewportRef = ref<HTMLElement | null>(null)
const viewportWidth = ref(0)
const scrollLeft = ref(0)
const pixelsPerSecond = ref(28)
const followPlayhead = ref(true)
const dragState = ref<DragState | null>(null)
let resizeObserver: ResizeObserver | null = null

const cueLanes = computed(() => assignCueLanes(props.cues))
const laneCount = computed(() => Math.max(1, ...cueLanes.value.map((item) => item.lane + 1)))
const trackWidth = computed(() =>
  Math.max(viewportWidth.value, (Math.max(props.durationMs, 1) / 1_000) * pixelsPerSecond.value + 40),
)
const trackHeight = computed(() => 38 + laneCount.value * 35 + 12)
const trackStyle = computed<CSSProperties>(() => ({
  width: `${Math.ceil(trackWidth.value)}px`,
  height: `${trackHeight.value}px`,
}))
const visibleStartMs = computed(() => Math.max(0, ((scrollLeft.value - 160) / pixelsPerSecond.value) * 1_000))
const visibleEndMs = computed(
  () => ((scrollLeft.value + viewportWidth.value + 160) / pixelsPerSecond.value) * 1_000,
)
const visibleCueLanes = computed(() =>
  cueLanes.value.filter(
    ({ cue }) => cue.endTime >= visibleStartMs.value && cue.startTime <= visibleEndMs.value,
  ),
)
const activeCueIds = computed(
  () =>
    new Set(
      props.cues
        .filter((cue) => cue.startTime <= props.currentTimeMs && cue.endTime > props.currentTimeMs)
        .map((cue) => cue.id),
    ),
)
const cueCountLabel = computed(() => `${props.cues.length} 条字幕`)
const zoomLabel = computed(() => `${Math.round(pixelsPerSecond.value)} px/秒`)
const tickIntervalMs = computed(() => {
  if (pixelsPerSecond.value >= 90) return 500
  if (pixelsPerSecond.value >= 42) return 1_000
  if (pixelsPerSecond.value >= 18) return 5_000
  if (pixelsPerSecond.value < 2) return 60_000
  if (pixelsPerSecond.value < 6) return 30_000
  return 10_000
})
const visibleTicks = computed<TimelineTick[]>(() => {
  const interval = tickIntervalMs.value
  const start = Math.floor(visibleStartMs.value / interval) * interval
  const end = Math.min(props.durationMs, visibleEndMs.value)
  const ticks: TimelineTick[] = []
  for (let time = start; time <= end + interval; time += interval) {
    if (time >= 0) {
      ticks.push({ time, label: formatCompactTime(time) })
    }
  }
  return ticks
})

const timeToPixel = (timeMs: number) => (timeMs / 1_000) * pixelsPerSecond.value

const cueStyle = (cue: ReviewCue, lane: number): CSSProperties => {
  const rendered = dragState.value?.cueId === cue.id ? dragState.value.current : cue
  const left = timeToPixel(rendered.startTime)
  const width = Math.max(16, timeToPixel(rendered.endTime - rendered.startTime))
  return {
    left: `${left}px`,
    top: `${36 + lane * 35}px`,
    width: `${width}px`,
  }
}

const compactCueText = (text: string) => text.replace(/\s+/g, ' ').trim() || '空字幕'

const handleScroll = () => {
  scrollLeft.value = viewportRef.value?.scrollLeft ?? 0
}

const handleTrackPointerDown = (event: PointerEvent) => {
  if (props.disabled || event.button !== 0 || !viewportRef.value) {
    return
  }
  const rect = viewportRef.value.getBoundingClientRect()
  const x = event.clientX - rect.left + viewportRef.value.scrollLeft
  emit('seek', clampNumber((x / pixelsPerSecond.value) * 1_000, 0, props.durationMs))
}

const startCueDrag = (event: PointerEvent, cue: ReviewCue, mode: DragMode) => {
  if (props.disabled || event.button !== 0) {
    return
  }
  emit('select', cue.id)
  dragState.value = {
    cueId: cue.id,
    mode,
    originX: event.clientX,
    before: { ...cue },
    current: { ...cue },
  }
  window.addEventListener('pointermove', handleCueDrag)
  window.addEventListener('pointerup', finishCueDrag, { once: true })
}

const handleCueDrag = (event: PointerEvent) => {
  const drag = dragState.value
  if (!drag) {
    return
  }
  const deltaMs = ((event.clientX - drag.originX) / pixelsPerSecond.value) * 1_000
  const candidates = [
    props.currentTimeMs,
    ...props.cues
      .filter((cue) => cue.id !== drag.cueId)
      .flatMap((cue) => [cue.startTime, cue.endTime]),
  ]
  const next =
    drag.mode === 'move'
      ? moveReviewCue(drag.before, deltaMs, props.durationMs, candidates, event.altKey)
      : resizeReviewCue(drag.before, drag.mode, deltaMs, props.durationMs, candidates, event.altKey)
  drag.current = next
  emit('previewChange', { ...next })
}

const finishCueDrag = () => {
  window.removeEventListener('pointermove', handleCueDrag)
  const drag = dragState.value
  dragState.value = null
  if (!drag) {
    return
  }
  if (drag.before.startTime !== drag.current.startTime || drag.before.endTime !== drag.current.endTime) {
    emit('commitChange', { before: drag.before, after: drag.current })
  }
}

const changeZoom = (direction: -1 | 1) => {
  const viewport = viewportRef.value
  const centerTime = viewport
    ? ((viewport.scrollLeft + viewport.clientWidth / 2) / pixelsPerSecond.value) * 1_000
    : props.currentTimeMs
  pixelsPerSecond.value = clampNumber(
    pixelsPerSecond.value * (direction > 0 ? 1.35 : 1 / 1.35),
    0.2,
    120,
  )
  requestAnimationFrame(() => {
    if (viewport) {
      viewport.scrollLeft = Math.max(0, timeToPixel(centerTime) - viewport.clientWidth / 2)
    }
  })
}

const fitTimeline = () => {
  if (props.durationMs <= 0) {
    return
  }
  pixelsPerSecond.value = clampNumber((Math.max(viewportWidth.value - 24, 1) * 1_000) / props.durationMs, 0.2, 120)
  requestAnimationFrame(() => {
    if (viewportRef.value) viewportRef.value.scrollLeft = 0
  })
}

const ensurePlayheadVisible = (timeMs: number) => {
  const viewport = viewportRef.value
  if (!viewport || !followPlayhead.value || dragState.value) {
    return
  }
  const x = timeToPixel(timeMs)
  const leftBoundary = viewport.scrollLeft + viewport.clientWidth * 0.18
  const rightBoundary = viewport.scrollLeft + viewport.clientWidth * 0.82
  if (x < leftBoundary || x > rightBoundary) {
    viewport.scrollTo({ left: Math.max(0, x - viewport.clientWidth * 0.35), behavior: 'smooth' })
  }
}

watch(() => props.currentTimeMs, ensurePlayheadVisible)

watch(
  () => props.selectedCueId,
  (cueId) => {
    const cue = props.cues.find((item) => item.id === cueId)
    const viewport = viewportRef.value
    if (!cue || !viewport) return
    const x = timeToPixel(cue.startTime)
    if (x < viewport.scrollLeft || x > viewport.scrollLeft + viewport.clientWidth - 80) {
      viewport.scrollTo({ left: Math.max(0, x - viewport.clientWidth * 0.25), behavior: 'smooth' })
    }
  },
)

onMounted(() => {
  if (viewportRef.value) {
    resizeObserver = new ResizeObserver(([entry]) => {
      viewportWidth.value = entry?.contentRect.width ?? 0
    })
    resizeObserver.observe(viewportRef.value)
    viewportWidth.value = viewportRef.value.clientWidth
  }
})

onBeforeUnmount(() => {
  resizeObserver?.disconnect()
  window.removeEventListener('pointermove', handleCueDrag)
  window.removeEventListener('pointerup', finishCueDrag)
})
</script>

<style scoped>
.review-timeline-shell {
  min-width: 0;
  border-top: 1px solid var(--hairline);
  background: color-mix(in srgb, var(--bg-surface) 72%, var(--bg));
}

.review-timeline-toolbar {
  min-height: 42px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px 6px 14px;
  border-bottom: 1px solid var(--hairline);
}

.review-timeline-title {
  color: var(--text);
  font-size: 13px;
  font-weight: 750;
}

.review-timeline-meta {
  color: var(--text-subtle);
  font-size: 11px;
}

.review-timeline-spacer { flex: 1; }

.review-timeline-tool {
  width: 30px;
  height: 30px;
  border: 0;
  border-radius: 9px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  background: transparent;
  cursor: pointer;
}

.review-timeline-tool:hover,
.review-timeline-tool.active {
  color: var(--text);
  background: var(--bg-surface-hover);
}

.review-timeline-tool svg { width: 16px; height: 16px; }

.review-timeline-viewport {
  width: 100%;
  min-height: 120px;
  max-height: 235px;
  overflow: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--border) transparent;
  outline: none;
}

.review-timeline-viewport:focus-visible {
  box-shadow: inset 0 0 0 2px var(--accent);
}

.review-timeline-track {
  position: relative;
  min-width: 100%;
  background-image: linear-gradient(to bottom, transparent 34px, var(--hairline) 35px, transparent 36px);
  user-select: none;
  touch-action: none;
}

.review-timeline-ruler {
  position: absolute;
  inset: 0 0 auto;
  height: 35px;
  pointer-events: none;
}

.review-timeline-tick {
  position: absolute;
  top: 0;
  height: 35px;
  color: var(--text-subtle);
  font-size: 10px;
  transform: translateX(-1px);
}

.review-timeline-tick i {
  display: block;
  width: 1px;
  height: 9px;
  background: var(--border);
}

.review-timeline-tick b {
  display: block;
  margin: 3px 0 0 4px;
  font-weight: 600;
  white-space: nowrap;
}

.review-timeline-cue {
  position: absolute;
  height: 29px;
  min-width: 16px;
  padding: 0 8px;
  border: 1px solid color-mix(in srgb, var(--accent) 36%, var(--border));
  border-radius: 7px;
  background: color-mix(in srgb, var(--accent-soft) 70%, var(--bg-surface));
  color: var(--text);
  cursor: grab;
  overflow: hidden;
  text-align: left;
  touch-action: none;
}

.review-timeline-cue:hover { border-color: var(--accent); }
.review-timeline-cue.active { background: color-mix(in srgb, var(--accent-soft) 90%, var(--bg-surface)); }
.review-timeline-cue.selected {
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent);
  z-index: 3;
}
.review-timeline-cue.dragging { cursor: grabbing; opacity: 0.9; z-index: 5; }

.review-timeline-cue-label {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 11px;
  line-height: 27px;
}

.review-timeline-handle {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 7px;
  cursor: ew-resize;
  z-index: 2;
}
.review-timeline-handle.start { left: 0; }
.review-timeline-handle.end { right: 0; }

.review-timeline-playhead {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 1px;
  background: #d94b3d;
  pointer-events: none;
  z-index: 8;
}

.review-timeline-playhead i {
  position: absolute;
  top: 0;
  left: -4px;
  width: 9px;
  height: 9px;
  border-radius: 2px 2px 5px 5px;
  background: #d94b3d;
}

.review-timeline-shell.compact .review-timeline-toolbar {
  min-height: 38px;
  padding-block: 4px;
}

.review-timeline-shell.compact .review-timeline-tool {
  width: 28px;
  height: 28px;
}

.review-timeline-shell.compact .review-timeline-viewport {
  min-height: 88px;
  max-height: 156px;
}
</style>
