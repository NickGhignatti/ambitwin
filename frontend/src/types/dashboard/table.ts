/**
 * The ends a metric's limit has. A present end can be set; its number is where a new building
 * starts, `null` when it has none. An absent end cannot be set.
 */
export interface LimitContract {
  min?: number | null
  max?: number | null
}

/** How a building's figure is made from its rooms' readings: their mean or their total. */
export type Aggregation = 'average' | 'sum'

export interface MetricContract {
  kind: string
  label: string
  interface: string
  unit?: string
  source?: string
  limit?: LimitContract
  /** Absent on a room attribute (a name, a capacity), which has no building figure. */
  aggregation?: Aggregation
}

export interface TableHeader {
  key: string
  metricKey?: string
  label: string
  cellClass?: string
}

export interface TableBody {
  roomId: string
  room_name: string
  room_max_occupancy: number
  status: string
  [metric: string]: unknown
}
