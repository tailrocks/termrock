import defaultMdxComponents from 'fumadocs-ui/mdx'
import type { MDXComponents } from 'mdx/types'
import { TerminalPreview } from '@/components/TerminalPreview'
import { PatternGallery } from '@/components/PatternGallery'
import { SeenInApplications } from '@/components/SeenInApplications'

export function getMDXComponents(components?: MDXComponents) {
  return {
    ...defaultMdxComponents,
    TerminalPreview,
    PatternGallery,
    SeenInApplications,
    ...components,
  } satisfies MDXComponents
}

export const useMDXComponents = getMDXComponents

declare global {
  type MDXProvidedComponents = ReturnType<typeof getMDXComponents>
}
