import { describe, expect, it } from 'vitest'
import type { RuleGroupType } from 'react-querybuilder'
import { DEFAULT_FILTER, deckFilter, findTag, tagSearchText, toApiFilter } from './fields'
import { DEFAULT_SORT, decodeSearch, encodeSearch } from './urlState'
import { neighbor } from './neighbor'
import { tagsForPrint } from './tags'

const group = (rules: RuleGroupType['rules'], not = false): RuleGroupType => ({ combinator: 'and', not, rules })

describe('toApiFilter', () => {
  it('converts number inputs and between pairs', () => {
    const filter = group([
      { field: 'cmc', operator: '<=', value: '3' },
      { field: 'power', operator: 'between', value: ['2', '4'] },
      { field: 'price_eur', operator: 'between', value: '1, 5' },
    ])

    expect(toApiFilter(filter).rules).toEqual([
      { field: 'cmc', operator: '<=', value: 3 },
      { field: 'power', operator: 'between', value: [2, 4] },
      { field: 'price_eur', operator: 'between', value: [1, 5] },
    ])
  })

  it('drops incomplete rules', () => {
    const filter = group([
      { field: 'name', operator: 'contains', value: '' },
      { field: 'cmc', operator: '=', value: 'x' },
      { field: 'tag', operator: 'has', value: { tag_id: '', label: '', include_children: true } },
      { field: 'keywords', operator: 'contains_any', value: [] },
      { field: 'power', operator: 'between', value: ['1', ''] },
    ])

    expect(toApiFilter(filter).rules).toEqual([])
  })

  it('drops a group left empty, even a negated one', () => {
    const filter = group([group([{ field: 'tag', operator: 'has', value: { tag_id: '', label: 'rem', include_children: true } }], true)])

    expect(toApiFilter(filter).rules).toEqual([])
  })

  it('keeps nested groups, negation and drops ids', () => {
    const filter: RuleGroupType = {
      id: 'root',
      combinator: 'and',
      rules: [
        {
          id: 'g1',
          combinator: 'or',
          not: true,
          rules: [{ id: 'r1', field: 'name', operator: 'regex', value: '^dragon' }],
        },
      ],
    }

    expect(toApiFilter(filter)).toEqual({
      combinator: 'and',
      rules: [{ combinator: 'or', not: true, rules: [{ field: 'name', operator: 'regex', value: '^dragon' }] }],
    })
  })

  it('sends only the tag id and the children option', () => {
    const filter = group([
      { field: 'tag', operator: 'has_not', value: { tag_id: 't1', label: 'removal', include_children: true } },
    ])

    expect(toApiFilter(filter).rules).toEqual([
      { field: 'tag', operator: 'has_not', value: { tag_id: 't1', include_children: true } },
    ])
  })

  it('splits keyword, set and year lists', () => {
    const filter = group([
      { field: 'keywords', operator: 'contains_all', value: 'Flying, Trample' },
      { field: 'set', operator: 'in', value: 'lea, leb' },
      { field: 'artist', operator: '=', value: ' Christopher Rush ' },
      { field: 'year', operator: 'in', value: '1993, 1994' },
      { field: 'year', operator: '=', value: '2020' },
    ])

    expect(toApiFilter(filter).rules).toEqual([
      { field: 'keywords', operator: 'contains_all', value: ['Flying', 'Trample'] },
      { field: 'set', operator: 'in', value: ['lea', 'leb'] },
      { field: 'artist', operator: '=', value: 'Christopher Rush' },
      { field: 'year', operator: 'in', value: [1993, 1994] },
      { field: 'year', operator: '=', value: 2020 },
    ])
  })

  it('starts with Commander legality', () => {
    expect(toApiFilter(DEFAULT_FILTER).rules).toEqual([
      { field: 'legality', operator: '=', value: { format: 'commander', status: 'legal' } },
    ])
  })
})

