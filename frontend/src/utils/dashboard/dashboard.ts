import type { ApiDataPoint } from '@/composables/commons/useSensorData.ts'
import type { Aggregation, MetricContract, TableBody } from '@/types/dashboard/table.ts'
import type { Room } from '@/types/digital-twin/building.ts'
import { getStatusByOccupants, isAlertStatus } from '@/utils/dashboard/status.ts'

export type RowFilter = 'all' | 'occupied' | 'alerts'
export type AqiBand = 'good' | 'moderate' | 'poor'

export interface BuildingSummary {
  rooms: number
  people: number
  capacity: number
  occupancy: number | null
  alerts: number
}

const EMPTY_STATUS = 'dashboard.table.rooms.status.empty'

const round = (value: number) => Math.round(value * 10) / 10

function valuesFor(points: ApiDataPoint[] | undefined, roomIds: Set<string>): number[] {
  return (points ?? [])
    .filter((point) => roomIds.has(point.roomId))
    .map((point) => point.value)
    .filter((value): value is number => typeof value === 'number')
}

const average = (values: number[]) =>
  values.length ? round(values.reduce((sum, v) => sum + v, 0) / values.length) : null

/** The building-wide figures shown above the table. */
export function summarize(
  rows: TableBody[],
  readings: Record<string, ApiDataPoint[]>,
): BuildingSummary {
  const roomIds = new Set(rows.map((row) => row.roomId))
  const people = valuesFor(readings.people_count, roomIds).reduce((sum, v) => sum + v, 0)
  const capacity = rows.reduce((sum, row) => sum + (row.room_max_occupancy || 0), 0)

  return {
    rooms: rows.length,
    people,
    capacity,
    occupancy: capacity > 0 ? people / capacity : null,
    alerts: rows.filter((row) => isAlertStatus(row.status)).length,
  }
}

/** One building figure from its rooms' readings, added up the way the metric declares. */
export function aggregate(
  points: ApiDataPoint[] | undefined,
  roomIds: Set<string>,
  aggregation: Aggregation,
): number | null {
  const values = valuesFor(points, roomIds)
  if (aggregation === 'average') return average(values)
  return values.length ? round(values.reduce((sum, v) => sum + v, 0)) : null
}

/** Cards that are no single metric: they count the building's rooms. */
export const ROOMS_CARD = 'rooms'
export const ALERTS_CARD = 'alerts'

export type CardMetric = MetricContract & { aggregation: Aggregation }

/** The catalog metrics a card can show: those that declare how their rooms add up. */
export const cardMetrics = (catalog: MetricContract[]): CardMetric[] =>
  catalog.filter((metric): metric is CardMetric => metric.aggregation !== undefined)

/** A building's cards before anyone chooses: rooms, each metric it reports, then alerts. */
export const defaultCards = (catalog: MetricContract[], reportedKinds: Set<string>): string[] => [
  ROOMS_CARD,
  ...cardMetrics(catalog)
    .filter((metric) => reportedKinds.has(metric.kind))
    .map((metric) => metric.kind),
  ALERTS_CARD,
]

export function filterRows(rows: TableBody[], query: string, filter: RowFilter): TableBody[] {
  const needle = query.trim().toLowerCase()
  return rows.filter((row) => {
    if (filter === 'occupied' && row.status === EMPTY_STATUS) return false
    if (filter === 'alerts' && !isAlertStatus(row.status)) return false
    return (
      !needle ||
      row.room_name.toLowerCase().includes(needle) ||
      row.roomId.toLowerCase().includes(needle)
    )
  })
}

/** A building's rooms before any telemetry: nobody in them yet. */
export const roomsToRows = (rooms: Room[]): TableBody[] =>
  rooms.map((room) => ({
    roomId: room.id,
    room_name: room.name?.trim() || room.id,
    room_max_occupancy: room.capacity,
    status: getStatusByOccupants(0, room.capacity),
  }))

/** Same bands the 3D twin colours rooms by. */
export function aqiBand(value: number | null): AqiBand | null {
  if (value === null) return null
  if (value < 50) return 'good'
  if (value < 100) return 'moderate'
  return 'poor'
}
