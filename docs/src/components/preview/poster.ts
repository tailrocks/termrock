import type { DemoDescriptor, TerminalFrame } from '@/components/preview/model'

export async function loadPosterFrame(
  story: string,
  signal?: AbortSignal,
): Promise<TerminalFrame> {
  const slug = story.replaceAll('/', '-')
  const response = await fetch(
    `/preview-posters/${slug}.json`,
    signal ? { signal } : undefined,
  )
  if (!response.ok) throw new Error(`poster ${response.status}`)
  return response.json() as Promise<TerminalFrame>
}

export function posterDescriptor(frame: TerminalFrame): DemoDescriptor {
  return {
    id: frame.story_id,
    title: frame.title,
    component: frame.component,
    description: 'Static preview before explicit live runtime activation.',
    cols: frame.story_cols,
    rows: frame.story_rows,
    interactive: frame.interactive,
    interactionKind: 'passive-paint',
    hints: [],
  }
}
