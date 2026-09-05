'use client'

import { Link } from '@tanstack/react-router'
import type { ComponentProps } from 'react'

type SiteFrameworkLinkProps = ComponentProps<'a'> & {
  prefetch?: boolean
}

export function SiteFrameworkLink({
  href = '#',
  prefetch = true,
  ...props
}: SiteFrameworkLinkProps) {
  // Fumadocs intentionally exposes native anchor props while TanStack narrows
  // several optional members. Their runtime anchor contracts are compatible.
  const linkProps = props as Omit<
    ComponentProps<typeof Link>,
    'to' | 'preload' | 'activeOptions'
  >

  return (
    <Link
      to={href}
      preload={prefetch ? 'intent' : false}
      activeOptions={{ exact: true }}
      {...linkProps}
    />
  )
}
