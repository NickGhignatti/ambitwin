<script setup lang="ts">
import { computed, ref, toRef } from 'vue'
import { useI18n } from 'vue-i18n'
import type { MetricContract, TableBody } from '@/types/dashboard/table.ts'
import {
  ALERTS_CARD,
  ROOMS_CARD,
  aggregate,
  aqiBand,
  cardMetrics,
  summarize,
} from '@/utils/dashboard/dashboard.ts'
import { buildRows } from '@/utils/dashboard/tableRows.ts'
import { METRIC_DISPLAY } from '@/utils/dashboard/metricDisplay.ts'
import { METRIC_I18N_KEY } from '@/utils/dashboard/metrics.ts'
import type { Tone } from '@/utils/commons/tone.ts'
import { useBuildingReadings } from '@/composables/dashboard/useBuildingSensor.ts'
import { useConnectedDevices } from '@/composables/dashboard/useConnectedDevices.ts'
import { useMetricCatalog } from '@/composables/dashboard/metricCatalog.ts'
import { useDashboardCards } from '@/composables/dashboard/useDashboardCards.ts'
import { useUserPermissions } from '@/composables/authentication/useUserPermissions.ts'
import { useBuildingsStore } from '@/stores/digital-twin/buildings.ts'
import KpiCard from '@/components/dashboard/kpi/KpiCard.vue'
import AddColumnPanel from '@/components/dashboard/table/AddColumnPanel.vue'
import ProgressBar from '@/components/commons/base/ProgressBar.vue'
import BaseBadge from '@/components/commons/base/BaseBadge.vue'
import BaseButton from '@/components/commons/base/BaseButton.vue'
import BaseIcon from '@/components/commons/base/BaseIcon.vue'
import IconButton from '@/components/commons/base/IconButton.vue'

const props = defineProps<{ buildingId?: string; rooms: TableBody[]; loading: boolean }>()

const { t, n } = useI18n()

const EMPTY = '—'
const PEOPLE_CARD = 'people_count'
const DEVICES_CARD = 'total_device_count'
const BAND_TONE: Record<string, Tone> = { good: 'success', moderate: 'warning', poor: 'danger' }
// a card names the building figure ("avg. temperature"), a column the room reading
const CARD_LABEL: Record<string, string> = {
  [ROOMS_CARD]: 'dashboard.kpi.rooms',
  [ALERTS_CARD]: 'dashboard.kpi.alerts',
  [PEOPLE_CARD]: 'dashboard.kpi.occupancy',
  [DEVICES_CARD]: 'dashboard.kpi.devices',
  temperature: 'dashboard.kpi.temperature',
  indoor_aqi: 'dashboard.kpi.airQuality',
}

const buildingId = toRef(props, 'buildingId')
const catalog = useMetricCatalog()
const metricByKind = computed(
  () => new Map(cardMetrics(catalog.metrics.value).map((metric) => [metric.kind, metric])),
)
// the alert count reads each room's occupancy status, so people are read even without their card
const { readings, isLoading: readingsLoading } = useBuildingReadings(
  buildingId,
  computed(() => [...new Set([PEOPLE_CARD, ...metricByKind.value.keys()])]),
)
const reportedKinds = computed(
  () => new Set(Object.keys(readings.value).filter((kind) => readings.value[kind]?.length)),
)
const editor = useDashboardCards(buildingId, catalog.metrics, reportedKinds)
const { devices, stale: devicesStale, isLoading: devicesLoading } = useConnectedDevices(buildingId)

const { canEdit } = useUserPermissions()
const buildingsStore = useBuildingsStore()
const canEditCards = computed(() =>
  canEdit(buildingId.value ? (buildingsStore.getById(buildingId.value)?.domains ?? []) : []),
)

const isLoading = computed(() => props.loading || readingsLoading.value || catalog.isLoading.value)
const summary = computed(() =>
  summarize(buildRows(props.rooms, [], readings.value), readings.value),
)
const roomIds = computed(() => new Set(props.rooms.map((room) => room.roomId)))

