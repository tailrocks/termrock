import { memo, useMemo } from 'react'
import type { TerminalFrame } from '@/components/preview/model'
import { frameToText } from '@/components/preview/painter'

export type PreviewSemanticStateProps = {
  readonly id: string
  readonly frame: TerminalFrame | null
}

/** A stable semantic frame avoids rebuilding hidden text for visual-only ticks. */
export const PreviewSemanticState = memo(function PreviewSemanticState({
  id,
  frame,
}: PreviewSemanticStateProps) {
  const text = useMemo(() => (frame ? frameToText(frame) : ''), [frame])
  return (
    <pre id={id} data-termrock-semantic-state="1" className="sr-only">
      {text}
    </pre>
  )
})
