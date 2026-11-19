import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import type { MetricContract } from '@/types/dashboard/table.ts'
import { DEFAULT_MAX_TEMPERATURE } from '@/utils/digital-twin/thresholds.ts'
import { defaultBuildingThresholds, thresholdWrites, withBoundValue } from './thresholds.ts'

const fixture = join(__dirname, '../../../../schemas/fixtures/metric-contract.json')
const catalog = (JSON.parse(readFileSync(fixture, 'utf8')) as { metrics: MetricContract[] }).metrics

describe('defaultBuildingThresholds', () => {
  it('fills every bound that has a default and leaves out metrics without bounds', () => {
    expect(defaultBuildingThresholds(catalog)).toEqual({
      temperature: { maxTemp: DEFAULT_MAX_TEMPERATURE, minTemp: 18 },
      airQuality: { maxCo2: 1000, maxAqi: 75 },
    })
  })
})

describe('withBoundValue', () => {
  it('sets a bound, and clearing the last bound of a metric drops the metric', () => {
    const set = withBoundValue({ temperature: { maxTemp: 24 } }, 'temperature', 'minTemp', 18)
    expect(set).toEqual({ temperature: { maxTemp: 24, minTemp: 18 } })

    expect(
      withBoundValue({ temperature: { maxTemp: 24 } }, 'temperature', 'maxTemp', null),
    ).toEqual({})
  })
})

describe('thresholdWrites', () => {
  it('writes the building per metric and only the rooms that set their own values', () => {
    const writes = thresholdWrites(catalog, 'b/1', {
      thresholds: { temperature: { maxTemp: 26, minTemp: 18 } },
      rooms: [
        { id: 'r1', thresholds: { temperature: { maxTemp: 24 }, peopleCount: { maxPeople: 30 } } },
        { id: 'r2', thresholds: { peopleCount: { maxPeople: 10 } } },
      ],
    })

    expect(writes).toEqual([
      {
        path: '/telemetry/thresholds/temperature/buildings/b%2F1',
        body: { maxTemp: 26, minTemp: 18 },
      },
      {
        path: '/telemetry/thresholds/temperature/buildings/b%2F1/rooms',
        body: { r1: { maxTemp: 24 } },
      },
      {
        path: '/telemetry/thresholds/peopleCount/buildings/b%2F1/rooms',
        body: { r1: { maxPeople: 30 }, r2: { maxPeople: 10 } },
      },
    ])
  })

  it('never writes a metric the catalog lists without bounds', () => {
    const writes = thresholdWrites(catalog, 'b1', {
      thresholds: { totalDeviceCount: { maxDevices: 5 } },
      rooms: [{ id: 'r1', thresholds: { totalDeviceCount: { maxDevices: 5 } } }],
    })

    expect(writes).toEqual([])
  })
})
