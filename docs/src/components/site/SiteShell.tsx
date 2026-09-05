'use client'

import { Link, useLocation } from '@tanstack/react-router'
import { SidebarTrigger } from 'fumadocs-ui/layouts/docs/slots/sidebar'
import {
  FullSearchTrigger,
  SearchTrigger,
} from 'fumadocs-ui/layouts/shared/slots/search-trigger'
import { ThemeSwitch } from 'fumadocs-ui/layouts/shared/slots/theme-switch'

const GITHUB_URL = 'https://github.com/tailrocks/termrock'

function MenuGlyph() {
  return (
    <span className="site-shell__menu-glyph" aria-hidden="true">
      <span />
    </span>
  )
}

export function SiteBrand() {
  return (
    <span className="site-brand">
      <span className="site-brand__mark" aria-hidden="true">
        TR
      </span>
      <span>TermRock</span>
    </span>
  )
}

type PrimarySection = 'components' | 'patterns' | 'docs'

const sectionPaths = {
  components: '/docs/components',
  patterns: '/docs/patterns',
  docs: '/docs',
} as const satisfies Record<PrimarySection, string>

function normalizedPathname(pathname: string): string {
  return pathname.replace(/\/+$/, '') || '/'
}

function matchesSection(pathname: string, section: PrimarySection): boolean {
  const sectionPath = sectionPaths[section]
  return pathname === sectionPath || pathname.startsWith(`${sectionPath}/`)
}

function activeSection(pathname: string): PrimarySection | undefined {
  const normalized = normalizedPathname(pathname)

  if (matchesSection(normalized, 'components')) return 'components'
  if (matchesSection(normalized, 'patterns')) return 'patterns'
  if (matchesSection(normalized, 'docs')) return 'docs'
  return undefined
}

function sectionCurrent(
  pathname: string,
  section: PrimarySection,
): 'page' | 'location' | undefined {
  const normalized = normalizedPathname(pathname)
  if (normalized === sectionPaths[section]) return 'page'
  return activeSection(normalized) === section ? 'location' : undefined
}

function NavigationLinks({ pathname }: { pathname: string }) {
  return (
    <>
      <li>
        <Link
          to="/docs/$"
          params={{ _splat: 'components' }}
          preload="intent"
          activeOptions={{ exact: true }}
          className="site-shell__link"
          aria-current={sectionCurrent(pathname, 'components')}
        >
          Components
        </Link>
      </li>
      <li>
        <Link
          to="/docs/$"
          params={{ _splat: 'patterns' }}
          preload="intent"
          activeOptions={{ exact: true }}
          className="site-shell__link"
          aria-current={sectionCurrent(pathname, 'patterns')}
        >
          Patterns
        </Link>
      </li>
      <li>
        <Link
          to="/docs/$"
          params={{ _splat: '' }}
          preload="intent"
          activeOptions={{ exact: true }}
          className="site-shell__link"
          aria-current={sectionCurrent(pathname, 'docs')}
        >
          Docs
        </Link>
      </li>
    </>
  )
}

function MobileNavigationLinks({ pathname }: { pathname: string }) {
  const section = activeSection(pathname)

  return (
    <>
      <li>
        <a
          href="/docs/components"
          className="site-shell__link"
          data-current={section === 'components' || undefined}
        >
          Components
        </a>
      </li>
      <li>
        <a
          href="/docs/patterns"
          className="site-shell__link"
          data-current={section === 'patterns' || undefined}
        >
          Patterns
        </a>
      </li>
      <li>
        <a
          href="/docs"
          className="site-shell__link"
          data-current={section === 'docs' || undefined}
        >
          Docs
        </a>
      </li>
    </>
  )
}

function SiteShell({ layout }: { layout: 'home' | 'docs' }) {
  const pathname = useLocation({ select: (location) => location.pathname })
  const shellClassName = layout === 'docs' ? 'site-shell site-shell--docs' : 'site-shell'

  return (
    <header className={shellClassName}>
      <div className="site-shell__inner">
        <Link
          to="/"
          preload="intent"
          activeOptions={{ exact: true }}
          className="site-shell__brand-link"
        >
          <SiteBrand />
        </Link>

        {layout === 'home' ? (
          <nav className="site-shell__nav" aria-label="Primary navigation">
            <ul className="site-shell__links">
              <NavigationLinks pathname={pathname} />
            </ul>
          </nav>
        ) : null}

        <div className="site-shell__actions">
          <div className="site-shell__search-full">
            <FullSearchTrigger hideIfDisabled />
          </div>
          <div className="site-shell__search-compact">
            <SearchTrigger hideIfDisabled />
          </div>
          <ThemeSwitch />
          <a
            className="site-shell__source site-shell__source-action"
            href={GITHUB_URL}
            target="_blank"
            rel="noreferrer noopener"
            aria-label="TermRock source on GitHub"
          >
            GitHub
          </a>

          {layout === 'docs' ? (
            <SidebarTrigger
              className="site-shell__docs-menu"
              aria-label="Open documentation navigation"
            >
              <MenuGlyph />
            </SidebarTrigger>
          ) : (
            <details className="site-shell__menu">
              <summary aria-label="Open primary navigation">
                <MenuGlyph />
              </summary>
              <nav className="site-shell__menu-panel" aria-label="Mobile navigation">
                <ul className="site-shell__mobile-links">
                  <MobileNavigationLinks pathname={pathname} />
                  <li>
                    <a
                      className="site-shell__source"
                      href={GITHUB_URL}
                      target="_blank"
                      rel="noreferrer noopener"
                    >
                      GitHub
                    </a>
                  </li>
                </ul>
              </nav>
            </details>
          )}
        </div>
      </div>
    </header>
  )
}

export function HomeSiteShell() {
  return <SiteShell layout="home" />
}

export function DocsSiteShell() {
  return <SiteShell layout="docs" />
}
