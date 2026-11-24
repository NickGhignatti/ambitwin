import { readFileSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import type { Page, Route } from '@playwright/test'

/**
 * The building's dashboard cards, with the backend stubbed. Cards come from the metric catalog,
 * each figure adds its rooms up the way its metric declares, and the saved order is the building's,
 * shared by everyone who views it.
 */

const DOMAIN = 'eng'
const BUILDING = 'bldg-cards'

const room = (id: string, x: number) => ({
  id,
  name: id,
  capacity: 10,
  position: { x, y: 2, z: 0 },
  dimensions: { width: 6, height: 4, depth: 6 },
})

const building = {
  id: BUILDING,
  name: 'Cards Block',
  domains: [DOMAIN],
  rooms: [room('R-1', 0), room('R-2', 7)],
}

const catalog = JSON.parse(
  readFileSync(new URL('../../schemas/fixtures/metric-contract.json', import.meta.url), 'utf8'),
) as { metrics: unknown[] }

// two rooms each: averaged temperature is 21, summed estimated people is 7
const READINGS: Record<string, number[]> = { temperature: [20, 22], ratio_device_count: [3, 4] }

interface Stub {
  role: string
  savedCards?: string[]
}

/** Stubs the dashboard's backend and returns the bodies the page saved. */
async function stubBackend(page: Page, { role, savedCards }: Stub) {
  const json = (route: Route, body: unknown, status = 200) => route.fulfill({ status, json: body })
  const saves: unknown[] = []

  await page.route('**/gateway/me', (route) => json(route, { sub: 'u1', accountName: 'Tester' }))
  await page.route('**/tenancy/me/memberships', (route) => json(route, [{ domain: DOMAIN, role }]))
  await page.route(`**/twin/buildings/${DOMAIN}`, (route) => json(route, [building]))
  await page.route(
    (url) => url.pathname === '/dashboard',
    (route) => json(route, { metrics: catalog.metrics }),
  )
  await page.route(`**/dashboard/preferences/${BUILDING}`, (route) => json(route, {}, 404))
  await page.route(`**/dashboard/preferences/${BUILDING}/cards`, (route) => {
    if (route.request().method() === 'PUT') {
      saves.push(route.request().postDataJSON())
      return route.fulfill({ status: 204 })
    }
    return savedCards ? json(route, { cards: savedCards }) : json(route, {}, 404)
  })
  await page.route('**/telemetry/*/entireBuilding*', (route) => {
    const metric = new URL(route.request().url()).pathname.split('/').at(-2)!
    const values = READINGS[metric] ?? []
    json(route, { data: values.map((value, i) => ({ roomId: `R-${i + 1}`, timestamp: 0, value })) })
  })
  await page.route(`**/telemetry/connected-devices/buildings/${BUILDING}`, (route) =>
    json(route, { message: 'none' }, 404),
  )
  return saves
}

async function openDashboard(page: Page) {
  await page.goto('/dashboards')
  const later = page.getByRole('dialog', { name: /alerts/i }).getByRole('button', { name: 'Later' })
  if (await later.isVisible().catch(() => false)) await later.click()
}

const shownCards = (page: Page) => page.locator('[data-card]')
const cardOrder = (page: Page) =>
  shownCards(page).evaluateAll((cards) => cards.map((card) => card.getAttribute('data-card')))

test('a building nobody customised shows the metrics it reports, added up per metric', async ({
  page,
}) => {
  await stubBackend(page, { role: 'business_admin' })

  await openDashboard(page)

  await expect
    .poll(() => cardOrder(page))
    .toEqual(['rooms', 'temperature', 'ratio_device_count', 'alerts'])
  await expect(page.locator('[data-card="temperature"]')).toContainText('21')
  await expect(page.locator('[data-card="temperature"]')).toContainText('Across reporting rooms')
  await expect(page.locator('[data-card="ratio_device_count"]')).toContainText('7')
  await expect(page.locator('[data-card="ratio_device_count"]')).toContainText('Sum of every room')
})

test('an editor removes, adds and reorders cards, and saves them for the building', async ({
  page,
}) => {
  const saves = await stubBackend(page, { role: 'business_admin' })
  await openDashboard(page)
  await expect(shownCards(page)).toHaveCount(4)

  await page.getByRole('button', { name: 'Edit cards' }).click()
  await page.getByRole('button', { name: 'Remove Rooms in alert' }).click()
  await page.getByRole('button', { name: 'Add card' }).click()
  await page.getByRole('button', { name: /Indoor Air Quality/ }).click()
  await page.getByRole('button', { name: 'Move Avg. temperature left' }).click()
  await page.getByRole('button', { name: 'Save' }).click()

  const order = ['temperature', 'rooms', 'ratio_device_count', 'indoor_aqi']
  await expect.poll(() => saves).toEqual([{ cards: order }])
  await expect.poll(() => cardOrder(page)).toEqual(order)
  await expect(page.getByRole('button', { name: 'Edit cards' })).toBeVisible()
})

test('a viewer sees the saved cards in their order and cannot edit them', async ({ page }) => {
  await stubBackend(page, { role: 'standard_customer', savedCards: ['alerts', 'rooms'] })

  await openDashboard(page)

  await expect.poll(() => cardOrder(page)).toEqual(['alerts', 'rooms'])
  await expect(page.getByRole('button', { name: 'Edit cards' })).toHaveCount(0)
})
