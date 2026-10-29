import { useQuery } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { searchCards, type SelectionCard } from '../api'

const SUGGESTIONS = 12
const DEBOUNCE_MS = 200

type Props = {
  label: string
  current: SelectionCard | null
  onChange: (oracleId: string | null) => void
  disabled?: boolean
}

/** Searches commander-eligible cards by name, most played first. */
export default function CommanderPicker({ label, current, onChange, disabled }: Props) {
  const [text, setText] = useState('')
  const [query, setQuery] = useState('')

  useEffect(() => {
    const timer = window.setTimeout(() => setQuery(text.trim()), DEBOUNCE_MS)
    return () => window.clearTimeout(timer)
  }, [text])

  const suggestions = useQuery({
    queryKey: ['commanders', query],
    queryFn: ({ signal }) =>
      searchCards(
        {
          filter: {
            combinator: 'and',
            rules: [
              { field: 'commander_eligible', operator: '=', value: true },
              { field: 'name', operator: 'contains', value: query },
            ],
          },
          sort: [
            { field: 'commander_rank_year', direction: 'asc' },
            { field: 'name', direction: 'asc' },
          ],
          offset: 0,
          limit: SUGGESTIONS,
        },
        signal,
      ),
    enabled: query !== '',
  })

  return (
    <fieldset className="commander-picker" disabled={disabled}>
      <legend>{label}</legend>
      {current ? (
        <p>
          <strong>{current.name ?? 'Card no longer in the catalog'}</strong>{' '}
          <button type="button" onClick={() => onChange(null)}>
            Remove
          </button>
        </p>
      ) : (
        <p className="hint">None</p>
      )}
      <input
        type="search"
        aria-label={`Search a ${label.toLowerCase()} by name`}
        placeholder="Search by name"
        value={text}
        onChange={(event) => setText(event.target.value)}
      />
      {suggestions.isError && <p role="alert">{suggestions.error.message}</p>}
      {query !== '' && suggestions.data && (
        <ul className="suggestions">
          {suggestions.data.cards.length === 0 && <li className="hint">No commander matches.</li>}
          {suggestions.data.cards.map((card) => (
            <li key={card.oracle_id}>
              <button
                type="button"
                onClick={() => {
                  onChange(card.oracle_id)
                  setText('')
                  setQuery('')
                }}
              >
                {card.name}
              </button>
            </li>
          ))}
        </ul>
      )}
    </fieldset>
  )
}
