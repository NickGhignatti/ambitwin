import type { MetricContract } from '@/types/dashboard/table.ts'

export type LimitEnd = 'min' | 'max'

/** Limit values by metric kind, then by end: `{ temperature: { min: 18, max: 27 } }`. */
export type ThresholdValues = Record<string, Partial<Record<LimitEnd, number>>>

/** One PATCH to telemetry's threshold API. */
export interface ThresholdWrite {
  path: string
  body: Record<string, unknown>
}

/** A room's capacity is its people limit, so this end is set from capacity, not its own input. */
export const CAPACITY_LIMIT = { kind: 'people_count', end: 'max' } as const

export const LIMIT_ENDS: readonly LimitEnd[] = ['min', 'max']

/** The ends a metric's limit has, in min-then-max order. */
export const endsOf = (metric: MetricContract): LimitEnd[] =>
  LIMIT_ENDS.filter((end) => metric.limit?.[end] !== undefined)

/** The metrics that take a limit, in catalog order. */
export const limitedMetrics = (catalog: MetricContract[]): MetricContract[] =>
  catalog.filter((metric) => endsOf(metric).length > 0)

/** Building-wide starting values: every end the catalog gives a starting number. */
export const defaultBuildingThresholds = (catalog: MetricContract[]): ThresholdValues => {
  const thresholds: ThresholdValues = {}
  for (const metric of limitedMetrics(catalog)) {
    for (const end of endsOf(metric)) {
      const start = metric.limit?.[end]
      if (typeof start === 'number') (thresholds[metric.kind] ??= {})[end] = start
    }
  }
  return thresholds
}

/** A copy of `values` with one end set, or removed when `value` is null. */
export const withLimitEnd = (
  values: ThresholdValues,
  kind: string,
  end: LimitEnd,
  value: number | null,
): ThresholdValues => {
  const { [end]: _removed, ...otherEnds } = values[kind] ?? {}
  const { [kind]: _metric, ...otherMetrics } = values
  const metricEnds = value === null ? otherEnds : { ...otherEnds, [end]: value }
  return Object.keys(metricEnds).length > 0 ? { ...otherMetrics, [kind]: metricEnds } : otherMetrics
}

interface ThresholdDraft {
  thresholds: ThresholdValues
  rooms: { id: string; thresholds: ThresholdValues }[]
}

const hasValues = (
  values: ThresholdValues[string] | undefined,
): values is ThresholdValues[string] => values !== undefined && Object.keys(values).length > 0

/**
 * The PATCHes that store a draft's limits: per limited metric, one for the building and one
 * batch for the rooms that set their own ends. A room's unset end falls back to the building's
 * in telemetry, so it is left out rather than copied.
 */
export const thresholdWrites = (
  catalog: MetricContract[],
  buildingId: string,
  draft: ThresholdDraft,
): ThresholdWrite[] =>
  limitedMetrics(catalog).flatMap((metric) => {
    const base = `/telemetry/thresholds/${metric.kind}/buildings/${encodeURIComponent(buildingId)}`
    const buildingValues = draft.thresholds[metric.kind]
    const roomValues = Object.fromEntries(
      draft.rooms
        .filter((room) => hasValues(room.thresholds[metric.kind]))
        .map((room) => [room.id, room.thresholds[metric.kind]]),
    )
    return [
      ...(hasValues(buildingValues) ? [{ path: base, body: buildingValues }] : []),
      ...(Object.keys(roomValues).length > 0 ? [{ path: `${base}/rooms`, body: roomValues }] : []),
    ]
  })
