import { useEffect, useState } from 'react'

type Health = { status: string; duckdb_version: string; databases: string[] }

export default function App() {
  const [health, setHealth] = useState<Health | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    fetch('/api/health')
      .then((r) => (r.ok ? r.json() : Promise.reject(new Error(`HTTP ${r.status}`))))
      .then(setHealth)
      .catch((e: Error) => setError(e.message))
  }, [])

  return (
    <main>
      <h1>hii-magic-ation</h1>
      {error && <p role="alert">API unreachable: {error}</p>}
      {health && (
        <p>
          API {health.status}, DuckDB {health.duckdb_version}, databases: {health.databases.join(', ')}
        </p>
      )}
    </main>
  )
}
