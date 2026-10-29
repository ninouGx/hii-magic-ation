import type { Selection, SelectionAlert, SelectionCard } from '../api'

/** A Commander deck holds 100 cards, commanders included. */
export const DECK_SIZE = 100

export type TypeGroup = { label: string; cards: SelectionCard[]; count: number }

/** Precedence decides the group of a card with several types, display order is separate. */
const PRECEDENCE: [string, string][] = [
  ['Creature', 'Creatures'],
  ['Land', 'Lands'],
  ['Planeswalker', 'Planeswalkers'],
  ['Battle', 'Battles'],
  ['Instant', 'Instants'],
  ['Sorcery', 'Sorceries'],
  ['Artifact', 'Artifacts'],
  ['Enchantment', 'Enchantments'],
]

const MISSING = 'Missing from the catalog'
const OTHER = 'Other'

const DISPLAY_ORDER = [
  'Creatures',
  'Planeswalkers',
  'Battles',
  'Instants',
  'Sorceries',
  'Artifacts',
  'Enchantments',
  'Lands',
  OTHER,
  MISSING,
]

function groupLabel(card: SelectionCard): string {
  if (card.name === null) return MISSING
  const frontType = (card.type_line ?? '').split(' // ')[0]
  const words = frontType.split(/[\s—-]+/)
  return PRECEDENCE.find(([type]) => words.includes(type))?.[1] ?? OTHER
}

/** Groups keep the card order of the API and skip empty types. */
export function groupByType(cards: SelectionCard[]): TypeGroup[] {
  const groups = new Map<string, SelectionCard[]>()
  for (const card of cards) {
    const label = groupLabel(card)
    groups.set(label, [...(groups.get(label) ?? []), card])
  }
  return DISPLAY_ORDER.flatMap((label) => {
    const members = groups.get(label)
    if (!members) return []
    return [{ label, cards: members, count: members.reduce((sum, card) => sum + card.quantity, 0) }]
  })
}

export function alertText(alert: SelectionAlert): string {
  switch (alert.kind) {
    case 'singleton':
      return `${alert.quantity} copies of ${alert.name}, the limit is ${alert.limit}.`
    case 'outside_identity':
      return `${alert.name} is outside the color identity of the commander.`
    case 'not_legal':
      return `${alert.name} is ${alert.status.replace('_', ' ')} in Commander.`
    case 'not_commander':
      return `${alert.name} cannot be a commander.`
    case 'commander_in_list':
      return `${alert.name} is a commander and is also in the list.`
    case 'missing_card':
      return 'A card is no longer in the catalog.'
  }
}

type AlertKind = SelectionAlert['kind']

/** Most serious first: a card that breaks the deck before one that only bends the theme. */
const ALERT_GROUPS: [AlertKind, string][] = [
  ['missing_card', 'Missing from the catalog'],
  ['not_commander', 'Cannot be a commander'],
  ['not_legal', 'Not legal in Commander'],
  ['singleton', 'Too many copies'],
  ['commander_in_list', 'Commander also in the list'],
  ['outside_identity', 'Outside the color identity of the commander'],
]

export type AlertGroup = { kind: AlertKind; title: string; cards: string[] }

/** Off-color cards and a commander kept in the list are choices a theme deck can make on purpose. */
export const isSerious = (alert: { kind: AlertKind }) => alert.kind !== 'outside_identity' && alert.kind !== 'commander_in_list'

function alertCard(alert: SelectionAlert): string {
  switch (alert.kind) {
    case 'singleton':
      return `${alert.name}, ${alert.quantity} of ${alert.limit}`
    case 'not_legal':
      return `${alert.name}, ${alert.status.replace('_', ' ')}`
    case 'missing_card':
      return alert.oracle_id
    default:
      return alert.name
  }
}

export function groupAlerts(alerts: SelectionAlert[]): AlertGroup[] {
  return ALERT_GROUPS.flatMap(([kind, label]) => {
    const cards = alerts.filter((alert) => alert.kind === kind).map(alertCard)
    return cards.length === 0 ? [] : [{ kind, title: `${label} (${cards.length})`, cards }]
  })
}

/** A few words shown beside the card in the list. */
export function alertLabel(alert: SelectionAlert): string {
  switch (alert.kind) {
    case 'singleton':
      return `limit ${alert.limit}`
    case 'outside_identity':
      return 'off-color'
    case 'not_legal':
      return alert.status.replace('_', ' ')
    case 'not_commander':
      return 'not a commander'
    case 'commander_in_list':
      return 'also commander'
    case 'missing_card':
      return 'missing'
  }
}

export function alertsByCard(alerts: SelectionAlert[]): Map<string, SelectionAlert[]> {
  const index = new Map<string, SelectionAlert[]>()
  for (const alert of alerts) index.set(alert.oracle_id, [...(index.get(alert.oracle_id) ?? []), alert])
  return index
}

/** `quantity` counts the copies in the list, a commander alone has none. */
export type Membership = { quantity: number; commander: boolean }

export function membership(deck: Pick<Selection, 'commander' | 'partner' | 'cards'>): Map<string, Membership> {
  const members = new Map<string, Membership>()
  for (const card of deck.cards) members.set(card.oracle_id, { quantity: card.quantity, commander: false })
  for (const leader of [deck.commander, deck.partner]) {
    if (leader) members.set(leader.oracle_id, { quantity: members.get(leader.oracle_id)?.quantity ?? 0, commander: true })
  }
  return members
}
