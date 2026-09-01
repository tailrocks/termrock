'use client'

import { useEffect, useRef, useState, type ReactElement } from 'react'
import { loadDemoCode } from '@/components/preview/runtime'
import { TERMROCK_INSTALL_COMMAND } from '@/lib/install'

type CopyState = 'idle' | 'copied' | 'failed'

type SourceState =
  | Readonly<{ status: 'idle' | 'loading' }>
  | Readonly<{ status: 'ready'; source: string }>
  | Readonly<{ status: 'failed'; message: string }>

function message(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason)
}

function CopyButton({ value, label }: Readonly<{ value: string; label: string }>): ReactElement {
  const [state, setState] = useState<CopyState>('idle')

  useEffect(() => {
    if (state === 'idle') return
    const timer = window.setTimeout(() => setState('idle'), 1_800)
    return () => window.clearTimeout(timer)
  }, [state])

  const buttonLabel =
    state === 'copied' ? 'Copied' : state === 'failed' ? 'Copy failed' : label

  return (
    <button
      type="button"
      className="doc-detail-copy"
      onClick={() => {
        if (!navigator.clipboard) {
          setState('failed')
          return
        }
        void navigator.clipboard.writeText(value).then(
          () => setState('copied'),
          () => setState('failed'),
        )
      }}
    >
      {buttonLabel}
    </button>
  )
}

export function ImplementationPanel({ story }: Readonly<{ story: string }>): ReactElement {
  const mountedRef = useRef(true)
  const [sourceState, setSourceState] = useState<SourceState>({ status: 'idle' })

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  const requestSource = (): void => {
    if (sourceState.status !== 'idle') return
    setSourceState({ status: 'loading' })
    void loadDemoCode().then(
      (sources) => {
        if (!mountedRef.current) return
        const source = sources[story]
        setSourceState(
          source
            ? { status: 'ready', source }
            : { status: 'failed', message: `No exact source registered for ${story}.` },
        )
      },
      (reason: unknown) => {
        if (mountedRef.current) {
          setSourceState({ status: 'failed', message: message(reason) })
        }
      },
    )
  }

  return (
    <div className="doc-detail-install-grid">
      <div>
        <p className="doc-detail-kicker">Install</p>
        <div className="doc-detail-command">
          <code>{TERMROCK_INSTALL_COMMAND}</code>
          <CopyButton value={TERMROCK_INSTALL_COMMAND} label="Copy command" />
        </div>
        <p className="doc-detail-note">
          Add TermRock once. Keep domain effects in the host application.
        </p>
      </div>

      <details
        className="doc-detail-details doc-detail-code-details"
        onToggle={(event) => {
          if (event.currentTarget.open) requestSource()
        }}
      >
        <summary>Minimal implementation</summary>
        <p className="doc-detail-note">
          Exact Rust setup used by <code>{story}</code>.
        </p>
        {sourceState.status === 'ready' ? (
          <div className="doc-detail-code">
            <CopyButton value={sourceState.source} label="Copy Rust" />
            <pre>
              <code>{sourceState.source}</code>
            </pre>
          </div>
        ) : sourceState.status === 'failed' ? (
          <p role="alert" className="doc-detail-error">
            {sourceState.message}
          </p>
        ) : (
          <p role="status" className="doc-detail-note">
            {sourceState.status === 'loading' ? 'Loading exact source…' : 'Open to load code.'}
          </p>
        )}
      </details>
    </div>
  )
}
