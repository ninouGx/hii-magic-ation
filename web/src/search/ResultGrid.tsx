import { useInfiniteQuery } from '@tanstack/react-query'
import { useEffect, useRef } from 'react'
import { searchCards } from '../api'
import type { ActiveSelection } from '../selections/activeSelection'
import { AddAllButton, CardActions } from '../selections/SelectionControls'
import CardDetail from './CardDetail'
import CardTile from './CardTile'
import { neighbor } from './neighbor'
import type { ApiGroup } from './fields'
import type { SortKey } from './urlState'

const PAGE_SIZE = 100

type Props = {
  filter: ApiGroup
  enabled: boolean
  sort: SortKey[]
  openCard: string | null
  onOpenCard: (oracleId: string | null) => void
  active: ActiveSelection
}

export default function ResultGrid({ filter, enabled, sort, openCard, onOpenCard, active }: Props) {
  const results = useInfiniteQuery({
    queryKey: ['search', filter, sort],
    queryFn: ({ pageParam, signal }) => searchCards({ filter, sort, offset: pageParam, limit: PAGE_SIZE }, signal),
    initialPageParam: 0,
    getNextPageParam: (last, pages) => {
      const loaded = pages.reduce((count, page) => count + page.cards.length, 0)
      return loaded < last.total ? loaded : undefined
    },
    placeholderData: (previous) => previous,
    enabled,
  })

  const sentinel = useRef<HTMLDivElement>(null)
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = results
  useEffect(() => {
    const target = sentinel.current
    if (!target || !hasNextPage) return
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting && !isFetchingNextPage) void fetchNextPage()
      },
      { rootMargin: '600px' },
    )
    observer.observe(target)
    return () => observer.disconnect()
  }, [hasNextPage, isFetchingNextPage, fetchNextPage])

  const cards = results.data?.pages.flatMap((page) => page.cards) ?? []
  const atLoadedEnd = openCard !== null && cards.at(-1)?.oracle_id === openCard

  useEffect(() => {
    if (atLoadedEnd && hasNextPage && !isFetchingNextPage) void fetchNextPage()
  }, [atLoadedEnd, hasNextPage, isFetchingNextPage, fetchNextPage])

  useEffect(() => {
    if (openCard) document.querySelector(`[data-oracle-id="${CSS.escape(openCard)}"]`)?.scrollIntoView({ block: 'nearest' })
  }, [openCard])
  const total = results.data?.pages[0]?.total

  return (
    <section aria-label="Results">
      <p role="status" className="result-count">
        {results.isError
          ? null
          : total === undefined
            ? 'Searching…'
            : `${total.toLocaleString()} cards${results.isFetching && !isFetchingNextPage ? ' · updating…' : ''}`}
      </p>
      {enabled && total !== undefined && !results.isPlaceholderData && (
        <AddAllButton key={JSON.stringify([filter, active.selection?.selection_id])} filter={filter} total={total} active={active} />
      )}
      {results.isError && <p role="alert">{results.error.message}</p>}
      <ul className="grid">
        {cards.map((card) => (
          <li
            key={card.oracle_id}
            data-oracle-id={card.oracle_id}
            title={card.type_line ?? undefined}
            tabIndex={0}
            aria-label={`Open ${card.name}`}
            onClick={() => onOpenCard(card.oracle_id)}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && event.target === event.currentTarget) onOpenCard(card.oracle_id)
            }}
          >
            <CardTile card={card} />
            <CardActions oracleId={card.oracle_id} name={card.name} active={active} compact />
          </li>
        ))}
      </ul>
      <div ref={sentinel} />
      {isFetchingNextPage && <p>Loading more…</p>}
      {openCard && (
        <CardDetail
          oracleId={openCard}
          previous={neighbor(cards, openCard, -1)}
          next={neighbor(cards, openCard, 1)}
          onNavigate={onOpenCard}
          onClose={() => onOpenCard(null)}
          actions={(name) => <CardActions oracleId={openCard} name={name} active={active} />}
        />
      )}
    </section>
  )
}
