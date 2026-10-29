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

export class HttpError extends Error {
  readonly status: number

  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

/** A 4xx answer stays the same on a retry. */
export const shouldRetry = (failureCount: number, error: Error) =>
  !(error instanceof HttpError && error.status < 500) && failureCount < 3

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, init)
  if (!response.ok) {
    throw new HttpError(response.status, (await response.text()) || `HTTP ${response.status}`)
  }
  if (response.status === 204) return undefined as T
  return response.json() as Promise<T>
}

const sendJson = (method: string, body: unknown): RequestInit => ({
  method,
  headers: { 'content-type': 'application/json' },
  body: JSON.stringify(body),
})

export const fetchCatalog = () => request<CatalogStatus>('/api/catalog')

export const fetchSync = () => request<SyncStatus>('/api/sync')

export const startSync = (plan: SyncPlan) =>
  request<SyncStatus>('/api/sync', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(plan),
  })

export type CardSummary = {
  oracle_id: string
  name: string
  mana_cost: string | null
  type_line: string | null
  image_small: string | null
  double_faced: boolean
  price_eur: number | null
}

export type SearchPage = { total: number; cards: CardSummary[] }

export type SearchRequest = {
  filter: unknown
  sort: { field: string; direction: 'asc' | 'desc' }[]
  offset: number
  limit: number
}

export type TagSuggestion = { tag_id: string; slug: string; label: string; kind: string; cards: number }

export const searchCards = (search: SearchRequest, signal?: AbortSignal) =>
  request<SearchPage>('/api/cards/search', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(search),
    signal,
  })

export const fetchTags = (text: string) =>
  request<TagSuggestion[]>(`/api/tags?${new URLSearchParams({ q: text })}`)

export const fetchKeywords = (text: string) =>
  request<string[]>(`/api/keywords?${new URLSearchParams({ q: text })}`)

export const fetchFormats = () => request<string[]>('/api/formats')

export type CardFace = {
  name: string | null
  mana_cost: string | null
  type_line: string | null
  oracle_text: string | null
  power: string | null
  toughness: string | null
  loyalty: string | null
}

export type CardDetail = CardFace & {
  oracle_id: string
  name: string
  cmc: number | null
  layout: string
  double_faced: boolean
  colors: string[]
  color_identity: string[]
  keywords: string[]
  is_commander_eligible: boolean
  game_changer: boolean
  edhrec_rank: number | null
  default_print_id: string | null
  faces: CardFace[]
  legalities: { format: string; status: string }[]
  commander_ranks: { period: string; rank: number; num_decks: number | null; trend_zscore: number | null }[]
  tags: { tag_id: string; slug: string; label: string; kind: string; source: string }[]
  rulings: { published_at: string; source: string; comment: string }[]
  prints: {
    print_id: string
    set_code: string
    collector_number: string
    released_at: string | null
    rarity: string | null
    artist: string | null
    image_small: string | null
    price_eur: number | null
    price_usd: number | null
    art_tag_ids: string[]
  }[]
}

export const fetchCard = (oracleId: string) => request<CardDetail>(`/api/cards/${encodeURIComponent(oracleId)}`)

export type SelectionSummary = {
  selection_id: string
  name: string
  commander_name: string | null
  partner_name: string | null
  count: number
  updated_at: string
}

export type SelectionCard = {
  oracle_id: string
  /** Null when a sync removed the card from the catalog. */
  name: string | null
  mana_cost: string | null
  type_line: string | null
  cmc: number | null
  quantity: number
  print_id: string | null
  image_small: string | null
  double_faced: boolean
  category: string | null
}

export type SelectionAlert =
  | { kind: 'singleton'; oracle_id: string; name: string; quantity: number; limit: number }
  | { kind: 'outside_identity' | 'not_commander' | 'commander_in_list'; oracle_id: string; name: string }
  | { kind: 'not_legal'; oracle_id: string; name: string; status: string }
  | { kind: 'missing_card'; oracle_id: string }

export type Selection = {
  selection_id: string
  name: string
  notes: string
  commander: SelectionCard | null
  partner: SelectionCard | null
  updated_at: string
  count: number
  cards: SelectionCard[]
  alerts: SelectionAlert[]
}

export type SelectionMeta = {
  name: string
  notes: string
  commander_oracle_id: string | null
  partner_oracle_id: string | null
  /** Null keeps the default print of the card. */
  commander_print_id: string | null
  partner_print_id: string | null
}

export type CardChange = { quantity: number; print_id: string | null; category: string | null }

const selectionPath = (id: string) => `/api/selections/${encodeURIComponent(id)}`
const selectionCardPath = (id: string, oracleId: string) => `${selectionPath(id)}/cards/${encodeURIComponent(oracleId)}`

export const fetchSelections = () => request<SelectionSummary[]>('/api/selections')

export const createSelection = (name: string) => request<Selection>('/api/selections', sendJson('POST', { name }))

export const fetchSelection = (id: string) => request<Selection>(selectionPath(id))

export const updateSelection = (id: string, meta: SelectionMeta) => request<Selection>(selectionPath(id), sendJson('PUT', meta))

export const deleteSelection = (id: string) => request<void>(selectionPath(id), { method: 'DELETE' })

export const addSelectionCard = (id: string, oracleId: string) =>
  request<Selection>(selectionCardPath(id, oracleId), { method: 'POST' })

export const setSelectionCard = (id: string, oracleId: string, change: CardChange) =>
  request<Selection>(selectionCardPath(id, oracleId), sendJson('PUT', change))

export const removeSelectionCard = (id: string, oracleId: string) =>
  request<Selection>(selectionCardPath(id, oracleId), { method: 'DELETE' })

/** Adds each card the filter matches, once. `added` counts the cards that were not there yet. */
export const addFilteredCards = (id: string, filter: unknown) =>
  request<{ added: number }>(`${selectionPath(id)}/cards`, sendJson('POST', { filter }))

/** Moxfield import text, and the oracle ids a sync removed from the catalog, left out of it. */
export type SelectionExport = { text: string; skipped: string[] }

export const fetchSelectionExport = (id: string) => request<SelectionExport>(`${selectionPath(id)}/export`)
