<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import type { BuildingDraft } from '@/types/administration/buildingDraft.ts'
import type { MetricContract } from '@/types/dashboard/table.ts'
import FormField from '@/components/commons/forms/FormField.vue'
import TextInput from '@/components/commons/forms/TextInput.vue'
import ThresholdInputs from '@/components/administration/registration/ThresholdInputs.vue'

defineProps<{ draft: BuildingDraft; metrics: MetricContract[] }>()

const emit = defineEmits<{ update: [patch: Partial<Omit<BuildingDraft, 'rooms'>>] }>()

const { t } = useI18n()
</script>

<template>
  <section class="space-y-5">
    <FormField v-slot="{ id }" :label="t('model.register.buildingName')">
      <TextInput
        :id="id"
        :model-value="draft.name"
        icon="buildings"
        :placeholder="t('model.register.buildingNamePlaceholder')"
        @update:model-value="emit('update', { name: String($event ?? '') })"
      />
    </FormField>

    <div class="space-y-2">
      <p class="text-body-sm text-on-surface-variant">{{ t('model.register.thresholds') }}</p>
      <ThresholdInputs
        :metrics="metrics"
        :values="draft.thresholds"
        @update="emit('update', { thresholds: $event })"
      />
    </div>
  </section>
</template>
