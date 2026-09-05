import {
  useEffect,
  useRef,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
  type RefObject,
} from 'react'

const FOCUSABLE = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

export type PreviewDialogProps = {
  readonly open: boolean
  readonly id: string
  readonly labelId: string
  readonly initialFocusRef: RefObject<HTMLElement | null>
  readonly restoreFocusRef: RefObject<HTMLElement | null>
  readonly onClose: () => void
  readonly children: ReactNode
}

export function PreviewDialog({
  open,
  id,
  labelId,
  initialFocusRef,
  restoreFocusRef,
  onClose,
  children,
}: PreviewDialogProps) {
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    initialFocusRef.current?.focus({ preventScroll: true })
    return () => {
      document.body.style.overflow = previousOverflow
      restoreFocusRef.current?.focus({ preventScroll: true })
    }
  }, [initialFocusRef, open, restoreFocusRef])

  const keyDown = (event: ReactKeyboardEvent<HTMLDivElement>): void => {
    if (!open) return
    if (event.key === 'Escape' && !event.shiftKey) {
      event.preventDefault()
      event.stopPropagation()
      onClose()
      return
    }
    if (event.key !== 'Tab') return
    const focusable = Array.from(
      containerRef.current?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? [],
    ).filter((element) => element.getClientRects().length > 0)
    if (focusable.length === 0) {
      event.preventDefault()
      return
    }
    const first = focusable[0]
    const last = focusable.at(-1)
    const activeIndex = focusable.findIndex(
      (element) => element === document.activeElement,
    )
    if (activeIndex === -1) {
      event.preventDefault()
      if (event.shiftKey) last?.focus()
      else first?.focus()
    } else if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last?.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first?.focus()
    }
  }

  return (
    <div
      ref={containerRef}
      id={id}
      role={open ? 'dialog' : undefined}
      aria-modal={open ? true : undefined}
      aria-labelledby={open ? labelId : undefined}
      data-termrock-preview-dialog={open ? 'open' : 'closed'}
      onKeyDownCapture={keyDown}
      style={
        open
          ? {
              position: 'fixed',
              inset: 12,
              zIndex: 1000,
              display: 'flex',
              flexDirection: 'column',
            }
          : { display: 'contents' }
      }
    >
      {children}
    </div>
  )
}
