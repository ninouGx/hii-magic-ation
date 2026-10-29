import { useMutation, useQueries, useQuery, useQueryClient } from '@tanstack/react-query'
import { useMemo } from 'react'
import { HttpError, addFilteredCards, addSelectionCard, fetchCard, fetchSelection, removeSelectionCard, type Selection } from '../api'
import { membership, type Membership } from './deck'

type CardTarget = { id: string; oracleId: string }

const COLOR_ORDER = ['W', 'U', 'B', 'R', 'G']

const NO_MEMBERS = new Map<string, Membership>()

/** Shares the cache key and the mutation scope of the Selections screen, so edits from both screens arrive in order. */
export function useActiveSelection(selectionId: string | null) {
  const queryClient = useQueryClient()
  const selection = useQuery({
    queryKey: ['selection', selectionId],
    queryFn: () => fetchSelection(selectionId as string),
    enabled: selectionId !== null,
  })
  const scope = { id: `selection-${selectionId}` }
  const applied = (updated: Selection) => {
    queryClient.setQueryData(['selection', updated.selection_id], updated)
    void queryClient.invalidateQueries({ queryKey: ['selections'] })
  }

  // The target travels with each call, so a change of selection while one is queued cannot redirect it.
  const add = useMutation({
    mutationFn: ({ id, oracleId }: CardTarget) => addSelectionCard(id, oracleId),
    onSuccess: applied,
    scope,
  })
  const remove = useMutation({
    mutationFn: ({ id, oracleId }: CardTarget) => removeSelectionCard(id, oracleId),
    onSuccess: applied,
    scope,
  })
  const addAll = useMutation({
    mutationFn: ({ id, filter }: { id: string; filter: unknown }) => addFilteredCards(id, filter),
    onSuccess: (_, { id }) => {
      void queryClient.invalidateQueries({ queryKey: ['selection', id] })
      void queryClient.invalidateQueries({ queryKey: ['selections'] })
    },
    scope,
  })

  const leaders = [selection.data?.commander, selection.data?.partner].flatMap((card) => (card ? [card.oracle_id] : []))
  const leaderCards = useQueries({
    queries: leaders.map((oracleId) => ({ queryKey: ['card', oracleId], queryFn: () => fetchCard(oracleId), staleTime: Infinity })),
  })
  const identityKey = leaderCards.every((card) => card.data)
    ? COLOR_ORDER.filter((color) => leaderCards.some((card) => card.data?.color_identity.includes(color))).join('')
    : null
  const loading = selectionId !== null && selection.isPending
  // Null without a commander, undefined while the selection or its commander cards load.
  const identity = useMemo(
    () => (loading ? undefined : leaders.length === 0 ? null : identityKey === null ? undefined : [...identityKey]),
    [loading, leaders.length, identityKey],
  )

  const members = useMemo(() => (selection.data ? membership(selection.data) : NO_MEMBERS), [selection.data])
  const missing = selection.error instanceof HttpError && selection.error.status === 404
  const current = (mutation: { error: Error | null; variables?: { id: string } }) =>
    mutation.variables?.id === selectionId ? mutation.error : null
  return {
    selection: selectionId === null || missing ? null : (selection.data ?? null),
    error:
      (missing ? null : selection.error) ??
      leaderCards.find((card) => card.error)?.error ??
      current(add) ??
      current(remove) ??
      current(addAll),
    missing,
    identity: selectionId === null || missing ? null : identity,
    members,
    add,
    remove,
    addAll,
  }
}

export type ActiveSelection = ReturnType<typeof useActiveSelection>
