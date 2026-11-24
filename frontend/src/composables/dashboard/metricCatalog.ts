import { onMounted, ref } from 'vue'
import { makeRequest } from '@/composables/commons/useApi.ts'
import type { MetricContract } from '@/types/dashboard/table.ts'

/**
 * Dashboard's merged metric catalog. Throws on a failed or empty answer, so a caller can tell
 * "the catalog is unreachable" from "there is nothing to add".
 */
export async function fetchMetricCatalog(): Promise<MetricContract[]> {
  const response = await makeRequest('/dashboard')
  if (!response.ok) throw new Error(`catalog request failed: ${response.status}`)
  const body = await response.text()
  if (body.trim() === '') throw new Error('catalog request returned an empty body')
  return JSON.parse(body).metrics ?? []
}

/** The catalog, loaded on mount, with a retry for when it failed. */
export function useMetricCatalog() {
  const metrics = ref<MetricContract[]>([])
  const isLoading = ref(true)
  const failed = ref(false)

  async function load() {
    isLoading.value = true
    failed.value = false
    try {
      metrics.value = await fetchMetricCatalog()
    } catch (error) {
      failed.value = true
      console.error('[useMetricCatalog]', error)
    } finally {
      isLoading.value = false
    }
  }
  onMounted(load)

  return { metrics, isLoading, failed, load }
}
