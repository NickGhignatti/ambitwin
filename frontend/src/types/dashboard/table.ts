export interface BoundContract {
  key: string
  field: string
  label: string
  unit: string | null
  direction: 'high' | 'low'
}

export interface MetricContract {
  kind: string
  label: string
  interface: string
  unit?: string
  source?: string
  bounds?: BoundContract[]
}

export interface TableHeader {
  key: string
  metricKey?: string
  label: string
  cellClass?: string
}

export interface TableBody {
  roomId: string
  roomName: string
  roomMaxOccupancy: number
  status: string
  [metric: string]: unknown
}
