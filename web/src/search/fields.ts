import type { RuleGroupType, RuleType } from 'react-querybuilder'

export type FieldKind = 'text' | 'number' | 'colors' | 'keywords' | 'tag' | 'bool' | 'legality' | 'pick' | 'year'

type Operator = { name: string; label: string }

const TEXT_OPERATORS: Operator[] = [
  { name: 'contains', label: 'contains' },
  { name: 'not_contains', label: 'does not contain' },
  { name: 'regex', label: 'matches regex' },
]

const NUMBER_OPERATORS: Operator[] = ['=', '<', '<=', '>', '>='].map((name) => ({ name, label: name }))
NUMBER_OPERATORS.push({ name: 'between', label: 'between' })

const OPERATORS: Record<FieldKind, Operator[]> = {
  text: TEXT_OPERATORS,
  number: NUMBER_OPERATORS,
  colors: [
    { name: 'exactly', label: 'exactly' },
    { name: 'includes', label: 'includes' },
    { name: 'at_most', label: 'at most' },
  ],
  keywords: [
    { name: 'contains_any', label: 'contains any of' },
    { name: 'contains_all', label: 'contains all of' },
  ],
  tag: [
    { name: 'has', label: 'has' },
    { name: 'has_not', label: 'has not' },
  ],
  bool: [{ name: '=', label: 'is' }],
  legality: [{ name: '=', label: 'is' }],
  pick: [
    { name: '=', label: 'is' },
    { name: 'in', label: 'is one of' },
  ],
  year: [
    { name: '=', label: 'is' },
    { name: 'in', label: 'is one of' },
  ],
}

export const COLORS = ['W', 'U', 'B', 'R', 'G'] as const

export const LEGALITY_STATUSES = ['legal', 'not_legal', 'banned', 'restricted'] as const

export type LegalityValue = { format: string; status: (typeof LEGALITY_STATUSES)[number] }

/** `label` is only for display, the API needs `tag_id`. */
export type TagValue = { tag_id: string; label: string; include_children: boolean }

const FIELD_LIST: [string, string, FieldKind][] = [
  ['legality', 'Legality', 'legality'],
  ['name', 'Name', 'text'],
  ['oracle_text', 'Oracle text', 'text'],
  ['type_line', 'Type line', 'text'],
  ['flavor_text', 'Flavor text', 'text'],
  ['rulings', 'Rulings', 'text'],
  ['colors', 'Colors', 'colors'],
  ['color_identity', 'Color identity', 'colors'],
  ['cmc', 'Mana value', 'number'],
  ['power', 'Power', 'number'],
  ['toughness', 'Toughness', 'number'],
  ['loyalty', 'Loyalty', 'number'],
  ['keywords', 'Keywords', 'keywords'],
  ['tag', 'Tag', 'tag'],
  ['commander_eligible', 'Can be commander', 'bool'],
  ['edhrec_rank', 'EDHREC rank', 'number'],
  ['commander_rank_week', 'Commander rank, week', 'number'],
  ['commander_rank_month', 'Commander rank, month', 'number'],
  ['commander_rank_year', 'Commander rank, 2 years', 'number'],
  ['num_decks_week', 'Decks, week', 'number'],
  ['num_decks_month', 'Decks, month', 'number'],
  ['num_decks_year', 'Decks, 2 years', 'number'],
  ['price_eur', 'Price EUR', 'number'],
  ['artist', 'Artist', 'pick'],
  ['set', 'Set code', 'pick'],
  ['year', 'Release year', 'year'],
]

export const FIELD_KINDS: Record<string, FieldKind> = Object.fromEntries(
  FIELD_LIST.map(([name, , kind]) => [name, kind]),
)

export function defaultValue(kind: FieldKind): unknown {
  switch (kind) {
    case 'colors':
      return []
    case 'keywords':
      return []
    case 'bool':
      return true
    case 'legality':
      return { format: 'commander', status: 'legal' } satisfies LegalityValue
    case 'tag':
      return { tag_id: '', label: '', include_children: true } satisfies TagValue
    default:
      return ''
  }
}

