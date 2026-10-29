import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import {
  fetchSelectionExport,
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
import { useChoosePrint } from './activeSelection'
import { DECK_SIZE, exportFileName, alertLabel, alertText, alertsByCard, groupAlerts, groupByType, isSerious, selectionMeta, type AlertGroup } from './deck'
import { PrintChoice } from './SelectionControls'

const OPEN_ALERT_LIMIT = 5

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
  const choosePrint = useChoosePrint(selectionId)
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

  const currentMeta = (): SelectionMeta => selectionMeta(deck)
  const groups = groupByType(deck.cards)
  const ordered = [...[deck.commander, deck.partner].flatMap((card) => (card ? [card] : [])), ...groups.flatMap((group) => group.cards)]
  const alerts = alertsByCard(deck.alerts)
  const error = meta.error ?? quantity.error ?? removal.error ?? choosePrint.error ?? deletion.error

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
          onOpen={onOpenCard}
          disabled={meta.isPending}
          onChange={(oracleId) =>
            meta.mutate({
              ...currentMeta(),
              commander_oracle_id: oracleId,
              commander_print_id: null,
              partner_oracle_id: oracleId ? currentMeta().partner_oracle_id : null,
              partner_print_id: oracleId ? currentMeta().partner_print_id : null,
            })
          }
        />
        {deck.commander && (
          <CommanderPicker
            label="Partner"
            current={deck.partner}
            onOpen={onOpenCard}
            disabled={meta.isPending}
            onChange={(oracleId) => meta.mutate({ ...currentMeta(), partner_oracle_id: oracleId, partner_print_id: null })}
          />
        )}
      </div>

      {deck.alerts.length > 0 && (
        <section aria-label="Alerts" className="alerts">
          <h3>Alerts ({deck.alerts.length})</h3>
          {groupAlerts(deck.alerts).map((group) => (
            <AlertDetails key={group.kind} group={group} />
          ))}
        </section>
      )}

      <ExportPanel deck={deck} />

      {groups.length === 0 && <p className="hint">No card yet. Add cards from the Explorer.</p>}
      {groups.map((group) => (
        <section key={group.label} className="type-group" aria-label={group.label}>
          <h3>
            {group.label} ({group.count})
          </h3>
          <ul>
            {group.cards.map((card) => (
              <li key={card.oracle_id} data-alert={alerts.get(card.oracle_id)?.some(isSerious) ?? false}>
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
                {alerts.get(card.oracle_id)?.map((alert) => (
                  <span key={alert.kind} className="alert-label" data-serious={isSerious(alert)} title={alertText(alert)}>
                    {alertLabel(alert)}
                  </span>
                ))}
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
          initialPrint={ordered.find((card) => card.oracle_id === openCard)?.print_id}
          printAction={(printId, card) => (
            <PrintChoice
              deck={deck}
              oracleId={card.oracle_id}
              printId={printId}
              defaultPrintId={card.default_print_id}
              pending={choosePrint.isPending}
              onChoose={(chosen) => choosePrint.mutate({ id: selectionId, oracleId: card.oracle_id, printId: chosen })}
            />
          )}
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

/**
 * A long group of theme choices starts closed, so a deck full of off-color cards does not push the list down.
 * After that, only the user opens or closes it.
 */
function AlertDetails({ group }: { group: AlertGroup }) {
  const [open, setOpen] = useState(() => isSerious(group) || group.cards.length <= OPEN_ALERT_LIMIT)
  return (
    <details data-serious={isSerious(group)} open={open} onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary>{group.title}</summary>
      <p>{group.cards.join(' · ')}</p>
    </details>
  )
}

/** The query key follows `updated_at`, so the text follows each change. */
function ExportPanel({ deck }: { deck: Selection }) {
  const exported = useQuery({
    queryKey: ['export', deck.selection_id, deck.updated_at],
    queryFn: () => fetchSelectionExport(deck.selection_id),
  })
  const [copied, setCopied] = useState<'done' | 'failed' | null>(null)
  const text = exported.data?.text ?? ''

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text)
      setCopied('done')
      window.setTimeout(() => setCopied(null), 2000)
    } catch {
      setCopied('failed')
    }
  }
  const download = () => {
    const url = URL.createObjectURL(new Blob([`${text}\n`], { type: 'text/plain' }))
    const link = document.createElement('a')
    link.href = url
    link.download = exportFileName(deck.name)
    link.click()
    URL.revokeObjectURL(url)
  }

  return (
    <details className="export">
      <summary>Export for Moxfield</summary>
      <p className="hint">
        In Moxfield, click + New Deck, choose the Commander format and your commander, then paste this list. The commanders come
        first, with the print chosen here.
      </p>
      {exported.isError && <p role="alert">{exported.error.message}</p>}
      <textarea readOnly aria-label="Moxfield list" rows={12} value={text} />
      <p className="actions">
        <button type="button" disabled={text === ''} onClick={() => void copy()}>
          Copy
        </button>
        <button type="button" disabled={text === ''} onClick={download}>
          Download .txt
        </button>
        {copied === 'done' && <span className="hint">Copied.</span>}
        {copied === 'failed' && <span role="alert">The browser refused the copy. Select the list and copy it by hand.</span>}
      </p>
      {exported.data && exported.data.skipped.length > 0 && (
        <p role="alert">{exported.data.skipped.length} cards no longer in the catalog are left out.</p>
      )}
    </details>
  )
}
