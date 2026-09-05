import { loader } from 'fumadocs-core/source'
import type { Folder, Node, Root } from 'fumadocs-core/page-tree'
import { docs } from 'collections/server'
import {
  catalogComponentFamilies,
  catalogComponents,
} from '@/generated/catalog'

function groupComponentFolder(folder: Folder): Folder {
  if (folder.index?.url !== '/docs/components') return folder

  const pagesByUrl = new Map(
    folder.children
      .filter((node): node is Extract<Node, { type: 'page' }> => node.type === 'page')
      .map((node) => [node.url, node]),
  )

  if (pagesByUrl.size !== catalogComponents.length) {
    throw new Error(
      `Component page tree has ${pagesByUrl.size} pages for ${catalogComponents.length} catalog entries`,
    )
  }

  const children = catalogComponentFamilies.map((family): Folder => {
    const familyPages = catalogComponents
      .filter((component) => component.family === family.id)
      .map((component) => {
        const page = pagesByUrl.get(component.href)
        if (!page) throw new Error(`Component page tree is missing ${component.href}`)
        return page
      })

    return {
      type: 'folder',
      $id: `${folder.$id ?? 'components'}:${family.id}`,
      name: family.title,
      description: family.description,
      defaultOpen: false,
      collapsible: true,
      children: familyPages,
    }
  })

  return {
    ...folder,
    defaultOpen: false,
    collapsible: true,
    children,
  }
}

function groupComponentSidebar(root: Root): Root {
  return {
    ...root,
    children: root.children.map((node) =>
      node.type === 'folder' ? groupComponentFolder(node) : node,
    ),
  }
}

export const source = loader({
  source: docs.toFumadocsSource(),
  baseUrl: '/docs',
  pageTree: {
    transformers: [{ root: groupComponentSidebar }],
  },
})
