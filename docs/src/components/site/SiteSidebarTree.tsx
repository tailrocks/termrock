'use client'

import { usePathname } from 'fumadocs-core/framework'
import type { Folder, Node } from 'fumadocs-core/page-tree'
import {
  SidebarFolder,
  SidebarFolderContent,
  SidebarFolderLink,
  SidebarFolderTrigger,
  SidebarItem,
} from 'fumadocs-ui/components/sidebar/base'
import type { SidebarPageTreeComponents } from 'fumadocs-ui/components/sidebar/page-tree'
import type { ReactNode } from 'react'

function normalizedPathname(pathname: string): string {
  return pathname.replace(/\/+$/, '') || '/'
}

function exactPath(pathname: string, url: string): boolean {
  return normalizedPathname(pathname) === normalizedPathname(url)
}

function containsPath(nodes: readonly Node[], pathname: string): boolean {
  return nodes.some((node) => {
    if (node.type === 'page') return exactPath(pathname, node.url)
    if (node.type === 'folder') {
      return (
        (node.index !== undefined && exactPath(pathname, node.index.url)) ||
        containsPath(node.children, pathname)
      )
    }
    return false
  })
}

function pageCount(nodes: readonly Node[]): number {
  return nodes.reduce(
    (count, node) =>
      count +
      (node.type === 'page'
        ? 1
        : node.type === 'folder'
          ? pageCount(node.children) + (node.index ? 1 : 0)
          : 0),
    0,
  )
}

function SiteSidebarItem({ item }: { item: Extract<Node, { type: 'page' }> }) {
  const pathname = usePathname()
  const current = exactPath(pathname, item.url)

  return (
    <SidebarItem
      href={item.url}
      {...(item.external === undefined ? {} : { external: item.external })}
      active={current}
      aria-current={current ? 'page' : undefined}
      className="site-sidebar__item"
      icon={item.icon}
    >
      {item.name}
    </SidebarItem>
  )
}

function SiteSidebarFolder({ item, children }: { item: Folder; children: ReactNode }) {
  const pathname = usePathname()
  const exact = item.index !== undefined && exactPath(pathname, item.index.url)
  const descendant = containsPath(item.children, pathname)
  const active = exact || descendant
  const current = exact ? 'page' : descendant ? 'location' : undefined

  return (
    <SidebarFolder
      active={active}
      {...(item.collapsible === undefined ? {} : { collapsible: item.collapsible })}
      {...(item.defaultOpen === undefined ? {} : { defaultOpen: item.defaultOpen })}
      className="site-sidebar__folder"
    >
      {item.index ? (
        <SidebarFolderLink
          href={item.index.url}
          {...(item.index.external === undefined ? {} : { external: item.index.external })}
          active={exact}
          aria-current={current}
          className="site-sidebar__folder-link"
        >
          <span>{item.name}</span>
          <small>{pageCount(item.children)}</small>
        </SidebarFolderLink>
      ) : (
        <SidebarFolderTrigger
          aria-current={descendant ? 'location' : undefined}
          className="site-sidebar__folder-trigger"
        >
          <span>{item.name}</span>
          <small>{pageCount(item.children)}</small>
        </SidebarFolderTrigger>
      )}
      <SidebarFolderContent className="site-sidebar__folder-content">
        {children}
      </SidebarFolderContent>
    </SidebarFolder>
  )
}

export const siteSidebarComponents = {
  Item: SiteSidebarItem,
  Folder: SiteSidebarFolder,
} satisfies Partial<SidebarPageTreeComponents>
