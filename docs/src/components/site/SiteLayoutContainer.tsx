'use client'

import { useDocsPage } from 'fumadocs-ui/layouts/docs/page'
import { Container as FumadocsDocsLayoutContainer } from 'fumadocs-ui/layouts/docs/slots/container'
import type { ComponentProps, ComponentPropsWithoutRef } from 'react'

function classNames(...values: Array<string | undefined | false>): string {
  return values.filter(Boolean).join(' ')
}

export function HomeLayoutContainer({
  className,
  ...props
}: ComponentPropsWithoutRef<'main'>) {
  return <div {...props} id="nd-home-layout" className={classNames('site-layout-home', className)} />
}

export function DocsLayoutContainer(props: ComponentProps<'div'>) {
  return <FumadocsDocsLayoutContainer {...props} />
}

export function DocsPageMain({ className, ...props }: ComponentProps<'article'>) {
  const { full } = useDocsPage()

  return (
    <main
      {...props}
      id="main-content"
      className={classNames('site-docs-main', className)}
      data-full={full}
      tabIndex={-1}
    />
  )
}
