import { ref, computed } from 'vue'
import { makeRequest, makeRequestWithRetry } from '@/composables/commons/useApi.ts'
import { useBuildingsStore } from '@/stores/digital-twin/buildings.ts'
import type { BuildingDraft, RoomDraft } from '@/types/administration/buildingDraft.ts'
import type { MetricContract } from '@/types/dashboard/table.ts'
import {
  CAPACITY_BOUND,
  boundedMetrics,
  defaultBuildingThresholds,
  thresholdWrites,
} from '@/utils/administration/thresholds.ts'

const fetchCatalog = async (): Promise<MetricContract[]> => {
  const response = await makeRequest('/telemetry/contracts')
  if (!response.ok) throw new Error(`metric catalog request failed: ${response.status}`)
  return (await response.json()).metrics ?? []
}

export function useBuildingDraft() {
  const buildingsStore = useBuildingsStore()
  const draft = ref<BuildingDraft | null>(null)
  const catalog = ref<MetricContract[]>([])
  const isSubmitting = ref(false)
  const hasData = computed(() => draft.value !== null)
  const metricsWithBounds = computed(() => boundedMetrics(catalog.value))

  // the form offers only the thresholds telemetry declares, so it waits for the catalog
  const loadFromJson = async (raw: any): Promise<void> => {
    if (catalog.value.length === 0) catalog.value = await fetchCatalog()
    draft.value = {
      name: raw.name?.trim() || '',
      thresholds: defaultBuildingThresholds(catalog.value),
      rooms: (raw.rooms ?? []).map((room: any) => ({
        id: room.id,
        name: room.name?.trim() || room.id,
        capacity: room.capacity ?? 0,
        position: room.position,
        dimensions: room.dimensions,
        color: room.color,
        thresholds: { [CAPACITY_BOUND.kind]: { [CAPACITY_BOUND.key]: room.capacity ?? 0 } },
      })),
    }
  }

  const updateBuilding = (patch: Partial<Omit<BuildingDraft, 'rooms'>>): void => {
    if (!draft.value) return
    Object.assign(draft.value, patch)
  }

  const updateRoom = (roomId: string, patch: Partial<RoomDraft>): void => {
    if (!draft.value) return
    const room = draft.value.rooms.find((r) => r.id === roomId)
    if (room) Object.assign(room, patch)
  }

  const clear = (): void => {
    draft.value = null
  }

  const submit = async (domainName: string): Promise<void> => {
    if (!draft.value) return
    isSubmitting.value = true

    try {
      const twinPayload = {
        name: draft.value.name,
        rooms: draft.value.rooms.map((r) => ({
          id: r.id,
          name: r.name,
          capacity: r.capacity,
          position: r.position,
          dimensions: r.dimensions,
          color: r.color,
        })),
      }

      const buildingId = await buildingsStore.register(twinPayload, domainName)

      // at most two writes per metric, each touching a different row, so they overlap
      await Promise.all(
        thresholdWrites(catalog.value, buildingId, draft.value).map(({ path, body }) =>
          makeRequestWithRetry(path, 'PATCH', { body: JSON.stringify(body) }),
        ),
      )
    } finally {
      isSubmitting.value = false
    }
  }

  return {
    draft,
    metricsWithBounds,
    hasData,
    isSubmitting,
    loadFromJson,
    updateBuilding,
    updateRoom,
    clear,
    submit,
  }
}
