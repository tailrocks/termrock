import { createFileRoute, notFound } from '@tanstack/react-router'
import { createServerFn } from '@tanstack/react-start'
import { staticFunctionMiddleware } from '@tanstack/start-static-server-functions'
import { useFumadocsLoader } from 'fumadocs-core/source/client'
import { DocsLayout } from 'fumadocs-ui/layouts/docs'
import {
  DocsBody,
  DocsDescription,
  DocsPage,
  DocsTitle,
} from 'fumadocs-ui/layouts/docs/page'
import { Suspense } from 'react'
import browserCollections from 'collections/browser'
import {
  CatalogFiltersProvider,
  type CatalogFilters,
} from '@/components/catalog/CatalogFilters'
import {
  ComponentDocLayout,
  PatternDocLayout,
} from '@/components/docs/DocDetailLayout'
import { catalogEntryById } from '@/components/docs/model'
import { useMDXComponents } from '@/components/mdx'
import { DocsPageMain } from '@/components/site/SiteLayoutContainer'
import { docsOptions } from '@/lib/layout.shared'
import { source } from '@/lib/source'

export const Route = createFileRoute('/docs/$')({
  validateSearch: (search): CatalogRouteSearch => {
    const q = optionalQuery(search['q'])
    const family = optionalFamily(search['family'])
    return {
      ...(q === undefined ? {} : { q }),
      ...(family === undefined ? {} : { family }),
    }
  },
  loader: async ({ params }) => {
    const slugs = params._splat?.split('/') ?? []
    const data = await serverLoader({ data: slugs })
    await clientLoader.preload(data.path)
    return data
  },
  head: ({ loaderData }) =>
    loaderData
      ? {
          meta: [
            { title: `${loaderData.title} — TermRock` },
            { name: 'description', content: loaderData.description },
          ],
        }
      : {},
  component: Page,
})

type CatalogRouteSearch = Readonly<{
  q?: string
  family?: string
}>

function optionalQuery(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() ? value : undefined
}

function optionalFamily(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() ? value.trim() : undefined
}

const serverLoader = createServerFn({ method: 'GET' })
  .validator((slugs: string[]) => slugs)
  .middleware([staticFunctionMiddleware])
  .handler(async ({ data: slugs }) => {
    const page = source.getPage(slugs)
    if (!page) throw notFound()
    return {
      path: page.path,
      title: page.data.title?.trim() || 'Documentation',
      description:
        page.data.description?.trim() ||
        'Composable, product-neutral terminal UI components for Rust and Ratatui.',
      pageTree: await source.serializePageTree(source.getPageTree()),
    }
  })

const clientLoader = browserCollections.docs.createClientLoader({
  component({ toc, frontmatter, default: MDX }, _props: undefined) {
    const catalogId =
      typeof frontmatter === 'object' &&
      frontmatter !== null &&
      'catalogId' in frontmatter &&
      typeof frontmatter.catalogId === 'string'
        ? frontmatter.catalogId
        : null
    const entry = catalogId ? catalogEntryById(catalogId) : null
    const content = <MDX components={useMDXComponents()} />
    const catalogContent = <DocsBody className="doc-detail-guidance-body">{content}</DocsBody>

    return (
      <DocsPage
        toc={toc}
        full={entry !== null}
        tableOfContent={{ enabled: entry === null }}
        slots={{ container: DocsPageMain }}
      >
        {entry ? (
          entry.entryKind === 'component' ? (
            <ComponentDocLayout entry={entry}>{catalogContent}</ComponentDocLayout>
          ) : (
            <PatternDocLayout entry={entry}>{catalogContent}</PatternDocLayout>
          )
        ) : (
          <>
            <DocsTitle>{frontmatter.title}</DocsTitle>
            <DocsDescription>{frontmatter.description}</DocsDescription>
            <DocsBody>{content}</DocsBody>
          </>
        )}
      </DocsPage>
    )
  },
})

function Page() {
  const data = useFumadocsLoader(Route.useLoaderData())
  const search = Route.useSearch()
  const navigate = Route.useNavigate()
  if (!data) throw new Error('Docs route loader data is unavailable')

  const filters: CatalogFilters = {
    query: search.q ?? '',
    family: search.family ?? '',
  }

  const updateFilters = (next: CatalogFilters): void => {
    const q = next.query.trim() ? next.query : undefined
    const family = next.family || undefined
    void navigate({
      search: {
        ...(q === undefined ? {} : { q }),
        ...(family === undefined ? {} : { family }),
      },
      replace: true,
      resetScroll: false,
    })
  }

  return (
    <DocsLayout {...docsOptions()} tree={data.pageTree}>
      <CatalogFiltersProvider filters={filters} updateFilters={updateFilters}>
        <Suspense>{clientLoader.useContent(data.path)}</Suspense>
      </CatalogFiltersProvider>
    </DocsLayout>
  )
}
