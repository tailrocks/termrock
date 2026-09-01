import type { DemoDescriptor, DemoUpdate } from '@/components/preview/model'

export type PreviewKeyDecision =
  | { readonly kind: 'ignore' }
  | { readonly kind: 'exit'; readonly moveFocus: boolean }
  | { readonly kind: 'dispatch'; readonly key: string; readonly shiftKey: boolean }

type PreviewKeyDispatch = Extract<PreviewKeyDecision, { readonly kind: 'dispatch' }>

/** Retains the keydown translation until its matching physical keyup. */
export class PreviewKeyAliases {
  readonly #active = new Map<string, PreviewKeyDispatch>()

  remember(code: string, decision: PreviewKeyDispatch): void {
    this.#active.set(code, decision)
  }

  repeat(code: string): PreviewKeyDispatch | undefined {
    return this.#active.get(code)
  }

  release(code: string): PreviewKeyDispatch | undefined {
    const decision = this.#active.get(code)
    this.#active.delete(code)
    return decision
  }

  clear(): void {
    this.#active.clear()
  }
}

const NAVIGATION_KEYS = new Set([
  'ArrowUp',
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight',
  'Enter',
  'Escape',
  'Tab',
  'Backspace',
  'Delete',
  'Home',
  'End',
  'PageUp',
  'PageDown',
  ' ',
])

export function shouldCapturePreviewKey(
  key: string,
  descriptor: DemoDescriptor | null,
  _capturesTextInput = false,
): boolean {
  if (!descriptor?.interactive) return false
  return NAVIGATION_KEYS.has(key) || key.length === 1
}

/**
 * Plain Tab/Escape always leave browser interaction mode. Shift+Enter and
 * Shift+Escape deliberately forward terminal Tab/Escape without trapping page focus.
 */
export function decidePreviewKey(
  key: string,
  shiftKey: boolean,
  interactionActive: boolean,
  descriptor: DemoDescriptor | null,
  update: DemoUpdate | null,
): PreviewKeyDecision {
  if (!interactionActive || !descriptor?.interactive) return { kind: 'ignore' }
  if (key === 'Tab') return { kind: 'exit', moveFocus: true }
  if (key === 'Escape' && !shiftKey) return { kind: 'exit', moveFocus: false }
  const forwardedKey = key === 'Enter' && shiftKey ? 'Tab' : key
  if (!shouldCapturePreviewKey(forwardedKey, descriptor, update?.capturesTextInput)) {
    return { kind: 'ignore' }
  }
  const alias = shiftKey && (key === 'Enter' || key === 'Escape')
  return { kind: 'dispatch', key: forwardedKey, shiftKey: alias ? false : shiftKey }
}

export type DemoEvent =
  | {
      readonly type: 'key'
      readonly key: string
      readonly kind: 'press' | 'repeat' | 'release'
      readonly shift?: boolean
      readonly ctrl?: boolean
      readonly alt?: boolean
      readonly meta?: boolean
    }
  | {
      readonly type: 'pointer'
      readonly kind: 'move' | 'down' | 'up' | 'drag'
      readonly x: number
      readonly y: number
      readonly button?: 'left' | 'right' | 'middle'
    }
  | {
      readonly type: 'wheel'
      readonly deltaX: number
      readonly deltaY: number
      readonly x: number
      readonly y: number
    }
  | { readonly type: 'paste'; readonly text: string }
  | { readonly type: 'resize'; readonly cols: number; readonly rows: number }
  | { readonly type: 'focus'; readonly focused: boolean }
  | { readonly type: 'tick'; readonly elapsedMs: number }
