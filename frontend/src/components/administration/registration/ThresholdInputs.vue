<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import type { MetricContract } from '@/types/dashboard/table.ts'
import { type ThresholdValues, withBoundValue } from '@/utils/administration/thresholds.ts'
import FormField from '@/components/commons/forms/FormField.vue'
import TextInput from '@/components/commons/forms/TextInput.vue'

const props = defineProps<{
  metrics: MetricContract[]
  values: ThresholdValues
  /** Shown in an empty input: the value that applies when this one is left unset. */
  fallback?: ThresholdValues
  /** Bound keys set elsewhere on the form, e.g. a room's people limit by its capacity. */
  hiddenKeys?: string[]
}>()

const emit = defineEmits<{ update: [values: ThresholdValues] }>()

const { t } = useI18n()

const visibleBounds = (metric: MetricContract) =>
  (metric.bounds ?? []).filter((bound) => !props.hiddenKeys?.includes(bound.key))

const setBound = (kind: string, key: string, input: unknown) => {
  const value = input === '' || input === null ? null : Number(input)
  if (value !== null && !Number.isFinite(value)) return
  emit('update', withBoundValue(props.values, kind, key, value))
}
</script>

<template>
  <div class="grid gap-4 sm:grid-cols-2 md:grid-cols-4">
    <template v-for="metric in metrics" :key="metric.kind">
      <FormField
        v-for="bound in visibleBounds(metric)"
        :key="`${metric.kind}.${bound.key}`"
        v-slot="{ id }"
        :label="
          t(`model.register.bound.${bound.direction}`, { label: bound.label }) +
          (bound.unit ? ` (${bound.unit})` : '')
        "
      >
        <TextInput
          :id="id"
          size="sm"
          type="number"
          :model-value="values[metric.kind]?.[bound.key] ?? null"
          :placeholder="String(fallback?.[metric.kind]?.[bound.key] ?? '')"
          @update:model-value="setBound(metric.kind, bound.key, $event)"
        />
      </FormField>
    </template>
  </div>
</template>
