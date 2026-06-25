import { describe, expect, it } from 'vitest'
import { detectPort, detectPreviewTarget } from './port-detect'

describe('detectPort', () => {
  it('detects common local dev server messages', () => {
    expect(detectPort('  Local:   http://localhost:5173/')).toBe(5173)
    expect(detectPort('server listening on 0.0.0.0:3000')).toBe(3000)
    expect(detectPort('ready on http://127.0.0.1:8080')).toBe(8080)
    expect(detectPort('started server on port: 4321')).toBe(4321)
    expect(detectPort('running at :9000')).toBe(9000)
  })

  it('normalizes preview URLs and rejects invalid ports', () => {
    expect(detectPreviewTarget('Local: http://0.0.0.0:5173')?.url).toBe(
      'http://localhost:5173',
    )
    expect(detectPort('Local: http://localhost:99999')).toBeNull()
    expect(detectPort('no server here')).toBeNull()
  })
})
