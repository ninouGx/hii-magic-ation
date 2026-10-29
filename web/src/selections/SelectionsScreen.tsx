import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { createSelection, fetchSelections } from '../api'
import SelectionView from './SelectionView'
import { DECK_SIZE } from './deck'

type ScreenState = { id: string | null; card: string | null }

const readUrl = (): ScreenState => {
  const params = new URLSearchParams(window.location.search)
  return { id: params.get('id'), card: params.get('card') }
}

export default function SelectionsScreen() {
  const queryClient = useQueryClient()
  const [state, setState] = useState<ScreenState>(readUrl)
  const [name, setName] = useState('')
  const list = useQuery({ queryKey: ['selections'], queryFn: fetchSelections })

  useEffect(() => {
    const params = new URLSearchParams()
    if (state.id) params.set('id', state.id)
    if (state.id && state.card) params.set('card', state.card)
    const search = params.size > 0 ? `?${params}` : ''
    window.history.replaceState(null, '', `${window.location.pathname}${search}`)
  }, [state])

  const creation = useMutation({
    mutationFn: createSelection,
    onSuccess: (created) => {
      queryClient.setQueryData(['selection', created.selection_id], created)
      void queryClient.invalidateQueries({ queryKey: ['selections'] })
      setName('')
      setState({ id: created.selection_id, card: null })
    },
  })

  return (
    <div className="selections-screen">
      <aside aria-label="Selections">
        <form
          className="actions"
          onSubmit={(event) => {
            event.preventDefault()
            if (name.trim() !== '') creation.mutate(name.trim())
          }}
        >
          <input aria-label="New selection name" placeholder="New selection" value={name} onChange={(event) => setName(event.target.value)} />
          <button type="submit" disabled={name.trim() === '' || creation.isPending}>
            Create
          </button>
        </form>
        {creation.isError && <p role="alert">{creation.error.message}</p>}
        {list.isError && <p role="alert">{list.error.message}</p>}
        {list.data?.length === 0 && <p className="hint">No selection yet.</p>}
        <ul className="selection-list">
          {list.data?.map((summary) => (
            <li key={summary.selection_id}>
              <button
                type="button"
                aria-current={summary.selection_id === state.id ? 'true' : undefined}
                onClick={() => setState({ id: summary.selection_id, card: null })}
              >
                <strong>{summary.name}</strong>
                <span className="hint">
                  {[summary.commander_name, summary.partner_name].filter(Boolean).join(' + ') || 'No commander'}, {summary.count} /{' '}
                  {DECK_SIZE}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </aside>
      {state.id ? (
        <SelectionView
          key={state.id}
          selectionId={state.id}
          openCard={state.card}
          onOpenCard={(card) => setState((current) => ({ ...current, card }))}
          onDeleted={() => setState({ id: null, card: null })}
        />
      ) : (
        <p className="hint">Choose a selection or create one.</p>
      )}
    </div>
  )
}
