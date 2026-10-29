import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import {
  deleteSelection,
  fetchSelection,
  removeSelectionCard,
  setSelectionCard,
  updateSelection,
  type Selection,
  type SelectionCard,
  type SelectionMeta,
} from '../api'
import CardDetail from '../search/CardDetail'
import { neighbor } from '../search/neighbor'
import CommanderPicker from './CommanderPicker'
import { DECK_SIZE, alertText, alertsByCard, groupByType } from './deck'

type Props = {
  selectionId: string
  openCard: string | null
  onOpenCard: (oracleId: string | null) => void
  onDeleted: () => void
}

export default function SelectionView({ selectionId, openCard, onOpenCard, onDeleted }: Props) {
  const queryClient = useQueryClient()
  const selection = useQuery({ queryKey: ['selection', selectionId], queryFn: () => fetchSelection(selectionId) })

  // Each answer holds the whole selection, so the changes run one at a time and arrive in order.
  const scope = { id: `selection-${selectionId}` }
  const applied = (updated: Selection) => {
    queryClient.setQueryData(['selection', selectionId], updated)
    void queryClient.invalidateQueries({ queryKey: ['selections'] })
  }

  const meta = useMutation({
    mutationFn: (next: SelectionMeta) => updateSelection(selectionId, next),
    onSuccess: applied,
    scope,
  })
  const quantity = useMutation({
    mutationFn: ({ card, value }: { card: SelectionCard; value: number }) =>
      setSelectionCard(selectionId, card.oracle_id, { quantity: value, print_id: card.print_id, category: card.category }),
    onSuccess: applied,
    scope,
  })
  const removal = useMutation({
    mutationFn: (oracleId: string) => removeSelectionCard(selectionId, oracleId),
    onSuccess: applied,
    scope,
  })
  const deletion = useMutation({
    mutationFn: () => deleteSelection(selectionId),
    onSuccess: () => {
      queryClient.removeQueries({ queryKey: ['selection', selectionId] })
      void queryClient.invalidateQueries({ queryKey: ['selections'] })
      onDeleted()
    },
    scope,
  })

  if (selection.isError) return <p role="alert">{selection.error.message}</p>
  if (!selection.data) return <p>Loading…</p>
  const deck = selection.data

  const currentMeta = (): SelectionMeta => ({
    name: deck.name,
    notes: deck.notes,
    commander_oracle_id: deck.commander?.oracle_id ?? null,
    partner_oracle_id: deck.partner?.oracle_id ?? null,
  })
  const groups = groupByType(deck.cards)
  const ordered = [...[deck.commander, deck.partner].flatMap((card) => (card ? [card] : [])), ...groups.flatMap((group) => group.cards)]
  const alerts = alertsByCard(deck.alerts)
  const error = meta.error ?? quantity.error ?? removal.error ?? deletion.error

  return (
    <section className="selection" aria-label={deck.name}>
      <header className="selection-header">
        <NameEditor key={deck.name} name={deck.name} onRename={(name) => meta.mutate({ ...currentMeta(), name })} />
        <p className="deck-count" data-full={deck.count === DECK_SIZE} data-over={deck.count > DECK_SIZE}>
          {deck.count} / {DECK_SIZE} cards
        </p>
        <DeleteButton name={deck.name} pending={deletion.isPending} onConfirm={() => deletion.mutate()} />
      </header>
      {error && <p role="alert">{error.message}</p>}

      <div className="commanders">
        <CommanderPicker
          label="Commander"
          current={deck.commander}
          disabled={meta.isPending}
          onChange={(oracleId) =>
            meta.mutate({
              ...currentMeta(),
              commander_oracle_id: oracleId,
              partner_oracle_id: oracleId ? currentMeta().partner_oracle_id : null,
            })
          }
        />
        {deck.commander && (
          <CommanderPicker
            label="Partner"
            current={deck.partner}
            disabled={meta.isPending}
            onChange={(oracleId) => meta.mutate({ ...currentMeta(), partner_oracle_id: oracleId })}
          />
        )}
      </div>

      {deck.alerts.length > 0 && (
        <section aria-label="Alerts" className="alerts">
          <h3>Alerts ({deck.alerts.length})</h3>
          <ul>
            {deck.alerts.map((alert, index) => (
              <li key={index} data-kind={alert.kind}>
                {alertText(alert)}
              </li>
            ))}
          </ul>
        </section>
      )}

      {groups.length === 0 && <p className="hint">No card yet. Add cards from the Explorer.</p>}
      {groups.map((group) => (
        <section key={group.label} className="type-group" aria-label={group.label}>
          <h3>
            {group.label} ({group.count})
          </h3>
          <ul>
            {group.cards.map((card) => (
              <li key={card.oracle_id} data-alert={alerts.has(card.oracle_id)}>
                <QuantityInput
                  key={card.quantity}
                  label={`Copies of ${card.name ?? card.oracle_id}`}
                  quantity={card.quantity}
                  onSave={(value) => quantity.mutate({ card, value })}
                />
                {card.name ? (
                  <button type="button" className="card-name" onClick={() => onOpenCard(card.oracle_id)}>
                    {card.name}
                  </button>
                ) : (
                  <span className="card-name">{card.oracle_id}</span>
                )}
                {card.mana_cost && <span className="mana">{card.mana_cost}</span>}
                {alerts.has(card.oracle_id) && (
                  <span className="alert-mark" title={alerts.get(card.oracle_id)?.map(alertText).join(' ')}>
                    ⚠
                  </span>
                )}
                <button
                  type="button"
                  aria-label={`Remove ${card.name ?? card.oracle_id}`}
                  onClick={() => removal.mutate(card.oracle_id)}
                >
                  ×
                </button>
              </li>
            ))}
          </ul>
        </section>
      ))}

      {openCard && (
        <CardDetail
          oracleId={openCard}
          previous={neighbor(ordered, openCard, -1)}
          next={neighbor(ordered, openCard, 1)}
          onNavigate={onOpenCard}
          onClose={() => onOpenCard(null)}
        />
      )}
    </section>
  )
}

