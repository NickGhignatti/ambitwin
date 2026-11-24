<script setup lang="ts">
import { ref } from 'vue'
import { useDashboardBuildings } from '@/composables/dashboard/useDashboardBuildings.ts'
import { useFullscreen } from '@/composables/commons/useFullscreen.ts'
import AppLayout from '@/components/commons/layout/AppLayout.vue'
import DashboardStatusBar from '@/components/dashboard/header/DashboardStatusBar.vue'
import ViewModeSwitch, {
  type DashboardView,
} from '@/components/dashboard/header/ViewModeSwitch.vue'
import KpiStrip from '@/components/dashboard/kpi/KpiStrip.vue'
import TelemetryPanel from '@/components/dashboard/table/TelemetryPanel.vue'
import ChartsPanel from '@/components/dashboard/charts/ChartsPanel.vue'

const { options, selectedId, rooms, isLoading } = useDashboardBuildings()

const view = ref<DashboardView>('table')
const focusArea = ref<HTMLElement | null>(null)
const { isFullscreen, toggle: toggleFocus } = useFullscreen(focusArea)
</script>

<template>
  <AppLayout>
    <div
      ref="focusArea"
      class="space-y-6 fullscreen:overflow-y-auto fullscreen:bg-surface-container-low fullscreen:p-6"
    >
      <DashboardStatusBar
        v-model:building-id="selectedId"
        :buildings="options"
        :room-count="rooms.length"
        :is-fullscreen="isFullscreen"
        @toggle-focus="toggleFocus"
      />

      <KpiStrip :building-id="selectedId" :rooms="rooms" :loading="isLoading" />

      <Transition
        mode="out-in"
        enter-active-class="transition duration-200 ease-out"
        enter-from-class="opacity-0 translate-y-1"
        leave-active-class="transition duration-150 ease-in"
        leave-to-class="opacity-0"
      >
        <TelemetryPanel v-if="view === 'table'" :building-id="selectedId" :rooms="rooms">
          <template #leading><ViewModeSwitch v-model="view" /></template>
        </TelemetryPanel>
        <ChartsPanel v-else :building-id="selectedId">
          <template #leading><ViewModeSwitch v-model="view" /></template>
        </ChartsPanel>
      </Transition>
    </div>
  </AppLayout>
</template>
