import { describe, expect, it } from 'vitest'
import type { SelectionCard } from '../api'
import { alertLabel, alertText, alertsByCard, groupAlerts, groupByType, isSerious, exportFileName, membership, printRequest, selectionMeta } from './deck'

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

  it('groups alerts by kind, the most serious first, with one entry per card', () => {
    const groups = groupAlerts([
      { kind: 'outside_identity', oracle_id: 'a', name: 'Counterspell' },
      { kind: 'singleton', oracle_id: 'b', name: 'Sol Ring', quantity: 2, limit: 1 },
      { kind: 'outside_identity', oracle_id: 'c', name: 'Brainstorm' },
      { kind: 'not_legal', oracle_id: 'd', name: 'Mana Crypt', status: 'banned' },
    ])

    expect(groups.map((group) => [group.title, group.cards])).toEqual([
      ['Not legal in Commander (1)', ['Mana Crypt, banned']],
      ['Too many copies (1)', ['Sol Ring, 2 of 1']],
      ['Outside the color identity of the commander (2)', ['Counterspell', 'Brainstorm']],
    ])
  })

  it('labels each alert of a card in a few words', () => {
    expect(alertLabel({ kind: 'outside_identity', oracle_id: 'a', name: 'Counterspell' })).toBe('off-color')
    expect(alertLabel({ kind: 'not_legal', oracle_id: 'a', name: 'Mana Crypt', status: 'not_legal' })).toBe('not legal')
    expect(alertLabel({ kind: 'singleton', oracle_id: 'a', name: 'Sol Ring', quantity: 2, limit: 1 })).toBe('limit 1')
    expect(alertLabel({ kind: 'commander_in_list', oracle_id: 'a', name: 'Atarka' })).toBe('also commander')
  })

  it('treats an off-color card as a theme choice, not a broken deck', () => {
    expect(isSerious({ kind: 'outside_identity' })).toBe(false)
    expect(isSerious({ kind: 'commander_in_list' })).toBe(false)
    expect(isSerious({ kind: 'not_legal' })).toBe(true)
    expect(isSerious({ kind: 'singleton' })).toBe(true)
  })
})

describe('membership', () => {
  it('maps list cards to their quantity and commanders to their role', () => {
    const deck = {
      commander: card('cmd', null),
      partner: card('ptn', null),
      cards: [card('bolt', null), card('forest', null, 12)],
    }

    const members = membership(deck)

    expect(members.get('bolt')).toEqual({ quantity: 1, commander: false })
    expect(members.get('forest')).toEqual({ quantity: 12, commander: false })
    expect(members.get('cmd')).toEqual({ quantity: 0, commander: true })
    expect(members.get('ptn')).toEqual({ quantity: 0, commander: true })
    expect(members.has('other')).toBe(false)
  })

  it('keeps the quantity of a commander that is also in the list', () => {
    const members = membership({ commander: card('cmd', null), partner: null, cards: [card('cmd', null, 2)] })

    expect(members.get('cmd')).toEqual({ quantity: 2, commander: true })
  })
})

describe('print choice', () => {
  const withPrint = (name: string, print_id: string | null, quantity = 1) => ({ ...card(name, null, quantity), print_id, category: 'Ramp' })
  const deck = {
    name: 'Deck',
    notes: 'n',
    commander: withPrint('cmd', 'cmd-old'),
    partner: withPrint('ptn', null),
    cards: [withPrint('bolt', null, 2)],
  }

  it('reads the meta of a selection, prints included', () => {
    expect(selectionMeta(deck)).toEqual({
      name: 'Deck',
      notes: 'n',
      commander_oracle_id: 'cmd',
      partner_oracle_id: 'ptn',
      commander_print_id: 'cmd-old',
      partner_print_id: null,
    })
  })

  it('saves a commander or partner print in the selection meta', () => {
    expect(printRequest(deck, 'cmd', 'cmd-new')).toEqual({ meta: { ...selectionMeta(deck), commander_print_id: 'cmd-new' } })
    expect(printRequest(deck, 'ptn', 'ptn-new')).toEqual({ meta: { ...selectionMeta(deck), partner_print_id: 'ptn-new' } })
  })

  it('saves a list card print and keeps its copies and category', () => {
    expect(printRequest(deck, 'bolt', 'bolt-new')).toEqual({ card: { quantity: 2, print_id: 'bolt-new', category: 'Ramp' } })
  })

  it('has nothing to save for a card outside the selection', () => {
    expect(printRequest(deck, 'other', 'x')).toBeNull()
  })
})

describe('exportFileName', () => {
  it('names the file after the selection without characters a file system refuses', () => {
    expect(exportFileName('Dragons: fire/ice?')).toBe('Dragons- fire-ice-.txt')
    expect(exportFileName('  ')).toBe('selection.txt')
  })
})
