import { describe, expect, it } from 'vitest'
import type { RuleGroupType } from 'react-querybuilder'
import { DEFAULT_FILTER, toApiFilter } from './fields'
import { DEFAULT_SORT, decodeSearch, encodeSearch } from './urlState'

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
    expect(decodeSearch('')).toEqual({ filter: DEFAULT_FILTER, sort: DEFAULT_SORT })
    expect(decodeSearch('?filter=%7Bnope&sort=[{"field":1}]')).toEqual({ filter: DEFAULT_FILTER, sort: DEFAULT_SORT })
  })
})
