import { createRootRoute, HeadContent, Outlet, Scripts } from '@tanstack/react-router'
import { createServerFn } from '@tanstack/react-start'
import { staticFunctionMiddleware } from '@tanstack/start-static-server-functions'
import { flattenTree } from 'fumadocs-core/page-tree'
import { RootProvider } from 'fumadocs-ui/provider/tanstack'
import { SiteNotFound } from '@/components/site/SiteNotFound'
import { SiteFrameworkLink } from '@/components/site/SiteFrameworkLink'
import {
  GuideSearchProvider,
  SiteSearch,
  type GuideSearchEntry,
} from '@/components/site/SiteSearch'
import { source } from '@/lib/source'
import appCss from '@/styles/app.css?url'

function isGuideUrl(url: string): boolean {
  return !url.startsWith('/docs/components/') && !url.startsWith('/docs/patterns/')
}

const guideSearchLoader = createServerFn({ method: 'GET' })
  .middleware([staticFunctionMiddleware])
  .handler(async (): Promise<readonly GuideSearchEntry[]> =>
    flattenTree(source.getPageTree().children)
      .filter((node) => isGuideUrl(node.url))
      .map((node) => {
        const page = source.getNodePage(node)
        if (!page) throw new Error(`Guide search page is missing for ${node.url}`)

        return {
          id: `guide-${page.slugs.join('-') || 'overview'}`,
          label: page.data.title.trim(),
          url: node.url,
          group: 'Guide',
          purpose: page.data.description?.trim() ?? '',
          tags: page.slugs,
          aliases: [],
        }
      }),
  )

export const Route = createRootRoute({
  loader: async () => ({ guideSearchEntries: await guideSearchLoader() }),
  head: () => ({
    meta: [
      { charSet: 'utf-8' },
      { name: 'viewport', content: 'width=device-width, initial-scale=1' },
      {
        name: 'description',
        content: 'Composable, product-neutral terminal UI components for Rust and Ratatui.',
      },
      { name: 'color-scheme', content: 'dark light' },
      { title: 'TermRock' },
    ],
    links: [
      { rel: 'stylesheet', href: appCss },
      { rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' },
    ],
  }),
  component: RootComponent,
  notFoundComponent: SiteNotFound,
})

function RootComponent() {
  const { guideSearchEntries } = Route.useLoaderData()

  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <HeadContent />
      </head>
      <body className="flex min-h-screen flex-col">
        <a className="site-skip-link" href="#main-content">
          Skip to main content
        </a>
        <GuideSearchProvider entries={guideSearchEntries}>
          <RootProvider
            components={{ Link: SiteFrameworkLink }}
            search={{ enabled: true, SearchDialog: SiteSearch }}
            theme={{ defaultTheme: 'system', enableSystem: true }}
          >
            <Outlet />
          </RootProvider>
        </GuideSearchProvider>
        <Scripts />
      </body>
    </html>
  )
}
