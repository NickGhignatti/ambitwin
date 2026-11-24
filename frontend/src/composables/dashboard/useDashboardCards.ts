import { computed, ref, watch, type Ref } from 'vue'
import { makeRequest } from '@/composables/commons/useApi.ts'
import type { MetricContract } from '@/types/dashboard/table.ts'
import { defaultCards } from '@/utils/dashboard/dashboard.ts'

const cardsUrl = (buildingId: string) =>
  `/dashboard/preferences/${encodeURIComponent(buildingId)}/cards`

/**
 * A building's dashboard cards, shared by everyone who views it, and the draft an editor
 * rearranges before saving. With nothing saved, the building shows `defaultCards`.
 */
export function useDashboardCards(
  buildingId: Ref<string | undefined>,
  catalog: Readonly<Ref<MetricContract[]>>,
  reportedKinds: Readonly<Ref<Set<string>>>,
) {
  const saved = ref<string[] | null>(null)
  const draft = ref<string[]>([])
  const isEditing = ref(false)
  const isSaving = ref(false)
  const saveFailed = ref(false)

  watch(
    buildingId,
    async (id) => {
      saved.value = null
      isEditing.value = false
      if (!id) return
      try {
        const response = await makeRequest(cardsUrl(id))
        if (response.status === 404) return
        if (!response.ok) throw new Error(`cards request failed: ${response.status}`)
        const { cards } = await response.json()
        // the user may have switched building while this was in flight
        if (buildingId.value === id && Array.isArray(cards)) saved.value = cards
      } catch (error) {
        console.error('[useDashboardCards] saved cards:', error)
      }
    },
    { immediate: true },
  )

  const cards = computed(() =>
    isEditing.value
      ? draft.value
      : (saved.value ?? defaultCards(catalog.value, reportedKinds.value)),
  )

  function edit() {
    draft.value = [...cards.value]
    saveFailed.value = false
    isEditing.value = true
  }

  function add(card: string) {
    if (!draft.value.includes(card)) draft.value = [...draft.value, card]
  }

  function remove(card: string) {
    draft.value = draft.value.filter((kept) => kept !== card)
  }

  function move(from: number, to: number) {
    if (to < 0 || to >= draft.value.length) return
    const reordered = [...draft.value]
    const [moved] = reordered.splice(from, 1) as [string]
    reordered.splice(to, 0, moved)
    draft.value = reordered
  }

  async function save() {
    const id = buildingId.value
    if (!id) return
    isSaving.value = true
    saveFailed.value = false
    try {
      const response = await makeRequest(cardsUrl(id), 'PUT', {
        body: JSON.stringify({ cards: draft.value }),
      })
      if (!response.ok) throw new Error(`saving cards failed: ${response.status}`)
      saved.value = draft.value
      isEditing.value = false
    } catch (error) {
      saveFailed.value = true
      console.error('[useDashboardCards] save:', error)
    } finally {
      isSaving.value = false
    }
  }

  return {
    cards,
    isEditing,
    isSaving,
    saveFailed,
    edit,
    cancel: () => (isEditing.value = false),
    add,
    remove,
    move,
    save,
  }
}
