/** Cell payload from shared Rust demo runtime (truecolor RGB). */
export type FrameCell = {
  readonly ch: string
  readonly fg: [number, number, number]
  readonly bg: [number, number, number]
  readonly bold?: boolean
  readonly dim?: boolean
  readonly underline?: boolean
  readonly reversed?: boolean
  readonly italic?: boolean
  readonly strike?: boolean
}

export type TerminalFrame = {
  readonly story_id: string
  readonly title: string
  readonly component: string
  readonly cols: number
  readonly rows: number
  readonly story_cols: number
  readonly story_rows: number
  readonly cells: FrameCell[]
  /** Canonical Rust hardware-cursor cell, when one is present. */
  readonly cursor: readonly [number, number] | null
  /** Canonical Rust hardware-cursor visibility. */
  readonly cursor_visible: boolean
  readonly interactive: boolean
  readonly theme: string
}

export type DemoDescriptor = {
  readonly id: string
  readonly title: string
  readonly component: string
  readonly description: string
  readonly cols: number
  readonly rows: number
  readonly interactive: boolean
  readonly interactionKind: string
  readonly hints: string[]
}

export type DemoUpdate = {
  readonly changed: boolean
  readonly outcome: string | null
  readonly hints: string[]
  readonly interactive: boolean
  readonly capturesTextInput: boolean
  readonly nextDeadlineMs: number | null
  readonly deadlineKind: 'visual-motion' | 'functional' | null
  readonly semanticRevision: number
}

export type PreviewSource = 'poster-loading' | 'static-poster' | 'rust-wasm' | 'failed'

export type PreviewView = 'preview' | 'code'
