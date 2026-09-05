'use client'

import {
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type WheelEvent as ReactWheelEvent,
} from 'react'
import { PreviewStatus, PreviewToolbar } from '@/components/preview/controls'
import { PreviewDialog } from '@/components/preview/dialog'
import {
  PreviewKeyAliases,
  decidePreviewKey,
  pointerCleanupEvents,
} from '@/components/preview/input'
import type {
  DemoDescriptor,
  DemoUpdate,
  PreviewView,
  TerminalFrame,
} from '@/components/preview/model'
import {
  DEFAULT_CELL_H,
  DEFAULT_CELL_W,
  paintCanvas,
  pointerCell,
  storySizeForCssHost,
} from '@/components/preview/painter'
import { previewClockPolicy } from '@/components/preview/motion'
import { loadDemoCode } from '@/components/preview/runtime'
import { PreviewSemanticState } from '@/components/preview/semantic'
import { usePreviewSession } from '@/components/preview/session'
import {
  PREVIEW_MONO_STACK,
  fontSizeForCell,
  formatCellProbe,
  paintDpr,
} from '@/components/preview-metrics'

export type TerminalPreviewProps = {
  readonly story: string
  readonly caption?: string
  readonly maxHeight?: number
  readonly interactive?: boolean
}

/**
 * Live terminal host. Rust owns persistent demo behavior; React owns browser
 * activation, accessible state, event translation, canvas paint, and modal focus.
 */
