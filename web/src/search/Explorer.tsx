import { useEffect, useMemo, useState } from 'react'
import { QueryBuilder, type RuleGroupType } from 'react-querybuilder'
import 'react-querybuilder/dist/query-builder.css'
import { useActiveSelection } from '../selections/activeSelection'
import { SelectionPicker } from '../selections/SelectionControls'
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
  const [chosen, setSelection] = useState<string | null>(initial.selection ?? null)
  const active = useActiveSelection(chosen)
  // A selection deleted on the other screen leaves a stale id in the URL.
  const selection = active.missing ? null : chosen

  useEffect(() => {
    window.history.replaceState(null, '', `${window.location.pathname}${encodeSearch({ filter, sort, card, selection })}`)
  }, [filter, sort, card, selection])

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
      <SelectionPicker value={selection} active={active} onChange={setSelection} />
      <ResultGrid filter={apiFilter} sort={sort} openCard={card} onOpenCard={setCard} active={active} />
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
