import { createFileRoute, Link } from '@tanstack/react-router'
import { HomeLayout } from 'fumadocs-ui/layouts/home'
import { baseOptions } from '@/lib/layout.shared'

export const Route = createFileRoute('/')({ component: Home })

function Home() {
  return (
    <HomeLayout {...baseOptions()}>
      <main className="mx-auto flex max-w-3xl flex-1 flex-col justify-center px-6 py-16 text-center">
        <p className="mb-3 font-mono text-sm text-fd-primary">Rust · Ratatui · composable</p>
        <h1 className="mb-5 text-4xl font-semibold tracking-tight sm:text-6xl">TermRock</h1>
        <p className="mx-auto mb-8 max-w-2xl text-balance text-fd-muted-foreground">
          Product-neutral terminal components with strong defaults, stable interaction contracts,
          semantic themes, and deterministic previews.
        </p>
        <Link
          to="/docs/$"
          params={{ _splat: '' }}
          className="mx-auto rounded-lg bg-fd-primary px-4 py-2.5 text-sm font-medium text-fd-primary-foreground"
        >
          Browse the catalog
        </Link>
      </main>
    </HomeLayout>
  )
}