/** Enter or leaving the field saves, a value below 1 restores the saved quantity. */
function QuantityInput({ label, quantity, onSave }: { label: string; quantity: number; onSave: (value: number) => void }) {
  const [draft, setDraft] = useState(String(quantity))
  const save = () => {
    const value = Number(draft)
    if (!Number.isInteger(value) || value < 1) setDraft(String(quantity))
    else if (value !== quantity) onSave(value)
  }
  return (
    <input
      type="number"
      min={1}
      aria-label={label}
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={save}
      onKeyDown={(event) => {
        if (event.key === 'Enter') event.currentTarget.blur()
      }}
    />
  )
}

/** Enter or leaving the field saves, Escape restores the saved name. */
function NameEditor({ name, onRename }: { name: string; onRename: (name: string) => void }) {
  const [draft, setDraft] = useState(name)
  const save = () => {
    const trimmed = draft.trim()
    if (trimmed === '') setDraft(name)
    else if (trimmed !== name) onRename(trimmed)
  }
  return (
    <input
      className="selection-name"
      aria-label="Selection name"
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={save}
      onKeyDown={(event) => {
        if (event.key === 'Enter') event.currentTarget.blur()
        if (event.key === 'Escape') setDraft(name)
      }}
    />
  )
}

/** A second click confirms, so no browser dialog blocks the page. */
function DeleteButton({ name, pending, onConfirm }: { name: string; pending: boolean; onConfirm: () => void }) {
  const [armed, setArmed] = useState(false)
  if (!armed) {
    return (
      <button type="button" onClick={() => setArmed(true)}>
        Delete
      </button>
    )
  }
  return (
    <span className="actions">
      <button type="button" className="danger" disabled={pending} onClick={onConfirm}>
        Delete {name} for good
      </button>
      <button type="button" onClick={() => setArmed(false)}>
        Cancel
      </button>
    </span>
  )
}
