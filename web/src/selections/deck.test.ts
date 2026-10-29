import { describe, expect, it } from 'vitest'
import type { SelectionCard } from '../api'
import { alertText, alertsByCard, groupByType } from './deck'

const card = (name: string | null, type_line: string | null, quantity = 1): SelectionCard => ({
  oracle_id: name ?? 'gone',
  name,
  mana_cost: null,
  type_line,
  cmc: null,
  quantity,
  print_id: null,
  image_small: null,
  double_faced: false,
  category: null,
})

describe('groupByType', () => {
  it('files each card under one type, creatures before lands and artifacts', () => {
    const groups = groupByType([
      card('Sol Ring', 'Artifact'),
      card('Dryad Arbor', 'Land Creature — Forest'),
      card('Steel Overseer', 'Artifact Creature — Construct'),
      card('Darksteel Citadel', 'Artifact Land'),
      card('Forest', 'Basic Land — Forest', 30),
    ])

    expect(groups.map((group) => [group.label, group.cards.map((entry) => entry.name), group.count])).toEqual([
      ['Creatures', ['Dryad Arbor', 'Steel Overseer'], 2],
      ['Artifacts', ['Sol Ring'], 1],
      ['Lands', ['Darksteel Citadel', 'Forest'], 31],
    ])
  })

  it('uses the front face type of a double-faced card', () => {
    const [group] = groupByType([card('Emeria’s Call // Emeria, Shattered Skyclave', 'Sorcery // Land')])

    expect(group.label).toBe('Sorceries')
  })

  it('keeps cards a sync removed in their own group', () => {
    const groups = groupByType([card(null, null)])

    expect(groups.map((group) => group.label)).toEqual(['Missing from the catalog'])
  })
})

describe('alerts', () => {
  it('describes each alert kind in one sentence', () => {
    expect(alertText({ kind: 'singleton', oracle_id: 'a', name: 'Sol Ring', quantity: 2, limit: 1 })).toBe(
      '2 copies of Sol Ring, the limit is 1.',
    )
    expect(alertText({ kind: 'outside_identity', oracle_id: 'a', name: 'Counterspell' })).toBe(
      'Counterspell is outside the color identity of the commander.',
    )
    expect(alertText({ kind: 'not_legal', oracle_id: 'a', name: 'Mana Crypt', status: 'banned' })).toBe(
      'Mana Crypt is banned in Commander.',
    )
    expect(alertText({ kind: 'missing_card', oracle_id: 'a' })).toBe('A card is no longer in the catalog.')
  })

  it('indexes alerts by card', () => {
    const index = alertsByCard([
      { kind: 'singleton', oracle_id: 'a', name: 'Sol Ring', quantity: 2, limit: 1 },
      { kind: 'not_legal', oracle_id: 'a', name: 'Sol Ring', status: 'banned' },
    ])

    expect(index.get('a')).toHaveLength(2)
    expect(index.get('b')).toBeUndefined()
  })
})
