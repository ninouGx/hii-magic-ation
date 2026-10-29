import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { shouldRetry } from './api'
import SourcesPanel from './SourcesPanel'
import Explorer from './search/Explorer'
import SelectionsScreen from './selections/SelectionsScreen'

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: shouldRetry } } })

const SCREENS = [
  ['/', 'Explorer'],
  ['/selections', 'Selections'],
  ['/settings', 'Settings'],
] as const

type Path = (typeof SCREENS)[number][0]

const currentPath = (): Path =>
  SCREENS.find(([target]) => target === window.location.pathname)?.[0] ?? '/'

export default function App() {
  const [path, setPath] = useState<Path>(currentPath)

  useEffect(() => {
    const onPop = () => setPath(currentPath())
    window.addEventListener('popstate', onPop)
    return () => window.removeEventListener('popstate', onPop)
  }, [])

  const navigate = (next: Path) => {
    window.history.pushState(null, '', next)
    setPath(next)
  }

  return (
    <QueryClientProvider client={queryClient}>
      <div className="layout">
        <nav aria-label="Screens">
          <strong>hii-magic-ation</strong>
          {SCREENS.map(([target, label]) => (
            <a
              key={target}
              href={target}
              aria-current={path === target ? 'page' : undefined}
              onClick={(event) => {
                event.preventDefault()
                navigate(target)
              }}
            >
              {label}
            </a>
          ))}
        </nav>
        <main>
          {path === '/' && <Explorer />}
          {path === '/selections' && <SelectionsScreen />}
          {path === '/settings' && <SourcesPanel />}
        </main>
      </div>
    </QueryClientProvider>
  )
}
