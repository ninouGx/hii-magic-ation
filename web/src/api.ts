export type CatalogCounts = {
  cards: number
  commanders: number
  cards_without_print: number
  prints: number
  rulings: number
  oracle_tags: number
  art_tags: number
  card_tags: number
  edhrec_ranks: number
  edhrec_unresolved: number
}

export type CatalogStatus = {
  ready: boolean
  counts: CatalogCounts | null
  edhrec_mapping_rate: number | null
  edhrec_unresolved: { period: string; rank: number | null; name: string | null }[]
  bulk_versions: Record<string, string>
}

export type SyncPlan = { scryfall: boolean; edhrec: boolean; rebuild: boolean }

export type SyncStatus = {
  state: 'idle' | 'running' | 'done' | 'failed'
  started_at: number | null
  finished_at: number | null
  report: unknown
  error: string | null
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, init)
  if (!response.ok) {
    throw new Error((await response.text()) || `HTTP ${response.status}`)
  }
  return response.json() as Promise<T>
}

export const fetchCatalog = () => request<CatalogStatus>('/api/catalog')

export const fetchSync = () => request<SyncStatus>('/api/sync')

export const startSync = (plan: SyncPlan) =>
  request<SyncStatus>('/api/sync', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(plan),
  })