describe('URL state', () => {
  it('round-trips the filter and the sort without ids', () => {
    const state = {
      filter: { id: 'root', combinator: 'or', rules: [{ id: 'r', field: 'cmc', operator: '=', value: '2' }] },
      sort: [
        { field: 'commander_rank_week', direction: 'asc' as const },
        { field: 'cmc', direction: 'desc' as const },
      ],
    }

    const url = encodeSearch(state)
    const decoded = decodeSearch(url)

    expect(url).not.toContain('root')
    expect(decoded.sort).toEqual(state.sort)
    expect(decoded.filter).toEqual({ combinator: 'or', rules: [{ field: 'cmc', operator: '=', value: '2' }] })
  })

  it('falls back to defaults on a missing or broken URL', () => {
    expect(decodeSearch('')).toEqual({ filter: DEFAULT_FILTER, sort: DEFAULT_SORT, card: null, selection: null, deck: true })
    expect(decodeSearch('?filter=%7Bnope&sort=[{"field":1}]')).toEqual({
      filter: DEFAULT_FILTER,
      sort: DEFAULT_SORT,
      card: null,
      selection: null,
      deck: true,
    })
  })

  it('keeps the open card and leaves it out once closed', () => {
    const open = encodeSearch({ filter: DEFAULT_FILTER, sort: DEFAULT_SORT, card: 'abc' })
    const closed = encodeSearch({ filter: DEFAULT_FILTER, sort: DEFAULT_SORT, card: null })

    expect(decodeSearch(open).card).toBe('abc')
    expect(closed).not.toContain('card=')
  })

  it('keeps the active selection and leaves it out when none is active', () => {
    const active = encodeSearch({ filter: DEFAULT_FILTER, sort: DEFAULT_SORT, selection: 'sel-1' })
    const none = encodeSearch({ filter: DEFAULT_FILTER, sort: DEFAULT_SORT, selection: null })

    expect(decodeSearch(active).selection).toBe('sel-1')
    expect(none).not.toContain('selection=')
  })

  it('keeps deck mode on by default and writes it only once turned off', () => {
    const on = encodeSearch({ filter: DEFAULT_FILTER, sort: DEFAULT_SORT, deck: true })
    const off = encodeSearch({ filter: DEFAULT_FILTER, sort: DEFAULT_SORT, deck: false })

    expect(on).not.toContain('deck=')
    expect(decodeSearch(off).deck).toBe(false)
    expect(decodeSearch(on).deck).toBe(true)
  })
})

describe('deckFilter', () => {
  const filter = { combinator: 'or', rules: [{ field: 'cmc', operator: '=', value: 2 }] }

  it('leaves the filter alone without a commander identity', () => {
    expect(deckFilter(filter, null)).toBe(filter)
  })

  it('keeps the filter and adds the commander identity beside it', () => {
    expect(deckFilter(filter, ['B', 'G'])).toEqual({
      combinator: 'and',
      rules: [filter, { field: 'color_identity', operator: 'at_most', value: ['B', 'G'] }],
    })
  })

  it('limits a colorless commander to colorless cards', () => {
    expect(deckFilter({ combinator: 'and', rules: [] }, [])).toEqual({
      combinator: 'and',
      rules: [{ field: 'color_identity', operator: 'at_most', value: [] }],
    })
  })
})

describe('tag lookup', () => {
  const options = [
    { tag_id: 'o', slug: 'removal', kind: 'oracle' },
    { tag_id: 'a', slug: 'removal', kind: 'art' },
  ]

  it('searches a pasted option by its slug', () => {
    expect(tagSearchText('removal (oracle)')).toBe('removal')
    expect(tagSearchText('removal-creature')).toBe('removal-creature')
  })

  it('resolves the option text to the tag of that kind', () => {
    expect(findTag(options, 'removal (art)')?.tag_id).toBe('a')
    expect(findTag(options, 'removal')).toBeUndefined()
    expect(findTag(undefined, 'removal (oracle)')).toBeUndefined()
  })
})

describe('neighbor', () => {
  const cards = [{ oracle_id: 'a' }, { oracle_id: 'b' }, { oracle_id: 'c' }]

  it('moves to the previous and next card of the result', () => {
    expect(neighbor(cards, 'b', 1)).toBe('c')
    expect(neighbor(cards, 'b', -1)).toBe('a')
  })

  it('stops at both ends and on a card no longer in the result', () => {
    expect(neighbor(cards, 'a', -1)).toBeNull()
    expect(neighbor(cards, 'c', 1)).toBeNull()
    expect(neighbor(cards, 'z', 1)).toBeNull()
  })
})

describe('tagsForPrint', () => {
  const tag = (tag_id: string, kind: string) => ({ tag_id, slug: tag_id, label: tag_id, kind, source: 'scryfall' })
  const tags = [tag('burn', 'oracle'), tag('lightning', 'art'), tag('goblin', 'art')]

  it('keeps the oracle tags and only the art tags of the shown print', () => {
    expect(tagsForPrint(tags, { art_tag_ids: ['goblin'] }).map((t) => t.tag_id)).toEqual(['burn', 'goblin'])
  })

  it('keeps every tag when no print is shown', () => {
    expect(tagsForPrint(tags, undefined)).toBe(tags)
  })
})
