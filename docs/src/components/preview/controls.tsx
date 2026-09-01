import type { RefObject } from 'react'
import { PREVIEW_MONO_STACK } from '@/components/preview-metrics'
import type {
  DemoDescriptor,
  DemoUpdate,
  PreviewSource,
  PreviewView,
} from '@/components/preview/model'

export type PreviewToolbarProps = {
  readonly view: PreviewView
  readonly onViewChange: (view: PreviewView) => void
  readonly variants: DemoDescriptor[]
  readonly activeStory: string
  readonly onStoryChange: (story: string) => void
  readonly canInteract: boolean
  readonly canRequestInteraction: boolean
  readonly runtimeRequested: boolean
  readonly runtimeLoading: boolean
  readonly interactionActive: boolean
  readonly interactionButtonRef: RefObject<HTMLButtonElement | null>
  readonly onRuntimeRequest: () => void
  readonly onInteractionToggle: () => void
}

export function PreviewToolbar({
  view,
  onViewChange,
  variants,
  activeStory,
  onStoryChange,
  canInteract,
  canRequestInteraction,
  runtimeRequested,
  runtimeLoading,
  interactionActive,
  interactionButtonRef,
  onRuntimeRequest,
  onInteractionToggle,
}: PreviewToolbarProps) {
  return (
    <div
      data-termrock-preview-controls="1"
      style={{
        display: 'flex',
        alignItems: 'center',
        flexWrap: 'wrap',
        gap: 8,
        marginBottom: 8,
        fontFamily: PREVIEW_MONO_STACK,
        fontSize: 12,
      }}
    >
      {(['preview', 'code'] as const).map((item) => (
        <button
          key={item}
          type="button"
          aria-pressed={view === item}
          onClick={() => onViewChange(item)}
          style={{
            border: `1px solid ${view === item ? '#39ff14' : '#334033'}`,
            borderRadius: 6,
            background: view === item ? '#132013' : '#0c100c',
            color: view === item ? '#d8ffd8' : '#91a091',
            padding: '4px 10px',
            font: 'inherit',
            cursor: 'pointer',
            textTransform: 'capitalize',
          }}
        >
          {item}
        </button>
      ))}
      {variants.length > 1 ? (
        <label style={{ marginLeft: 'auto', color: '#91a091' }}>
          Variant{' '}
          <select
            aria-label="Preview variant"
            value={activeStory}
            onChange={(event) => onStoryChange(event.target.value)}
            style={{
              marginLeft: 6,
              border: '1px solid #334033',
              borderRadius: 5,
              background: '#0c100c',
              color: '#c8d6c8',
              padding: '3px 7px',
              font: 'inherit',
            }}
          >
            {variants.map((variant) => (
              <option key={variant.id} value={variant.id}>
                {variant.title}
              </option>
            ))}
          </select>
        </label>
      ) : null}
      {!runtimeRequested ? (
        <button
          type="button"
          data-termrock-run-live="1"
          onClick={onRuntimeRequest}
          className="rounded-md border border-fd-border bg-fd-card px-2.5 py-1 text-fd-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-fd-primary"
        >
          Run live
        </button>
      ) : runtimeLoading ? (
        <span data-termrock-runtime-loading="1" role="status">
          Starting live preview…
        </span>
      ) : null}
      {canInteract || (canRequestInteraction && !runtimeRequested) ? (
        <button
          ref={interactionButtonRef}
          type="button"
          data-termrock-interaction="1"
          aria-pressed={canInteract ? interactionActive : false}
          onClick={onInteractionToggle}
          className="rounded-md border border-fd-border bg-fd-card px-2.5 py-1 text-fd-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-fd-primary"
        >
          {interactionActive ? 'Stop interacting' : 'Interact with preview'}
        </button>
      ) : null}
    </div>
  )
}

export type PreviewStatusProps = {
  readonly source: PreviewSource
  readonly canInteract: boolean
  readonly animated: boolean
  readonly update: DemoUpdate | null
  readonly descriptor: DemoDescriptor | null
  readonly hoverProbe: string
  readonly fullscreen: boolean
  readonly dialogId: string
  readonly onFullscreenToggle: () => void
  readonly onReset: () => void
}

function sourceLabel(
  source: PreviewSource,
  canInteract: boolean,
  animated: boolean,
): string {
  if (source === 'poster-loading') return '○ loading poster'
  if (source === 'static-poster') return '○ static preview'
  if (source === 'failed') return '○ preview unavailable'
  if (animated) return '◐ timed Rust demo'
  if (canInteract) return '● live Rust demo'
  return '○ live paint'
}

export function PreviewStatus({
  source,
  canInteract,
  animated,
  update,
  descriptor,
  hoverProbe,
  fullscreen,
  dialogId,
  onFullscreenToggle,
  onReset,
}: PreviewStatusProps) {
  return (
    <div
      data-termrock-status="1"
      style={{
        display: 'flex',
        alignItems: 'center',
        flexWrap: 'wrap',
        gap: 10,
        padding: '6px 12px',
        borderTop: '1px solid #1a201a',
        fontFamily: PREVIEW_MONO_STACK,
        fontSize: 11,
        color: '#718071',
        background: '#0c0e0c',
        userSelect: 'none',
      }}
    >
      <span style={{ color: canInteract || animated ? '#39ff14' : '#6a7a6a' }}>
        {sourceLabel(source, canInteract, animated)}
      </span>
      {update?.outcome ? (
        <span data-termrock-outcome="1" style={{ color: '#b4e8b4' }}>
          {update.outcome}
        </span>
      ) : null}
      {hoverProbe ? <span data-termrock-cell-probe="1">{hoverProbe}</span> : null}
      <span data-termrock-hints="1" style={{ marginLeft: 'auto' }}>
        {(update?.hints ?? descriptor?.hints ?? []).join(' · ') ||
          'No input — rendered state only'}
      </span>
      <button
        type="button"
        aria-haspopup="dialog"
        aria-expanded={fullscreen}
        aria-controls={dialogId}
        onClick={(event) => {
          event.stopPropagation()
          onFullscreenToggle()
        }}
        style={{
          border: '1px solid #334033',
          borderRadius: 5,
          background: '#121712',
          color: '#a8b8a8',
          padding: '2px 7px',
          font: 'inherit',
          cursor: 'pointer',
        }}
      >
        {fullscreen ? 'Exit full preview' : 'Full preview'}
      </button>
      {source === 'rust-wasm' ? (
        <button
          type="button"
          onClick={(event) => {
            event.stopPropagation()
            onReset()
          }}
          style={{
            border: '1px solid #334033',
            borderRadius: 5,
            background: '#121712',
            color: '#a8b8a8',
            padding: '2px 7px',
            font: 'inherit',
            cursor: 'pointer',
          }}
        >
          Reset
        </button>
      ) : null}
    </div>
  )
}