export const FIELDS = FIELD_LIST.map(([name, label, kind]) => ({
  name,
  label,
  operators: OPERATORS[kind],
  defaultValue: defaultValue(kind),
}))

export const DEFAULT_FILTER: RuleGroupType = {
  combinator: 'and',
  rules: [{ field: 'legality', operator: '=', value: defaultValue('legality') }],
}

export type ApiRule = { field: string; operator: string; value: unknown }
export type ApiGroup = { combinator: string; not?: boolean; rules: (ApiRule | ApiGroup)[] }

/** Deck mode: with a commander identity, the result keeps only cards a Commander deck of that commander can play. */
export function deckFilter(filter: ApiGroup, identity: string[] | null): ApiGroup {
  if (identity === null) return filter
  const rule: ApiRule = { field: 'color_identity', operator: 'at_most', value: identity }
  return { combinator: 'and', rules: filter.rules.length > 0 ? [filter, rule] : [rule] }
}

/** Drops incomplete rules and the groups they leave empty, so a rule being
 * typed never changes the result, even under a NOT. */
export function toApiFilter(group: RuleGroupType): ApiGroup {
  const rules = group.rules.flatMap<ApiRule | ApiGroup>((node) => {
    if (typeof node === 'string') return []
    if ('rules' in node) {
      const nested = toApiFilter(node)
      return nested.rules.length > 0 ? [nested] : []
    }
    const rule = toApiRule(node)
    return rule ? [rule] : []
  })
  return group.not ? { combinator: group.combinator, not: true, rules } : { combinator: group.combinator, rules }
}

function toApiRule(rule: RuleType): ApiRule | null {
  const value = apiValue(FIELD_KINDS[rule.field], rule.operator, rule.value)
  return value === null ? null : { field: rule.field, operator: rule.operator, value }
}

function apiValue(kind: FieldKind | undefined, operator: string, value: unknown): unknown {
  switch (kind) {
    case 'text':
      return typeof value === 'string' && value !== '' ? value : null
    case 'number':
      return operator === 'between' ? numberPair(value) : toNumber(value)
    case 'colors':
      return Array.isArray(value) ? value : null
    case 'keywords':
      return nonEmpty(splitList(value))
    case 'tag': {
      const tag = value as Partial<TagValue> | undefined
      return tag?.tag_id ? { tag_id: tag.tag_id, include_children: tag.include_children ?? true } : null
    }
    case 'bool':
      return typeof value === 'boolean' ? value : null
    case 'legality':
      return value
    case 'pick':
      return operator === 'in' ? nonEmpty(splitList(value)) : (splitList(value)[0] ?? null)
    case 'year': {
      const years = splitList(value).map(toNumber)
      if (years.length === 0 || years.some((year) => year === null)) return null
      return operator === 'in' ? years : years[0]
    }
    default:
      return null
  }
}

function toNumber(value: unknown): number | null {
  if (typeof value === 'number') return Number.isFinite(value) ? value : null
  if (typeof value !== 'string' || value.trim() === '') return null
  const number = Number(value)
  return Number.isFinite(number) ? number : null
}

function numberPair(value: unknown): [number, number] | null {
  const parts = Array.isArray(value) ? value : splitList(value)
  const [low, high] = parts.map(toNumber)
  return parts.length === 2 && low !== null && high !== null ? [low, high] : null
}

export function splitList(value: unknown): string[] {
  const parts = Array.isArray(value) ? value : typeof value === 'string' ? value.split(',') : []
  return parts.map((part) => String(part).trim()).filter((part) => part !== '')
}

function nonEmpty<T>(list: T[]): T[] | null {
  return list.length > 0 ? list : null
}

export const tagOptionText = (tag: { slug: string; kind: string }) => `${tag.slug} (${tag.kind})`

/** The API searches slugs and labels, so the kind suffix of an option is left out. */
export const tagSearchText = (text: string) => text.replace(/ \([a-z]+\)$/, '').trim()

export function findTag<T extends { slug: string; kind: string }>(options: T[] | undefined, text: string) {
  return options?.find((option) => tagOptionText(option) === text)
}
