import { describe, expect, it } from 'vitest'

import { formatPathForDisplay } from './pathDisplay'

describe('formatPathForDisplay', () => {
  it('removes the Windows device prefix from drive paths', () => {
    expect(formatPathForDisplay(String.raw`\\?\F:\videos\sample.mp4`)).toBe(
      String.raw`F:\videos\sample.mp4`,
    )
  })

  it('restores a regular UNC path after removing the device prefix', () => {
    expect(formatPathForDisplay(String.raw`\\?\UNC\server\share\sample.mp4`)).toBe(
      String.raw`\\server\share\sample.mp4`,
    )
  })

  it('leaves regular paths unchanged', () => {
    expect(formatPathForDisplay(String.raw`D:\videos\sample.mp4`)).toBe(String.raw`D:\videos\sample.mp4`)
  })
})
