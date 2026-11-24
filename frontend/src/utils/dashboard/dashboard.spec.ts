import { describe, expect, it } from 'vitest'
import type { TableBody } from '@/types/dashboard/table.ts'
import type { Room } from '@/types/digital-twin/building.ts'
import { aggregate, aqiBand, filterRows, roomsToRows, summarize } from './dashboard.ts'

const STATUS = 'dashboard.table.rooms.status'

const room = (roomId: string, capacity: number, status = `${STATUS}.empty`): TableBody => ({
  roomId,
  room_name: `Room ${roomId}`,
  room_max_occupancy: capacity,
  status,
})

const rooms = [
  room('a', 10, `${STATUS}.normal`),
  room('b', 20, `${STATUS}.full`),
  room('c', 10, `${STATUS}.empty`),
]

const point = (roomId: string, value: number) => ({ timestamp: 0, roomId, value })

describe('the building summary above the table', () => {
  it('adds up people against the capacity of every room', () => {
    const summary = summarize(rooms, { people_count: [point('a', 4), point('b', 20)] })
    expect(summary).toMatchObject({ rooms: 3, people: 24, capacity: 40, occupancy: 0.6 })
  })

  it('averages a measure over the rooms that report it', () => {
    const roomIds = new Set(rooms.map((row) => row.roomId))
    expect(aggregate([point('a', 20), point('b', 23)], roomIds, 'average')).toBe(21.5)
    expect(aggregate([point('c', 42)], roomIds, 'average')).toBe(42)
  })

  it('has no figure and no occupancy when nothing reports', () => {
    expect(aggregate(undefined, new Set(), 'average')).toBeNull()
    expect(summarize([], {}).occupancy).toBeNull()
  })

  it('counts full and overcrowded rooms as alerts', () => {
    expect(summarize([...rooms, room('d', 5, `${STATUS}.overcrowded`)], {}).alerts).toBe(2)
  })

  it('ignores readings for rooms that are not in the building', () => {
    expect(summarize(rooms, { people_count: [point('zz', 99)] }).people).toBe(0)
  })
})

describe('filtering the table rows', () => {
  it('keeps every row with no query and no filter', () => {
    expect(filterRows(rooms, '', 'all')).toHaveLength(3)
  })

  it('matches the query against room name and id, ignoring case', () => {
    expect(filterRows(rooms, 'ROOM B', 'all').map((r) => r.roomId)).toEqual(['b'])
    expect(filterRows(rooms, 'c', 'all').map((r) => r.roomId)).toEqual(['c'])
  })

  it('keeps only rooms with people in them', () => {
    expect(filterRows(rooms, '', 'occupied').map((r) => r.roomId)).toEqual(['a', 'b'])
  })

  it('keeps only rooms in alert', () => {
    expect(filterRows(rooms, '', 'alerts').map((r) => r.roomId)).toEqual(['b'])
  })
})

describe('the rows a building starts the table with', () => {
  const twinRoom = (id: string, name: string): Room => ({
    id,
    name,
    capacity: 30,
    position: { x: 0, y: 0, z: 0 },
    dimensions: { width: 1, height: 1, depth: 1 },
  })

  it('has one empty row per room, named after it', () => {
    expect(roomsToRows([twinRoom('r1', ' Lab ')])).toEqual([
      { roomId: 'r1', room_name: 'Lab', room_max_occupancy: 30, status: `${STATUS}.empty` },
    ])
  })

  it('falls back to the room id for a blank name', () => {
    expect(roomsToRows([twinRoom('r2', '  ')])[0]!.room_name).toBe('r2')
  })
})

describe('the band an air quality index falls in', () => {
  it.each([
    [null, null],
    [0, 'good'],
    [49, 'good'],
    [50, 'moderate'],
    [99, 'moderate'],
    [100, 'poor'],
  ])('%s reads as %s', (value, band) => {
    expect(aqiBand(value)).toBe(band)
  })
})
