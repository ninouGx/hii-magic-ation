import { describe, expect, it } from 'vitest'
import { HttpError, shouldRetry } from './api'

describe('shouldRetry', () => {
  it('never retries a client error', () => {
    expect(shouldRetry(0, new HttpError(400, 'invalid regex'))).toBe(false)
  })

  it('retries server and network errors three times', () => {
    expect(shouldRetry(0, new HttpError(500, 'boom'))).toBe(true)
    expect(shouldRetry(2, new TypeError('network'))).toBe(true)
    expect(shouldRetry(3, new HttpError(503, 'busy'))).toBe(false)
  })
})