export function TerminalPreview({
  story,
  caption,
  maxHeight = 420,
  interactive = true,
}: TerminalPreviewProps) {
  const figureRef = useRef<HTMLElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const hostRef = useRef<HTMLDivElement>(null)
  const textSinkRef = useRef<HTMLTextAreaElement>(null)
  const stageRef = useRef<HTMLDivElement>(null)
  const interactionButtonRef = useRef<HTMLButtonElement>(null)
  const frameRef = useRef<TerminalFrame | null>(null)
  const descriptorRef = useRef<DemoDescriptor | null>(null)
  const updateRef = useRef<DemoUpdate | null>(null)
  const dragRef = useRef(false)
  const interactionFocusRequestedRef = useRef(false)
  const interactionBeforeFullscreenRef = useRef(false)
  const fullscreenRestoreFocusRef = useRef<HTMLElement | null>(null)
  const keyAliasesRef = useRef(new PreviewKeyAliases())
  const lastGridRef = useRef({ cols: 0, rows: 0 })
  const [runtimeRequested, setRuntimeRequested] = useState(false)
  const [visible, setVisible] = useState(false)
  const [reducedMotion, setReducedMotion] = useState(false)
  const [focused, setFocused] = useState(false)
  const [interactionActive, setInteractionActive] = useState(false)
  const [hover, setHover] = useState<{ readonly x: number; readonly y: number } | null>(
    null,
  )
  const [fullscreen, setFullscreen] = useState(false)
  const [view, setView] = useState<PreviewView>('preview')
  const [activeStory, setActiveStory] = useState(story)
  const titleId = useId()
  const instructionsId = useId()
  const semanticStateId = useId()
  const dialogId = useId()
  const session = usePreviewSession(activeStory, runtimeRequested)
  const {
    frame,
    semanticFrame,
    descriptor,
    update,
    catalog,
    source,
    loading,
    error,
    dispatch,
    reset,
    getMountedAt,
  } = session

  useEffect(() => {
    setActiveStory(story)
    setView('preview')
    setInteractionActive(false)
    setFullscreen(false)
    interactionFocusRequestedRef.current = false
    interactionBeforeFullscreenRef.current = false
    fullscreenRestoreFocusRef.current = null
    keyAliasesRef.current.clear()
  }, [story])

  frameRef.current = frame
  descriptorRef.current = descriptor
  updateRef.current = update
  const clockPolicy = previewClockPolicy(update, reducedMotion)

  useEffect(() => {
    const figure = figureRef.current
    if (!figure) return
    if (typeof IntersectionObserver === 'undefined') {
      setVisible(true)
      return
    }
    const observer = new IntersectionObserver(
      ([entry]) => {
        const nextVisible = Boolean(entry?.isIntersecting)
        setVisible(nextVisible)
      },
      { rootMargin: '160px' },
    )
    observer.observe(figure)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    const preference = window.matchMedia('(prefers-reduced-motion: reduce)')
    const syncPreference = (): void => setReducedMotion(preference.matches)
    syncPreference()
    preference.addEventListener('change', syncPreference)
    return () => preference.removeEventListener('change', syncPreference)
  }, [])

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas || !frame) return
    let cancelled = false
    const paint = (): void => {
      if (cancelled) return
      const raw = window.devicePixelRatio || 1
      paintCanvas(canvas, frame, DEFAULT_CELL_W, DEFAULT_CELL_H, paintDpr(raw))
    }
    paint()
    void document.fonts?.ready.then(paint)
    void document.fonts
      ?.load(String(fontSizeForCell(DEFAULT_CELL_H)) + 'px JetBrains Mono')
      .then(paint)
      .catch(() => undefined)
    return () => {
      cancelled = true
    }
  }, [frame])

  useEffect(() => {
    const stage = stageRef.current
    if (!stage || !descriptor || view !== 'preview' || source !== 'rust-wasm') return
    let animationFrame = 0
    const resize = (): void => {
      const width = Math.max(72, stage.clientWidth - 24)
      const height = Math.max(72, Math.min(maxHeight, stage.clientHeight) - 24)
      const { storyCols, storyRows } = storySizeForCssHost(
        width,
        height,
        DEFAULT_CELL_W,
        DEFAULT_CELL_H,
      )
      if (
        lastGridRef.current.cols === storyCols &&
        lastGridRef.current.rows === storyRows
      ) {
        return
      }
      lastGridRef.current = { cols: storyCols, rows: storyRows }
      dispatch({ type: 'resize', cols: storyCols, rows: storyRows }, 'always')
    }
    const schedule = (): void => {
      cancelAnimationFrame(animationFrame)
      animationFrame = requestAnimationFrame(resize)
    }
    const observer = new ResizeObserver(schedule)
    observer.observe(stage)
    lastGridRef.current = { cols: 0, rows: 0 }
    schedule()
    return () => {
      cancelAnimationFrame(animationFrame)
      observer.disconnect()
    }
  }, [descriptor, dispatch, maxHeight, source, view])

  useEffect(() => {
    if (
      source !== 'rust-wasm' ||
      view !== 'preview' ||
      !visible ||
      clockPolicy === 'stopped'
    ) {
      return
    }
    let animationFrame = 0
    let previousStep = -1
    const animate = (now: number): void => {
      if (document.visibilityState === 'visible') {
        const elapsedMs = Math.max(0, Math.floor(now - getMountedAt()))
        const step = Math.floor(elapsedMs / 100)
        if (step !== previousStep) {
          previousStep = step
          dispatch(
            { type: 'tick', elapsedMs },
            clockPolicy === 'visual-motion' ? 'always' : 'changed',
          )
        }
      }
      animationFrame = requestAnimationFrame(animate)
    }
    animationFrame = requestAnimationFrame(animate)
    return () => cancelAnimationFrame(animationFrame)
  }, [
    clockPolicy,
    dispatch,
    getMountedAt,
    source,
    visible,
    view,
  ])

  useEffect(() => {
    const host = hostRef.current
    if (!host || !interactionActive) return
    const beforeInput = (event: InputEvent): void => {
      if (
        (!updateRef.current?.capturesTextInput &&
          descriptorRef.current?.interactionKind !== 'editor-form') ||
        !event.data
      ) {
        return
      }
      event.preventDefault()
      for (const value of event.data) dispatch({ type: 'key', key: value, kind: 'press' })
    }
    const paste = (event: ClipboardEvent): void => {
      if (
        !updateRef.current?.capturesTextInput &&
        descriptorRef.current?.interactionKind !== 'editor-form'
      ) {
        return
      }
      const text = event.clipboardData?.getData('text')
      if (!text) return
      event.preventDefault()
      dispatch({ type: 'paste', text })
    }
    host.addEventListener('beforeinput', beforeInput)
    host.addEventListener('paste', paste)
    return () => {
      host.removeEventListener('beforeinput', beforeInput)
      host.removeEventListener('paste', paste)
    }
  }, [dispatch, interactionActive])

  const canInteract = Boolean(
    interactive && source === 'rust-wasm' && descriptor?.interactive,
  )
  const canRequestInteraction = Boolean(interactive && descriptor?.interactive)
  const animated = descriptor?.interactionKind === 'timed-state'
  const capturesTextInput = Boolean(update?.capturesTextInput)
  const variants = descriptor
    ? catalog.filter((entry) => entry.component === descriptor.component)
    : []
  const [sourceCode, setSourceCode] = useState('// Loading exact shared Rust source…')

  useEffect(() => {
    if (!canInteract && interactionActive) setInteractionActive(false)
  }, [canInteract, interactionActive])

  useEffect(() => {
    if (view !== 'code') return
    let cancelled = false
    setSourceCode('// Loading exact shared Rust source…')
    void loadDemoCode()
      .then((code) => {
        if (cancelled) return
        setSourceCode(
          code[activeStory] ??
            code[story] ??
            `// Source setup is documented on the canonical ${descriptor?.component ?? 'component'} page.`,
        )
      })
      .catch(() => {
        if (!cancelled) setSourceCode('// Exact shared source could not be loaded.')
      })
    return () => {
      cancelled = true
    }
  }, [activeStory, descriptor?.component, story, view])

  useLayoutEffect(() => {
    if (!capturesTextInput || !focused || !interactionActive) return
    textSinkRef.current?.focus({ preventScroll: true })
  }, [capturesTextInput, focused, interactionActive])

  useLayoutEffect(() => {
    if (!interactionActive || !interactionFocusRequestedRef.current) return
    interactionFocusRequestedRef.current = false
    const target = capturesTextInput ? textSinkRef.current : hostRef.current
    target?.focus({ preventScroll: true })
  }, [capturesTextInput, interactionActive])

  const clearPointerState = useCallback((): void => {
    dragRef.current = false
    setHover(null)
    if (!canInteract) return
    const current = frameRef.current
    if (!current) return
    for (const event of pointerCleanupEvents(current)) dispatch(event)
  }, [canInteract, dispatch])

  const exitInteraction = useCallback(
    (restoreEntryFocus: boolean): void => {
      interactionFocusRequestedRef.current = false
      keyAliasesRef.current.clear()
      clearPointerState()
      setInteractionActive(false)
      dispatch({ type: 'focus', focused: false })
      if (restoreEntryFocus) {
        requestAnimationFrame(() => interactionButtonRef.current?.focus({ preventScroll: true }))
      }
    },
    [clearPointerState, dispatch],
  )

  const enterInteraction = useCallback((): void => {
    interactionFocusRequestedRef.current = true
    setRuntimeRequested(true)
    if (!canInteract) return
    setInteractionActive(true)
  }, [canInteract])

  useEffect(() => {
    if (
      !canInteract ||
      interactionActive ||
      !interactionFocusRequestedRef.current
    ) {
      return
    }
    setInteractionActive(true)
  }, [canInteract, interactionActive])

  const closeFullscreen = useCallback((): void => {
    const restoreInteraction = interactionBeforeFullscreenRef.current
    interactionBeforeFullscreenRef.current = false
    setFullscreen(false)
    if (restoreInteraction && canInteract) {
      fullscreenRestoreFocusRef.current = capturesTextInput
        ? textSinkRef.current
        : hostRef.current
      setInteractionActive(true)
    } else {
      exitInteraction(false)
    }
  }, [canInteract, capturesTextInput, exitInteraction])

  const toggleFullscreen = useCallback((): void => {
    if (fullscreen) {
      closeFullscreen()
      return
    }
    interactionBeforeFullscreenRef.current = interactionActive
    fullscreenRestoreFocusRef.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null
    if (canInteract && !interactionActive) enterInteraction()
    setFullscreen(true)
  }, [canInteract, closeFullscreen, enterInteraction, fullscreen, interactionActive])

  const keyEvent = (
    event: ReactKeyboardEvent,
    kind: 'press' | 'repeat' | 'release',
  ): void => {
    if (
      event.target !== hostRef.current &&
      event.target !== textSinkRef.current
    ) {
      return
    }
    if (fullscreen && event.key === 'Tab') return
    if (
      capturesTextInput &&
      (event.ctrlKey || event.metaKey) &&
      event.key.toLowerCase() === 'v'
    ) return
    if (
      capturesTextInput &&
      event.key.length === 1 &&
      !event.ctrlKey &&
      !event.altKey &&
      !event.metaKey
    ) return
    const physicalKey = event.code || event.key
    const decide = () =>
      decidePreviewKey(
        event.key,
        event.shiftKey,
        interactionActive,
        descriptor,
        update,
      )
    const decision =
      kind === 'release'
        ? keyAliasesRef.current.release(physicalKey) ?? decide()
        : kind === 'repeat'
          ? keyAliasesRef.current.repeat(physicalKey) ?? decide()
          : decide()
    if (decision.kind === 'ignore') return
    if (decision.kind === 'exit') {
      if (kind !== 'press') return
      if (!decision.moveFocus) {
        event.preventDefault()
        event.stopPropagation()
      }
      exitInteraction(!decision.moveFocus)
      return
    }
    if (kind === 'press') keyAliasesRef.current.remember(physicalKey, decision)
    event.preventDefault()
    event.stopPropagation()
    dispatch({
      type: 'key',
      key: decision.key,
      kind,
      shift: decision.shiftKey,
      ctrl: event.ctrlKey,
      alt: event.altKey,
      meta: event.metaKey,
    })
  }

  const eventCell = (
    event: { readonly clientX: number; readonly clientY: number },
  ): { readonly x: number; readonly y: number } | null => {
    const canvas = canvasRef.current
    const current = frameRef.current
    if (!canvas || !current) return null
    return pointerCell(
      event.clientX,
      event.clientY,
      canvas,
      current,
      DEFAULT_CELL_W,
      DEFAULT_CELL_H,
    )
  }

  const pointerMove = (event: ReactPointerEvent): void => {
    const cell = eventCell(event)
    if (!cell) {
      clearPointerState()
      return
    }
    setHover(cell)
    if (!canInteract) return
    dispatch({
      type: 'pointer',
      kind: dragRef.current ? 'drag' : 'move',
      x: cell.x,
      y: cell.y,
    })
  }

  const pointerDown = (event: ReactPointerEvent): void => {
    if (!canInteract || event.button !== 0) return
    const cell = eventCell(event)
    if (!cell) return
    if (!interactionActive) enterInteraction()
    event.currentTarget.setPointerCapture(event.pointerId)
    dragRef.current = true
    dispatch({ type: 'pointer', kind: 'down', x: cell.x, y: cell.y })
  }

  const pointerUp = (event: ReactPointerEvent): void => {
    if (!canInteract || !dragRef.current) return
    const cell = eventCell(event)
    dragRef.current = false
    if (!cell) {
      clearPointerState()
      return
    }
    dispatch({ type: 'pointer', kind: 'up', x: cell.x, y: cell.y })
  }

  const wheel = (event: ReactWheelEvent): void => {
    if (!canInteract || !interactionActive) return
    const cell = eventCell(event)
    if (!cell || (event.deltaX === 0 && event.deltaY === 0)) return
    event.preventDefault()
    event.stopPropagation()
    dispatch({
      type: 'wheel',
      deltaX: Math.sign(event.deltaX),
      deltaY: Math.sign(event.deltaY),
      x: cell.x,
      y: cell.y,
    })
  }

  const hoverProbe =
    hover && frame
      ? formatCellProbe(
          hover.x,
          hover.y,
          frame.cells[hover.y * frame.cols + hover.x] ?? null,
        )
      : ''
  const accessibleName = descriptor
    ? `${descriptor.component}: ${descriptor.title}`
    : `Terminal preview: ${activeStory}`
  const announcement = error
    ? `Preview unavailable: ${error}`
    : update?.outcome
      ? `Preview state: ${update.outcome}`
      : loading
        ? 'Loading terminal preview.'
        : source === 'static-poster'
          ? 'Static terminal preview loaded.'
          : 'Terminal preview ready.'

  const chrome: CSSProperties = {
    borderRadius: 12,
    border: focused ? '1px solid #39ff14' : '1px solid #1e261e',
    background: 'linear-gradient(180deg, #222622 0%, #121512 14%, #070907 100%)',
    boxShadow: focused
      ? '0 0 0 1px rgba(57,255,20,0.28), 0 0 24px rgba(57,255,20,0.08), 0 16px 48px rgba(0,0,0,0.6)'
      : '0 1px 0 rgba(255,255,255,0.04) inset, 0 16px 48px rgba(0,0,0,0.5)',
    overflow: 'hidden',
    width: '100%',
    maxWidth: '100%',
    outline: 'none',
    filter: focused ? 'none' : 'brightness(0.94)',
    transition: reducedMotion
      ? 'none'
      : 'border-color 120ms ease, box-shadow 120ms ease, filter 120ms ease',
    ...(fullscreen
      ? {
          height: '100%',
          maxWidth: 'none',
          display: 'flex',
          flexDirection: 'column',
        }
      : {}),
  }

  return (
    <figure
      ref={figureRef}
      className="not-prose my-6"
      data-termrock-preview={activeStory}
      data-preview-live={source}
      data-preview-interactive={canInteract ? 'true' : 'false'}
      data-preview-engaged={interactionActive ? 'true' : 'false'}
      data-preview-focused={focused ? 'true' : 'false'}
      data-preview-cols={frame?.cols ?? ''}
      data-preview-rows={frame?.rows ?? ''}
      data-preview-outcome={update?.outcome ?? ''}
      data-preview-semantic-revision={update?.semanticRevision ?? 0}
      data-preview-hover={hover ? String(hover.x) + ',' + String(hover.y) : ''}
    >
      <PreviewToolbar
        view={view}
        onViewChange={(nextView) => {
          setView(nextView)
          if (nextView === 'code') exitInteraction(false)
        }}
        variants={variants}
        activeStory={activeStory}
        onStoryChange={(nextStory) => {
          setActiveStory(nextStory)
          setView('preview')
          setInteractionActive(false)
          setFullscreen(false)
          interactionFocusRequestedRef.current = false
          interactionBeforeFullscreenRef.current = false
          fullscreenRestoreFocusRef.current = null
          keyAliasesRef.current.clear()
        }}
        canInteract={canInteract}
        canRequestInteraction={canRequestInteraction}
        runtimeRequested={runtimeRequested}
        runtimeLoading={runtimeRequested && loading}
        interactionActive={interactionActive}
        interactionButtonRef={interactionButtonRef}
        onRuntimeRequest={() => setRuntimeRequested(true)}
        onInteractionToggle={() => {
          if (interactionActive) exitInteraction(false)
          else enterInteraction()
        }}
      />
      <span
        data-termrock-announcer="1"
        className="sr-only"
        role="status"
        aria-live="polite"
        aria-atomic="true"
      >
        {announcement}
      </span>
      <p id={instructionsId} className="sr-only">
        {interactionActive
          ? 'Terminal interaction is active. Tab stops interacting and moves to the next page control. Escape stops interacting and returns to the interaction button. Shift plus Enter sends Tab to the terminal. Shift plus Escape sends Escape to the terminal.'
          : canInteract
            ? 'Choose Interact with preview to send keyboard input to the terminal. Page Tab navigation remains available.'
            : canRequestInteraction
              ? 'Static terminal poster. Choose Run live to start the Rust demo, or Interact with preview to start it and send keyboard input.'
              : source === 'rust-wasm'
                ? 'Live Rust terminal preview. Interactive input is unavailable.'
                : runtimeRequested
                  ? 'Starting the live Rust terminal preview.'
                  : 'Static terminal poster. Choose Run live to start the Rust demo.'}
      </p>
      <PreviewSemanticState id={semanticStateId} frame={semanticFrame} />
      <PreviewDialog
        id={dialogId}
        open={fullscreen}
        labelId={titleId}
        initialFocusRef={hostRef}
        restoreFocusRef={fullscreenRestoreFocusRef}
        onClose={closeFullscreen}
      >
        <div
          ref={hostRef}
          role={canInteract ? 'application' : 'img'}
          tabIndex={canInteract && interactionActive ? 0 : -1}
          aria-label={accessibleName}
          aria-describedby={`${instructionsId} ${semanticStateId}`}
          aria-keyshortcuts={canInteract ? 'Escape Tab Shift+Escape Shift+Enter' : undefined}
          onKeyDown={(event) => keyEvent(event, event.repeat ? 'repeat' : 'press')}
          onKeyUp={(event) => keyEvent(event, 'release')}
          onPointerMove={pointerMove}
          onPointerLeave={clearPointerState}
          onPointerDown={pointerDown}
          onPointerUp={pointerUp}
          onPointerCancel={clearPointerState}
          onWheel={wheel}
          onFocus={() => {
            setFocused(true)
            if (interactionActive) dispatch({ type: 'focus', focused: true })
            if (
              interactionActive &&
              capturesTextInput &&
              document.activeElement === hostRef.current
            ) textSinkRef.current?.focus()
          }}
          onBlur={(event) => {
            if (event.currentTarget.contains(event.relatedTarget)) return
            setFocused(false)
            setInteractionActive(false)
            interactionFocusRequestedRef.current = false
            keyAliasesRef.current.clear()
            clearPointerState()
            dispatch({ type: 'focus', focused: false })
          }}
          style={{ ...chrome, display: view === 'preview' ? chrome.display : 'none' }}
        >
          {capturesTextInput ? (
            <textarea
              ref={textSinkRef}
              aria-label="Terminal preview text input"
              tabIndex={-1}
              defaultValue=""
              autoCapitalize="off"
              autoComplete="off"
              spellCheck={false}
              style={{
                position: 'fixed',
                left: -10_000,
                top: 0,
                width: 1,
                height: 1,
                opacity: 0,
                pointerEvents: 'none',
              }}
            />
          ) : null}
          <div
            id={titleId}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 8,
              padding: '9px 12px',
              borderBottom: '1px solid #1a201a',
              background: 'linear-gradient(180deg, #181b18 0%, #0e100e 100%)',
              fontFamily: PREVIEW_MONO_STACK,
              fontSize: 12,
              color: '#8a9a8a',
              userSelect: 'none',
            }}
          >
            <span style={{ display: 'flex', gap: 6 }} aria-hidden="true">
              {['#ff5f57', '#febc2e', '#28c840'].map((color) => (
                <span
                  key={color}
                  style={{ width: 10, height: 10, borderRadius: 999, background: color }}
                />
              ))}
            </span>
            <span style={{ color: '#c8d6c8', flexShrink: 0 }}>Ghostty</span>
            <span className="hidden sm:inline">· TermRock —</span>
            <span
              style={{
                color: '#39ff14',
                minWidth: 0,
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              }}
            >
              {activeStory}
            </span>
            <span
              className="hidden sm:inline"
              style={{ marginLeft: 'auto', fontSize: 11, flexShrink: 0 }}
            >
              {frame
                ? String(frame.cols) + '×' + String(frame.rows) + ' · RGB24'
                : 'loading poster…'}
            </span>
          </div>
          <div
            ref={stageRef}
            data-termrock-stage="1"
            style={{
              padding: 12,
              background:
                'radial-gradient(120% 80% at 50% 0%, #121412 0%, #0a0a0a 55%, #050505 100%)',
              boxShadow: 'inset 0 0 0 1px #151915, inset 0 12px 28px rgba(0,0,0,0.45)',
              overflow: 'auto',
              maxHeight: fullscreen ? 'calc(100vh - 116px)' : maxHeight,
              flex: fullscreen ? 1 : undefined,
              minHeight: 120,
              width: '100%',
              display: 'flex',
              justifyContent: 'center',
              alignItems: 'flex-start',
              boxSizing: 'border-box',
            }}
          >
            {error && !frame ? (
              <pre style={{ color: '#ff5e7a', fontSize: 12, whiteSpace: 'pre-wrap' }}>
                {error}
              </pre>
            ) : frame ? (
              <canvas
                ref={canvasRef}
                aria-hidden="true"
                data-termrock-canvas="1"
                data-canvas-cols={frame.cols}
                data-canvas-rows={frame.rows}
                style={{ display: 'block', imageRendering: 'pixelated', flexShrink: 0 }}
              />
            ) : (
              <span style={{ color: '#6a7a6a', fontFamily: PREVIEW_MONO_STACK }}>
                Loading terminal poster…
              </span>
            )}
          </div>
          <PreviewStatus
            source={source}
            canInteract={canInteract}
            animated={animated}
            update={update}
            descriptor={descriptor}
            hoverProbe={hoverProbe}
            fullscreen={fullscreen}
            dialogId={dialogId}
            onFullscreenToggle={toggleFullscreen}
            onReset={reset}
          />
        </div>
      </PreviewDialog>
      {view === 'code' ? (
        <pre
          data-termrock-code="1"
          style={{
            margin: 0,
            maxHeight,
            overflow: 'auto',
            border: '1px solid #243024',
            borderRadius: 10,
            background: '#080b08',
            color: '#c8d6c8',
            padding: 16,
            fontFamily: PREVIEW_MONO_STACK,
            fontSize: 12,
            lineHeight: 1.6,
            whiteSpace: 'pre',
          }}
        >
          <code>{sourceCode}</code>
        </pre>
      ) : null}
      {caption ? (
        <figcaption
          style={{
            marginTop: 8,
            fontSize: 13,
            color: '#8a9a8a',
            fontFamily: PREVIEW_MONO_STACK,
          }}
        >
          {caption}
        </figcaption>
      ) : null}
    </figure>
  )
}
