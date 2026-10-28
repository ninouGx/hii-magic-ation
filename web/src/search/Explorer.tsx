import { useEffect, useMemo, useState } from 'react'
import { QueryBuilder, type RuleGroupType } from 'react-querybuilder'
import 'react-querybuilder/dist/query-builder.css'
import ResultGrid from './ResultGrid'
import SortControls from './SortControls'
import FieldValueEditor from './ValueEditors'
import { FIELDS, toApiFilter } from './fields'
import { decodeSearch, encodeSearch, type SortKey } from './urlState'

const DEBOUNCE_MS = 250

export default function Explorer() {
  const [initial] = useState(() => decodeSearch(window.location.search))
  const [filter, setFilter] = useState<RuleGroupType>(initial.filter)
  const [sort, setSort] = useState<SortKey[]>(initial.sort)
  const [card, setCard] = useState<string | null>(initial.card ?? null)

  useEffect(() => {
    window.history.replaceState(null, '', `${window.location.pathname}${encodeSearch({ filter, sort, card })}`)
  }, [filter, sort, card])

  const apiFilter = useDebounced(useMemo(() => toApiFilter(filter), [filter]), DEBOUNCE_MS)

  return (
    <>
      <QueryBuilder
        fields={FIELDS}
        query={filter}
        onQueryChange={setFilter}
        controlElements={{ valueEditor: FieldValueEditor }}
        showNotToggle
        resetOnFieldChange
        enableDragAndDrop={false}
      />
      <SortControls sort={sort} onChange={setSort} />
      <ResultGrid filter={apiFilter} sort={sort} openCard={card} onOpenCard={setCard} />
    </>
  )
}

/** Keeps the previous value until `value` stops changing, compared as JSON. */
function useDebounced<T>(value: T, delay: number): T {
  const [debounced, setDebounced] = useState(value)
  const key = JSON.stringify(value)
  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(JSON.parse(key) as T), delay)
    return () => window.clearTimeout(timer)
  }, [key, delay])
  return debounced
}
