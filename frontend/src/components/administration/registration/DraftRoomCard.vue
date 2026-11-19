<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import type { RoomDraft } from '@/types/administration/buildingDraft.ts'
import type { MetricContract } from '@/types/dashboard/table.ts'
import {
  CAPACITY_BOUND,
  type ThresholdValues,
  withBoundValue,
} from '@/utils/administration/thresholds.ts'
import TextInput from '@/components/commons/forms/TextInput.vue'
import FormField from '@/components/commons/forms/FormField.vue'
import ThresholdInputs from '@/components/administration/registration/ThresholdInputs.vue'

const props = defineProps<{
  room: RoomDraft
  metrics: MetricContract[]
  buildingThresholds: ThresholdValues
}>()

const emit = defineEmits<{ update: [patch: Partial<RoomDraft>] }>()

const { t } = useI18n()

const setCapacity = (value: unknown) => {
  const capacity = Number(value) || 0
  emit('update', {
    capacity,
    thresholds: withBoundValue(
      props.room.thresholds,
      CAPACITY_BOUND.kind,
      CAPACITY_BOUND.key,
      capacity,
    ),
  })
}
</script>

<template>
  <article class="surface-card overflow-hidden">
    <div class="space-y-4 p-4">
      <header class="flex items-end gap-3">
        <FormField v-slot="{ id }" class="flex-1" :label="t('model.rooms.editRoom.name')">
          <TextInput
            :id="id"
            size="sm"
            :model-value="room.name"
            @update:model-value="emit('update', { name: String($event ?? '') })"
          />
        </FormField>
        <FormField v-slot="{ id }" class="w-28" :label="t('dashboard.table.headers.capacity')">
          <TextInput
            :id="id"
            size="sm"
            type="number"
            min="0"
            icon="users"
            :model-value="room.capacity"
            @update:model-value="setCapacity"
          />
        </FormField>
      </header>
      <p class="truncate font-mono text-[10px] text-on-surface-variant">{{ room.id }}</p>

      <p class="text-label-stat text-on-surface-variant">
        {{ t('model.register.roomThresholds') }}
      </p>
      <ThresholdInputs
        :metrics="metrics"
        :values="room.thresholds"
        :fallback="buildingThresholds"
        :hidden-keys="[CAPACITY_BOUND.key]"
        @update="emit('update', { thresholds: $event })"
      />
    </div>
  </article>
</template>
