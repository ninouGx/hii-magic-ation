import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { fetchCatalog, fetchSync, startSync, type CatalogCounts, type SyncPlan } from './api'

const COUNT_LABELS: [keyof CatalogCounts, string][] = [
  ['cards', 'Cards'],
  ['commanders', 'Commanders'],
  ['prints', 'English prints'],
  ['rulings', 'Rulings'],
  ['oracle_tags', 'Oracle tags'],
  ['art_tags', 'Art tags'],
  ['card_tags', 'Taggings'],
  ['edhrec_ranks', 'EDHREC ranks'],
  ['edhrec_unresolved', 'EDHREC unresolved'],
  ['cards_without_print', 'Cards without English print'],
]

const PLANS: [string, Omit<SyncPlan, 'rebuild'>][] = [
  ['Sync all', { scryfall: true, edhrec: true }],
  ['Scryfall only', { scryfall: true, edhrec: false }],
  ['EDHREC only', { scryfall: false, edhrec: true }],
]

const formatTime = (unix: number | null) =>
  unix === null ? '' : new Date(unix * 1000).toLocaleTimeString()

export default function SourcesPanel() {
  const queryClient = useQueryClient()
  const [rebuild, setRebuild] = useState(false)
  const catalog = useQuery({ queryKey: ['catalog'], queryFn: fetchCatalog })
  const sync = useQuery({
    queryKey: ['sync'],
    queryFn: fetchSync,
    refetchInterval: (query) => (query.state.data?.state === 'running' ? 2000 : false),
  })
  const start = useMutation({
    mutationFn: startSync,
    onSuccess: (status) => queryClient.setQueryData(['sync'], status),
  })

  const syncState = sync.data?.state
  useEffect(() => {
    if (syncState === 'done') {
      void queryClient.invalidateQueries({ queryKey: ['catalog'] })
    }
  }, [syncState, queryClient])

  const running = syncState === 'running' || start.isPending
  const counts = catalog.data?.counts

  return (
    <section className="panel">
      <h2>Sources</h2>

      <div className="actions">
        {PLANS.map(([label, plan]) => (
          <button
            key={label}
            type="button"
            disabled={running}
            onClick={() => start.mutate({ ...plan, rebuild })}
          >
            {label}
          </button>
        ))}
        <label>
          <input
            type="checkbox"
            checked={rebuild}
            onChange={(event) => setRebuild(event.target.checked)}
          />
          Rebuild catalog even without changes
        </label>
      </div>

      <p role="status">
        {running && `Sync running since ${formatTime(sync.data?.started_at ?? null)}…`}
        {syncState === 'done' && `Last sync finished at ${formatTime(sync.data?.finished_at ?? null)}.`}
        {syncState === 'failed' && `Last sync failed: ${sync.data?.error}`}
        {start.error && `Sync refused: ${start.error.message}`}
      </p>

      {catalog.error && <p role="alert">Catalog unavailable: {catalog.error.message}</p>}
      {catalog.data && !catalog.data.ready && <p>No catalog yet. Run a first sync.</p>}

      {counts && (
        <table>
          <tbody>
            {COUNT_LABELS.map(([key, label]) => (
              <tr key={key}>
                <th scope="row">{label}</th>
                <td>{counts[key].toLocaleString()}</td>
              </tr>
            ))}
            {catalog.data?.edhrec_mapping_rate != null && (
              <tr>
                <th scope="row">EDHREC mapping rate</th>
                <td>{(catalog.data.edhrec_mapping_rate * 100).toFixed(2)} %</td>
              </tr>
            )}
          </tbody>
        </table>
      )}

      {catalog.data && Object.keys(catalog.data.bulk_versions).length > 0 && (
        <>
          <h3>Scryfall bulk versions</h3>
          <ul>
            {Object.entries(catalog.data.bulk_versions).map(([bulk, version]) => (
              <li key={bulk}>
                {bulk}: {new Date(version).toLocaleString()}
              </li>
            ))}
          </ul>
        </>
      )}

      {catalog.data && catalog.data.edhrec_unresolved.length > 0 && (
        <>
          <h3>Unresolved EDHREC entries</h3>
          <ul>
            {catalog.data.edhrec_unresolved.map((entry) => (
              <li key={`${entry.period}-${entry.rank}-${entry.name}`}>
                {entry.period} #{entry.rank}: {entry.name}
              </li>
            ))}
          </ul>
        </>
      )}
    </section>
  )
}
