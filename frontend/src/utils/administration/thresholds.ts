import type { MetricContract } from '@/types/dashboard/table.ts'
import { DEFAULT_MAX_TEMPERATURE } from '@/utils/digital-twin/thresholds.ts'

/** Threshold values by metric kind, then by bound key: `{ temperature: { maxTemp: 27 } }`. */
export type ThresholdValues = Record<string, Record<string, number>>

/** One PATCH to telemetry's threshold API. */
export interface ThresholdWrite {
  path: string
  body: Record<string, unknown>
}

/** A room's capacity is its people limit, so this bound is set from capacity, not its own input. */
export const CAPACITY_BOUND = { kind: 'peopleCount', key: 'maxPeople' } as const

// a bound missing here starts empty, and an empty bound is never sent
const DEFAULT_BOUND_VALUES: Record<string, number> = {
  minTemp: 18,
  maxTemp: DEFAULT_MAX_TEMPERATURE,
  maxAqi: 75,
  maxCo2: 1000,
}

/** The metrics that take thresholds, in catalog order. */
export const boundedMetrics = (catalog: MetricContract[]): MetricContract[] =>
  catalog.filter((metric) => (metric.bounds ?? []).length > 0)

/** Building-wide starting values: every catalog bound that has a known default. */
export const defaultBuildingThresholds = (catalog: MetricContract[]): ThresholdValues => {
  const thresholds: ThresholdValues = {}
  for (const metric of boundedMetrics(catalog)) {
    for (const bound of metric.bounds!) {
      const defaultValue = DEFAULT_BOUND_VALUES[bound.key]
      if (defaultValue !== undefined) (thresholds[metric.kind] ??= {})[bound.key] = defaultValue
    }
  }
  return thresholds
}

/** A copy of `values` with one bound set, or removed when `value` is null. */
export const withBoundValue = (
  values: ThresholdValues,
  kind: string,
  key: string,
  value: number | null,
): ThresholdValues => {
  const { [key]: _removed, ...otherBounds } = values[kind] ?? {}
  const { [kind]: _metric, ...otherMetrics } = values
  const metricBounds = value === null ? otherBounds : { ...otherBounds, [key]: value }
  return Object.keys(metricBounds).length > 0
    ? { ...otherMetrics, [kind]: metricBounds }
    : otherMetrics
}

interface ThresholdDraft {
  thresholds: ThresholdValues
  rooms: { id: string; thresholds: ThresholdValues }[]
}

const hasValues = (values: Record<string, number> | undefined): values is Record<string, number> =>
  values !== undefined && Object.keys(values).length > 0

/**
 * The PATCHes that store a draft's thresholds: per bounded metric, one for the building and one
 * batch for the rooms that set their own values. A room without its own value falls back to the
 * building's in telemetry, so it is left out rather than copied.
 */
export const thresholdWrites = (
  catalog: MetricContract[],
  buildingId: string,
  draft: ThresholdDraft,
): ThresholdWrite[] =>
  boundedMetrics(catalog).flatMap((metric) => {
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
