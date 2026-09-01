import type { DemoUpdate } from '@/components/preview/model'

export type PreviewClockPolicy = 'stopped' | 'visual-motion' | 'functional-deadline'

/** Reduced motion suppresses decorative clocks, never state-transition deadlines. */
export function previewClockPolicy(
  update: DemoUpdate | null,
  reducedMotion: boolean,
): PreviewClockPolicy {
  if (update?.nextDeadlineMs == null) return 'stopped'
  if (update.deadlineKind === 'functional') return 'functional-deadline'
  if (update.deadlineKind === 'visual-motion' && !reducedMotion) return 'visual-motion'
  return 'stopped'
}