function figureOf(card: string): number | null {
  const metric = metricByKind.value.get(card)
  return metric ? aggregate(readings.value[card], roomIds.value, metric.aggregation) : null
}

function labelOf(card: string): string {
  const key = CARD_LABEL[card] ?? METRIC_I18N_KEY[card]
  return key ? t(key) : (metricByKind.value.get(card)?.label ?? card)
}

const unitOf = (card: string) =>
  METRIC_DISPLAY[card]?.unit?.trim() ?? metricByKind.value.get(card)?.unit ?? ''

function captionOf(card: string): string {
  if (figureOf(card) === null) return t('dashboard.kpi.noReading')
  const isTotal = metricByKind.value.get(card)?.aggregation === 'sum'
  return isTotal ? t('dashboard.kpi.totalCaption') : t('dashboard.kpi.averageCaption')
}

const devicesCaption = computed(() => {
  if (devices.value === null) return t('dashboard.kpi.noReading')
  return devicesStale.value ? t('dashboard.kpi.devicesStale') : t('dashboard.kpi.devicesCaption')
})

const isAdding = ref(false)
const addableCards = computed<MetricContract[]>(() =>
  [ROOMS_CARD, ...metricByKind.value.keys(), ALERTS_CARD]
    .filter((card) => !editor.cards.value.includes(card))
    .map((card) => ({ kind: card, label: labelOf(card), interface: '', unit: unitOf(card) })),
)

function addCard(metric: MetricContract) {
  editor.add(metric.kind)
  isAdding.value = false
}

function stopEditing(finish: () => unknown) {
  isAdding.value = false
  void finish()
}

const dragFrom = ref<number | null>(null)
const dragOver = ref<number | null>(null)

function onDragStart(index: number, event: DragEvent) {
  dragFrom.value = index
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
}

function onDrop(index: number) {
  if (dragFrom.value !== null && dragFrom.value !== index) editor.move(dragFrom.value, index)
  resetDrag()
}

function resetDrag() {
  dragFrom.value = null
  dragOver.value = null
}
</script>

