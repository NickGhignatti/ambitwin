import { describe, expect, it } from 'vitest'
import type { TableBody, TableHeader } from '@/types/dashboard/table.ts'
import { buildRows, sensorKinds } from './tableRows.ts'

const room = (roomId: string, capacity: number): TableBody => ({
  roomId,
  room_name: roomId,
  room_max_occupancy: capacity,
  status: '',
})

const header = (metricKey: string): TableHeader => ({ key: metricKey, metricKey, label: metricKey })

const reading = (roomId: string, value: number) => ({ roomId, timestamp: 0, value })

describe('sensorKinds', () => {
  it('fetches every telemetry column, including metrics the table has never heard of', () => {
    expect(sensorKinds([header('temperature'), header('total_device_count')])).toEqual(
      expect.arrayContaining(['temperature', 'total_device_count']),
    )
  })

  it('never fetches what the room itself carries', () => {
    expect(sensorKinds([header('room_name'), header('room_max_occupancy'), header('status')])).toEqual(
      ['people_count'],
    )
  })

  it('always fetches people count, because status is derived from it', () => {
    expect(sensorKinds([header('temperature')])).toContain('people_count')
  })

  it('lists each metric once', () => {
    expect(sensorKinds([header('people_count')])).toEqual(['people_count'])
  })
})

describe('buildRows', () => {
  it('fills a column for any metric, keyed by the metric', () => {
    const [row] = buildRows([room('r1', 10)], [header('total_device_count')], {
      total_device_count: [reading('r1', 7)],
    })
    expect(row!.total_device_count).toBe(7)
  })

  it('rounds a reading to one decimal', () => {
    const [row] = buildRows([room('r1', 10)], [header('temperature')], {
      temperature: [reading('r1', 21.456)],
    })
    expect(row!.temperature).toBe(21.5)
  })

  it('shows a placeholder for a room with no reading yet', () => {
    const [row] = buildRows([room('r1', 10)], [header('ratio_device_count')], {
      ratio_device_count: [reading('r2', 3)],
    })
    expect(row!.ratio_device_count).toBe('--')
  })

  it('derives status from the live people count against max occupancy', () => {
    const [row] = buildRows([room('r1', 10)], [header('status')], {
      people_count: [reading('r1', 10)],
    })
    expect(row!.status).toBe('dashboard.table.rooms.status.full')
  })

  it('treats a room with no people reading as empty', () => {
    const [row] = buildRows([room('r1', 10)], [header('status')], {})
    expect(row!.status).toBe('dashboard.table.rooms.status.empty')
  })

  it('leaves room columns as the room provided them', () => {
    const [row] = buildRows([room('r1', 10)], [header('room_name'), header('room_max_occupancy')], {})
    expect(row).toMatchObject({ room_name: 'r1', room_max_occupancy: 10 })
  })
})
