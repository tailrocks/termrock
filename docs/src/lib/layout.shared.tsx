import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared'
import {
  DocsLayoutContainer,
  HomeLayoutContainer,
} from '@/components/site/SiteLayoutContainer'
import { DocsSiteShell, HomeSiteShell, SiteBrand } from '@/components/site/SiteShell'
import { siteSidebarComponents } from '@/components/site/SiteSidebarTree'

type SiteLayoutOptions = Omit<BaseLayoutProps, 'slots'>

type HomeSiteLayoutOptions = SiteLayoutOptions & {
  slots: {
    header: typeof HomeSiteShell
    container: typeof HomeLayoutContainer
  }
}

type DocsSiteLayoutOptions = SiteLayoutOptions & {
  sidebar: {
    components: typeof siteSidebarComponents
    defaultOpenLevel: number
  }
  slots: {
    header: typeof DocsSiteShell
    container: typeof DocsLayoutContainer
  }
}

const sourceLink = {
  type: 'button',
  text: 'GitHub',
  url: 'https://github.com/tailrocks/termrock',
  external: true,
  secondary: true,
} as const

function siteOptions(): SiteLayoutOptions {
  return {
    nav: {
      title: <SiteBrand />,
      url: '/',
    },
    links: [
      {
        text: 'Components',
        url: '/docs/components',
        active: 'nested-url',
      },
      {
        text: 'Patterns',
        url: '/docs/patterns',
        active: 'nested-url',
      },
      {
        text: 'Docs',
        url: '/docs',
        active: 'url',
      },
      sourceLink,
    ],
    searchToggle: { enabled: true },
    themeSwitch: { enabled: true },
  }
}

export function homeOptions(): HomeSiteLayoutOptions {
  return {
    ...siteOptions(),
    slots: {
      header: HomeSiteShell,
      container: HomeLayoutContainer,
    },
  }
}

export function docsOptions(): DocsSiteLayoutOptions {
  return {
    ...siteOptions(),
    links: [sourceLink],
    sidebar: {
      components: siteSidebarComponents,
      defaultOpenLevel: 0,
    },
    slots: {
      header: DocsSiteShell,
      container: DocsLayoutContainer,
    },
  }
}
