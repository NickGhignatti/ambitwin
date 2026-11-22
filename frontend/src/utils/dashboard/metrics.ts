import type { TableHeader } from '@/types/dashboard/table.ts'
import type { MetricContract } from '@/types/dashboard/table.ts'

export const METRIC_I18N_KEY: Record<string, string> = {
  room_name: 'model.rooms.editRoom.name',
  room_max_occupancy: 'dashboard.table.headers.capacity',
  people_count: 'model.rooms.occupancy',
  temperature: 'model.rooms.temperature',
  co2: 'dashboard.table.headers.co2',
  indoor_aqi: 'dashboard.table.headers.indoorAqi',
  humidity: 'dashboard.table.headers.humidity',
  total_device_count: 'dashboard.table.headers.connectedDevices',
  ratio_device_count: 'dashboard.table.headers.estimatedPeople',
  estimated_occupancy: 'dashboard.table.headers.estimatedOccupancy',
  status: 'dashboard.table.headers.status',
}

export const METRIC_CELL_CLASS: Record<string, string> = {
  room_name: 'min-w-48',
  temperature: 'whitespace-nowrap',
  room_max_occupancy: 'tabular-nums',
  people_count: 'min-w-44',
}

/** Builds a TableHeader from a raw metric key string (as stored in dashboard preferences). */
export const metricKeyToHeader = (metricKey: string): TableHeader => ({
  key: metricKey,
  metricKey,
  label: METRIC_I18N_KEY[metricKey] ?? metricKey,
  ...(METRIC_CELL_CLASS[metricKey] ? { cellClass: METRIC_CELL_CLASS[metricKey] } : {}),
})

export const headerId = (h: TableHeader): string => h.metricKey ?? h.key

export const headerFromMetric = (metric: MetricContract, cellClass?: string): TableHeader => ({
  key: metric.kind,
  metricKey: metric.kind,
  label: METRIC_I18N_KEY[metric.kind] ?? metric.label,
  ...(cellClass ? { cellClass } : {}),
})

/** Keys a header by its metric, so it matches the row field `buildRows` fills. */
export const enrichHeader = (h: TableHeader): TableHeader => {
  const mk = h.metricKey ?? h.key
  return {
    ...h,
    key: mk,
    metricKey: mk,
    label: METRIC_I18N_KEY[mk] ?? h.label,
  }
}
