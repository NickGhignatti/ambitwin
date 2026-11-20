<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import type { MetricContract } from '@/types/dashboard/table.ts'
import { METRIC_I18N_KEY } from '@/utils/dashboard/metrics.ts'
import {
  type LimitEnd,
  type ThresholdValues,
  endsOf,
  withLimitEnd,
} from '@/utils/administration/thresholds.ts'
import FormField from '@/components/commons/forms/FormField.vue'
import TextInput from '@/components/commons/forms/TextInput.vue'

const props = defineProps<{
  metrics: MetricContract[]
  values: ThresholdValues
  /** Shown in an empty input: the value that applies when this one is left unset. */
  fallback?: ThresholdValues
  /** An end set elsewhere on the form, e.g. a room's people limit by its capacity. */
  hidden?: { kind: string; end: LimitEnd }
}>()

const emit = defineEmits<{ update: [values: ThresholdValues] }>()

const { t } = useI18n()

const visibleEnds = (metric: MetricContract) =>
  endsOf(metric).filter((end) => props.hidden?.kind !== metric.kind || props.hidden.end !== end)

// the catalog's label is English; a metric the client knows is shown in the user's language
const labelOf = (metric: MetricContract, end: LimitEnd) => {
  const key = METRIC_I18N_KEY[metric.kind]
  const name = key ? t(key) : metric.label
  const unit = metric.unit ? ` (${metric.unit})` : ''
  return `${t(`model.register.limit.${end}`, { label: name })}${unit}`
}

const setEnd = (kind: string, end: LimitEnd, input: unknown) => {
  const value = input === '' || input === null ? null : Number(input)
  if (value !== null && !Number.isFinite(value)) return
  emit('update', withLimitEnd(props.values, kind, end, value))
}
</script>

<template>
  <div class="grid gap-4 sm:grid-cols-2 md:grid-cols-4">
    <template v-for="metric in metrics" :key="metric.kind">
      <FormField
        v-for="end in visibleEnds(metric)"
        :key="`${metric.kind}.${end}`"
        v-slot="{ id }"
        :label="labelOf(metric, end)"
      >
        <TextInput
          :id="id"
          size="sm"
          type="number"
          :model-value="values[metric.kind]?.[end] ?? null"
          :placeholder="String(fallback?.[metric.kind]?.[end] ?? '')"
          @update:model-value="setEnd(metric.kind, end, $event)"
        />
      </FormField>
    </template>
  </div>
</template>
