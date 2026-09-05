import { createFileRoute } from '@tanstack/react-router'
import { HomeLayout } from 'fumadocs-ui/layouts/home'
import { HomePage } from '@/components/home/HomePage'
import { homeOptions } from '@/lib/layout.shared'

export const Route = createFileRoute('/')({
  head: () => ({
    meta: [
      { title: 'Terminal UI components for Rust — TermRock' },
      {
        name: 'description',
        content:
          'Build polished Ratatui applications from terminal-native Rust components, interaction contracts, and application patterns.',
      },
    ],
  }),
  component: Home,
})

function Home() {
  return (
    <HomeLayout {...homeOptions()}>
      <HomePage />
    </HomeLayout>
  )
}
