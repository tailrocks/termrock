import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared'

export function baseOptions(): BaseLayoutProps {
  return {
    nav: { title: 'TermRock', url: '/docs' },
    githubUrl: 'https://github.com/tailrocks/termrock',
    searchToggle: { enabled: false },
  }
}
