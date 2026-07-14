import { describe, expect, it } from 'vitest'
import {
  assignCueLanes,
  findPlaybackCueId,
  formatReviewTime,
  moveReviewCue,
  parseReviewTime,
  resizeReviewCue,
  snapReviewTime,
  type ReviewCue,
} from './subtitleReviewModel'

const cue = (id: string, startTime: number, endTime: number): ReviewCue => ({
  id,
  startTime,
  endTime,
  text: id,
  rawText: id,
  textMode: 'plain',
  styleName: 'Default',
  layer: '0',
  actor: '',
  marginL: '0000',
  marginR: '0000',
  marginV: '0000',
  effect: '',
  hasInlineTags: false,
  sourceOrder: 0,
  isNew: false,
})

describe('subtitle review time helpers', () => {
  it('formats and parses millisecond timecodes', () => {
    expect(formatReviewTime(3_723_045)).toBe('01:02:03.045')
    expect(parseReviewTime('01:02:03,045')).toBe(3_723_045)
    expect(parseReviewTime('62:03.045')).toBe(3_723_045)
    expect(parseReviewTime('00:70:00.000')).toBeNull()
  })

  it('snaps to nearby cue boundaries before the grid', () => {
    expect(snapReviewTime(1_047, [1_050])).toBe(1_050)
    expect(snapReviewTime(1_047, [], false)).toBe(1_000)
    expect(snapReviewTime(1_047, [1_050], true)).toBe(1_047)
  })
})

describe('subtitle review timeline math', () => {
  it('assigns overlapping cues to separate lanes', () => {
    const lanes = assignCueLanes([cue('a', 0, 2_000), cue('b', 1_000, 3_000), cue('c', 3_000, 4_000)])
    expect(lanes.map((item) => item.lane)).toEqual([0, 1, 0])
  })

  it('moves a cue without changing duration or leaving the video', () => {
    expect(moveReviewCue(cue('a', 500, 1_500), 4_000, 5_000, [], true)).toMatchObject({
      startTime: 4_000,
      endTime: 5_000,
    })
  })

  it('enforces a minimum duration when resizing', () => {
    expect(resizeReviewCue(cue('a', 1_000, 2_000), 'start', 2_000, 5_000, [], true)).toMatchObject({
      startTime: 1_900,
      endTime: 2_000,
    })
  })

  it('chooses a stable active cue for playback following', () => {
    const cues = [cue('later', 1_000, 3_000), cue('earlier', 500, 2_000)]
    expect(findPlaybackCueId(cues, 1_500)).toBe('earlier')
    expect(findPlaybackCueId(cues, 1_500, 'later')).toBe('later')
    expect(findPlaybackCueId(cues, 4_000, 'later')).toBe('')
  })
})
