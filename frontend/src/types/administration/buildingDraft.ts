import type { Coordinates, RoomDimensions } from '../digital-twin/building.ts'
import type { ThresholdValues } from '@/utils/administration/thresholds.ts'

export interface RoomDraft {
  id: string
  name: string
  capacity: number
  /** Only the values this room sets itself; the rest fall back to the building's. */
  thresholds: ThresholdValues
  position: Coordinates
  dimensions: RoomDimensions
  color?: string
}

export interface BuildingDraft {
  name: string
  thresholds: ThresholdValues
  rooms: RoomDraft[]
}