<template>
  <section class="space-y-3" :aria-label="t('dashboard.kpi.title')">
    <div v-if="canEditCards" class="flex flex-wrap items-center justify-end gap-2">
      <template v-if="editor.isEditing.value">
        <p v-if="editor.saveFailed.value" role="alert" class="mr-auto text-body-sm text-error">
          {{ t('dashboard.kpi.saveFailed') }}
        </p>
        <BaseButton
          size="sm"
          variant="ghost"
          icon="arrow-counter-clockwise"
          :disabled="editor.isSaving.value"
          @click="stopEditing(editor.cancel)"
        >
          {{ t('commons.cancel') }}
        </BaseButton>
        <BaseButton
          size="sm"
          variant="primary"
          icon="floppy-disk"
          :loading="editor.isSaving.value"
          @click="stopEditing(editor.save)"
        >
          {{ t('commons.save') }}
        </BaseButton>
      </template>
      <BaseButton v-else size="sm" variant="ghost" icon="sliders-horizontal" @click="editor.edit">
        {{ t('dashboard.kpi.editCards') }}
      </BaseButton>
    </div>

    <AddColumnPanel
      v-if="isAdding && editor.isEditing.value"
      :title="t('dashboard.kpi.addCard')"
      :empty-message="t('dashboard.kpi.allCardsShown')"
      :metrics="addableCards"
      :is-fetching="catalog.isLoading.value"
      :failed="catalog.failed.value"
      @select="addCard"
      @close="isAdding = false"
      @retry="catalog.load"
    />

    <ul class="grid grid-cols-[repeat(auto-fill,minmax(13rem,1fr))] gap-4">
      <li
        v-for="(card, index) in editor.cards.value"
        :key="card"
        class="flex flex-col gap-2"
        :class="[
          editor.isEditing.value &&
            'cursor-grab rounded-2xl outline-2 outline-offset-2 outline-primary/40 outline-dashed',
          dragFrom === index && 'opacity-40',
          dragOver === index && dragFrom !== index && 'outline-primary outline-solid',
        ]"
        :draggable="editor.isEditing.value"
        :data-card="card"
        @dragstart="editor.isEditing.value && onDragStart(index, $event)"
        @dragover.prevent="dragOver = index"
        @drop="onDrop(index)"
        @dragend="resetDrag"
      >
        <KpiCard
          v-if="card === ROOMS_CARD"
          class="flex-1"
          :label="labelOf(card)"
          icon="door-open"
          :value="String(summary.rooms)"
          :caption="t('dashboard.kpi.roomsCaption')"
          :loading="isLoading"
        />

        <KpiCard
          v-else-if="card === ALERTS_CARD"
          class="flex-1"
          :label="labelOf(card)"
          icon="warning-octagon"
          :value="String(summary.alerts)"
          :alert="summary.alerts > 0"
          :caption="
            summary.alerts > 0 ? t('dashboard.kpi.alertsCaption') : t('dashboard.kpi.allClear')
          "
          :loading="isLoading"
        />

        <KpiCard
          v-else-if="card === PEOPLE_CARD"
          class="flex-1"
          :label="labelOf(card)"
          icon="users-three"
          :value="summary.occupancy === null ? EMPTY : n(summary.occupancy, 'percent')"
          :loading="isLoading"
        >
          <p class="text-label-stat text-on-surface-variant">
            {{ t('dashboard.kpi.people', { people: summary.people, capacity: summary.capacity }) }}
          </p>
          <ProgressBar class="mt-1" :value="summary.occupancy" :label="labelOf(card)" />
        </KpiCard>

        <!-- telemetry sums a building's devices at its newest report; a client sum would mix reports -->
        <KpiCard
          v-else-if="card === DEVICES_CARD"
          class="flex-1"
          :label="labelOf(card)"
          icon="wifi-high"
          :value="devices === null ? EMPTY : n(devices.totalDeviceCount)"
          :caption="devicesCaption"
          :alert="devicesStale"
          :loading="props.loading || devicesLoading"
        />

        <KpiCard
          v-else
          class="flex-1"
          :label="labelOf(card)"
          :icon="METRIC_DISPLAY[card]?.icon ?? 'gauge'"
          :value="figureOf(card) === null ? EMPTY : n(figureOf(card)!)"
          :unit="figureOf(card) === null ? '' : unitOf(card)"
          :loading="isLoading"
        >
          <BaseBadge
            v-if="card === 'indoor_aqi' && aqiBand(figureOf(card))"
            class="self-start"
            size="sm"
            :tone="BAND_TONE[aqiBand(figureOf(card))!]"
          >
            {{ t(`dashboard.kpi.aqi.${aqiBand(figureOf(card))}`) }}
          </BaseBadge>
          <p v-else class="text-label-stat text-on-surface-variant">{{ captionOf(card) }}</p>
        </KpiCard>

        <div v-if="editor.isEditing.value" class="flex justify-center gap-1">
          <IconButton
            icon="caret-left"
            size="sm"
            :label="t('dashboard.kpi.moveEarlier', { card: labelOf(card) })"
            :disabled="index === 0"
            @click="editor.move(index, index - 1)"
          />
          <IconButton
            icon="caret-right"
            size="sm"
            :label="t('dashboard.kpi.moveLater', { card: labelOf(card) })"
            :disabled="index === editor.cards.value.length - 1"
            @click="editor.move(index, index + 1)"
          />
          <IconButton
            icon="x"
            size="sm"
            variant="danger"
            :label="t('dashboard.kpi.removeCard', { card: labelOf(card) })"
            @click="editor.remove(card)"
          />
        </div>
      </li>

      <li v-if="editor.isEditing.value">
        <button
          type="button"
          class="flex h-full min-h-32 w-full flex-col items-center justify-center gap-1 rounded-2xl border-2 border-dashed border-outline-variant text-body-sm font-medium text-on-surface-variant transition-colors hover:border-primary hover:text-primary"
          @click="isAdding = true"
        >
          <BaseIcon name="plus" class="text-2xl" />
          {{ t('dashboard.kpi.addCard') }}
        </button>
      </li>
    </ul>
  </section>
</template>
