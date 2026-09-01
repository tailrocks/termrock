'use client'

import { useState, type ReactElement } from 'react'

type CopyState = 'idle' | 'copied' | 'failed'

export type CopyInstallCommandProps = Readonly<{
  command: string
}>

export function CopyInstallCommand({ command }: CopyInstallCommandProps): ReactElement {
  const [state, setState] = useState<CopyState>('idle')

  const copy = (): void => {
    if (!navigator.clipboard) {
      setState('failed')
      return
    }

    void navigator.clipboard.writeText(command).then(
      () => setState('copied'),
      () => setState('failed'),
    )
  }

  const buttonLabel = state === 'copied' ? 'Copied' : 'Copy command'
  const status =
    state === 'copied'
      ? 'Install command copied to clipboard.'
      : state === 'failed'
        ? 'Clipboard unavailable. Select the command to copy it.'
        : 'Pin the reviewed revision before sharing your application.'

  return (
    <div className="home-copy">
      <div className="home-copy__command">
        <span aria-hidden="true">$</span>
        <code>{command}</code>
        <button type="button" onClick={copy}>
          {buttonLabel}
        </button>
      </div>
      <p className="home-copy__status" role="status" aria-live="polite">
        {status}
      </p>
    </div>
  )
}
