import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import type { MetricContract } from '@/types/dashboard/table.ts'
import { defaultBuildingThresholds, thresholdWrites, withLimitEnd } from './thresholds.ts'

const fixture = join(__dirname, '../../../../schemas/fixtures/metric-contract.json')
const catalog = (JSON.parse(readFileSync(fixture, 'utf8')) as { metrics: MetricContract[] }).metrics

describe('defaultBuildingThresholds', () => {
  it('starts every end the catalog gives a number, and skips ends with none', () => {
    expect(defaultBuildingThresholds(catalog)).toEqual({
      temperature: { min: 18, max: 27 },
      co2: { max: 1000 },
      indoor_aqi: { max: 75 },
      humidity: { min: 30, max: 60 },
    })
  })
})

describe('withLimitEnd', () => {
  it('sets an end, and clearing the last end of a metric drops the metric', () => {
    expect(withLimitEnd({ temperature: { max: 24 } }, 'temperature', 'min', 18)).toEqual({
      temperature: { max: 24, min: 18 },
    })
    expect(withLimitEnd({ temperature: { max: 24 } }, 'temperature', 'max', null)).toEqual({})
  })
})

describe('thresholdWrites', () => {
  it('writes the building per metric and only the rooms that set their own ends', () => {
    const writes = thresholdWrites(catalog, 'b/1', {
      thresholds: { temperature: { min: 18, max: 27 } },
      rooms: [
        { id: 'r1', thresholds: { temperature: { max: 24 }, people_count: { max: 30 } } },
        { id: 'r2', thresholds: { people_count: { max: 10 } } },
      ],
    })

    expect(writes).toEqual([
      {
        path: '/telemetry/thresholds/temperature/buildings/b%2F1',
        body: { min: 18, max: 27 },
      },
      {
        path: '/telemetry/thresholds/temperature/buildings/b%2F1/rooms',
        body: { r1: { max: 24 } },
      },
      {
        path: '/telemetry/thresholds/people_count/buildings/b%2F1/rooms',
        body: { r1: { max: 30 }, r2: { max: 10 } },
      },
    ])
  })

  it('never writes a metric the catalog lists without a limit', () => {
    const writes = thresholdWrites(catalog, 'b1', {
      thresholds: { total_device_count: { max: 5 } },
      rooms: [{ id: 'r1', thresholds: { total_device_count: { max: 5 } } }],
    })

    expect(writes).toEqual([])
  })
})
