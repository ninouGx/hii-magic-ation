import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import SourcesPanel from './SourcesPanel'

const queryClient = new QueryClient()

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <main>
        <h1>hii-magic-ation</h1>
        <SourcesPanel />
      </main>
    </QueryClientProvider>
  )
}
