import defaultMdxComponents from 'fumadocs-ui/mdx'
import type { MDXComponents } from 'mdx/types'
import { TerminalPreview } from '@/components/TerminalPreview'

export function getMDXComponents(components?: MDXComponents) {
  return {
    ...defaultMdxComponents,
    TerminalPreview,
    ...components,
  } satisfies MDXComponents
}

export const useMDXComponents = getMDXComponents

declare global {
  type MDXProvidedComponents = ReturnType<typeof getMDXComponents>
}
