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
  }[]
}

export const fetchCard = (oracleId: string) => request<CardDetail>(`/api/cards/${encodeURIComponent(oracleId)}`)
