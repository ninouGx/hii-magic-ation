import { useInfiniteQuery } from '@tanstack/react-query'
import { useEffect, useRef } from 'react'
import { searchCards } from '../api'
import CardTile from './CardTile'
import type { ApiGroup } from './fields'
import type { SortKey } from './urlState'

const PAGE_SIZE = 100

type Props = { filter: ApiGroup; sort: SortKey[] }

export default function ResultGrid({ filter, sort }: Props) {
  const results = useInfiniteQuery({
    queryKey: ['search', filter, sort],
    queryFn: ({ pageParam, signal }) => searchCards({ filter, sort, offset: pageParam, limit: PAGE_SIZE }, signal),
    initialPageParam: 0,
    getNextPageParam: (last, pages) => {
      const loaded = pages.reduce((count, page) => count + page.cards.length, 0)
      return loaded < last.total ? loaded : undefined
    },
    placeholderData: (previous) => previous,
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
      {results.isError && <p role="alert">{results.error.message}</p>}
      <ul className="grid">
        {cards.map((card) => (
          <li key={card.oracle_id} title={card.type_line ?? undefined}>
            <CardTile card={card} />
          </li>
        ))}
      </ul>
      <div ref={sentinel} />
      {isFetchingNextPage && <p>Loading more…</p>}
    </section>
  )
}
