import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { fetchSelections } from '../api'
import type { ActiveSelection } from './activeSelection'
import { DECK_SIZE } from './deck'

/** Picks the selection that the Explorer adds cards to. */
export function SelectionPicker({
  value,
  active,
  onChange,
}: {
  value: string | null
  active: ActiveSelection
  onChange: (id: string | null) => void
}) {
  const list = useQuery({ queryKey: ['selections'], queryFn: fetchSelections })
  const error = list.error ?? active.error

  return (
    <div className="actions selection-picker">
      <label>
        Add cards to{' '}
        <select value={value ?? ''} onChange={(event) => onChange(event.target.value || null)}>
          <option value="">No selection</option>
          {list.data?.map((summary) => (
            <option key={summary.selection_id} value={summary.selection_id}>
              {summary.name}
            </option>
          ))}
        </select>
      </label>
      {active.selection && (
        <span className="hint">
          {[active.selection.commander?.name, active.selection.partner?.name].filter(Boolean).join(' + ') || 'No commander'},{' '}
          {active.selection.count} / {DECK_SIZE}
        </span>
      )}
      {error && <p role="alert">{error.message}</p>}
    </div>
  )
}

/** Membership badge and add and remove buttons, for a grid tile and the detail window. */
export function CardActions({ oracleId, name, active, compact }: { oracleId: string; name: string; active: ActiveSelection; compact?: boolean }) {
  const deck = active.selection
  if (!deck) return null
  const member = active.members.get(oracleId)
  const pending = active.add.isPending || active.remove.isPending
  const target = { id: deck.selection_id, oracleId }

  return (
    // The click must not reach the grid tile, which opens the detail window.
    <div className={compact ? 'card-actions compact' : 'card-actions'} onClick={(event) => event.stopPropagation()}>
      {member?.commander && <span className="in-selection">Commander</span>}
      {member && member.quantity > 0 && (
        <span className="in-selection" title={`${member.quantity} in ${deck.name}`}>
          {compact ? `×${member.quantity}` : `${member.quantity} in ${deck.name}`}
        </span>
      )}
      {member && member.quantity > 0 && (
        <button type="button" aria-label={`Remove ${name} from ${deck.name}`} disabled={pending} onClick={() => active.remove.mutate(target)}>
          {compact ? '−' : 'Remove'}
        </button>
      )}
      {!member?.commander && (
        <button
          type="button"
          aria-label={member ? `Add a copy of ${name} to ${deck.name}` : `Add ${name} to ${deck.name}`}
          disabled={pending}
          onClick={() => active.add.mutate(target)}
        >
          {compact ? '+' : member ? 'Add a copy' : `Add to ${deck.name}`}
        </button>
      )}
    </div>
  )
}

/** Adds every result of the filter after a second click, so no browser dialog blocks the page. */
export function AddAllButton({ filter, total, active }: { filter: unknown; total: number; active: ActiveSelection }) {
  const [armed, setArmed] = useState(false)
  const [added, setAdded] = useState<number | null>(null)
  const deck = active.selection
  if (!deck || total === 0) return null
  const label = `${total.toLocaleString()} ${total === 1 ? 'card' : 'cards'}`

  if (!armed) {
    return (
      <span className="actions">
        <button type="button" onClick={() => setArmed(true)}>
          Add all {label} to {deck.name}
        </button>
        {added !== null && <span className="hint">{added.toLocaleString()} new cards added.</span>}
      </span>
    )
  }
  return (
    <span className="actions">
      <button
        type="button"
        disabled={active.addAll.isPending}
        onClick={() =>
          active.addAll.mutate({ id: deck.selection_id, filter }, {
            onSuccess: (result) => setAdded(result.added),
            onSettled: () => setArmed(false),
          })
        }
      >
        Confirm, add {label}
      </button>
      <button type="button" onClick={() => setArmed(false)}>
        Cancel
      </button>
    </span>
  )
}
