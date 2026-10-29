import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useMemo } from 'react'
import { HttpError, addFilteredCards, addSelectionCard, fetchSelection, removeSelectionCard, type Selection } from '../api'
import { membership, type Membership } from './deck'

type CardTarget = { id: string; oracleId: string }

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

  const members = useMemo(() => (selection.data ? membership(selection.data) : NO_MEMBERS), [selection.data])
  const missing = selection.error instanceof HttpError && selection.error.status === 404
  const current = (mutation: { error: Error | null; variables?: { id: string } }) =>
    mutation.variables?.id === selectionId ? mutation.error : null
  return {
    selection: selectionId === null || missing ? null : (selection.data ?? null),
    error: (missing ? null : selection.error) ?? current(add) ?? current(remove) ?? current(addAll),
    missing,
    members,
    add,
    remove,
    addAll,
  }
}

export type ActiveSelection = ReturnType<typeof useActiveSelection>
