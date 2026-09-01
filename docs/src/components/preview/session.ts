import { useCallback, useEffect, useRef, useState } from 'react'
import type { DemoEvent } from '@/components/preview/input'
import type {
  DemoDescriptor,
  DemoUpdate,
  PreviewSource,
  TerminalFrame,
} from '@/components/preview/model'
import { loadPosterFrame, posterDescriptor } from '@/components/preview/poster'
import {
  dispatchRuntimeEvent,
  loadPreviewRuntime,
  readCatalog,
  readFrame,
  type PreviewRuntime,
} from '@/components/preview/runtime'

type PosterState =
  | { readonly status: 'loading'; readonly story: string }
  | { readonly status: 'ready'; readonly story: string; readonly frame: TerminalFrame }
  | { readonly status: 'failure'; readonly story: string; readonly message: string }

type SettledPosterState = Exclude<PosterState, { readonly status: 'loading' }>

type PreviewSessionState =
  | { readonly status: 'poster-loading'; readonly story: string }
  | {
      readonly status: 'poster-ready'
      readonly story: string
      readonly frame: TerminalFrame
      readonly descriptor: DemoDescriptor
    }
  | {
      readonly status: 'poster-failure'
      readonly story: string
      readonly message: string
    }
  | {
      readonly status: 'runtime-loading'
      readonly story: string
      readonly poster: SettledPosterState
    }
  | {
      readonly status: 'runtime-ready'
      readonly story: string
      readonly frame: TerminalFrame
      readonly semanticFrame: TerminalFrame
      readonly descriptor: DemoDescriptor
      readonly update: DemoUpdate
      readonly catalog: readonly DemoDescriptor[]
      readonly error: string | null
    }
  | {
      readonly status: 'poster-only'
      readonly story: string
      readonly frame: TerminalFrame
      readonly descriptor: DemoDescriptor
    }
  | { readonly status: 'failure'; readonly story: string; readonly message: string }

type RuntimeOwner = {
  readonly story: string
  readonly runtime: PreviewRuntime
  readonly handle: number
  mountedAt: number
}

export type PreviewPaintPolicy = 'changed' | 'always'

export type PreviewSession = {
  readonly frame: TerminalFrame | null
  readonly semanticFrame: TerminalFrame | null
  readonly descriptor: DemoDescriptor | null
  readonly update: DemoUpdate | null
  readonly catalog: readonly DemoDescriptor[]
  readonly source: PreviewSource
  readonly loading: boolean
  readonly error: string | null
  readonly dispatch: (
    event: DemoEvent,
    paintPolicy?: PreviewPaintPolicy,
  ) => DemoUpdate | null
  readonly reset: () => void
  readonly getMountedAt: () => number
}

function errorMessage(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason)
}

function isAbort(reason: unknown): boolean {
  return reason instanceof DOMException && reason.name === 'AbortError'
}

function loadingState(story: string): PreviewSessionState {
  return { status: 'poster-loading', story }
}

function projectSession(
  state: PreviewSessionState,
  runtimeRequested: boolean,
): Omit<PreviewSession, 'dispatch' | 'reset' | 'getMountedAt'> {
  switch (state.status) {
    case 'poster-loading':
      return {
        frame: null,
        semanticFrame: null,
        descriptor: null,
        update: null,
        catalog: [],
        source: 'poster-loading',
        loading: true,
        error: null,
      }
    case 'poster-ready':
      return {
        frame: state.frame,
        semanticFrame: state.frame,
        descriptor: state.descriptor,
        update: null,
        catalog: [],
        source: 'static-poster',
        loading: runtimeRequested,
        error: null,
      }
    case 'poster-failure':
      return {
        frame: null,
        semanticFrame: null,
        descriptor: null,
        update: null,
        catalog: [],
        source: 'failed',
        loading: runtimeRequested,
        error: state.message,
      }
    case 'runtime-loading':
      if (state.poster.status === 'ready') {
        const descriptor = posterDescriptor(state.poster.frame)
        return {
          frame: state.poster.frame,
          semanticFrame: state.poster.frame,
          descriptor,
          update: null,
          catalog: [],
          source: 'static-poster',
          loading: true,
          error: null,
        }
      }
      return {
        frame: null,
        semanticFrame: null,
        descriptor: null,
        update: null,
        catalog: [],
        source: 'failed',
        loading: true,
        error: state.poster.message,
      }
    case 'runtime-ready':
      return {
        frame: state.frame,
        semanticFrame: state.semanticFrame,
        descriptor: state.descriptor,
        update: state.update,
        catalog: state.catalog,
        source: 'rust-wasm',
        loading: false,
        error: state.error,
      }
    case 'poster-only':
      return {
        frame: state.frame,
        semanticFrame: state.frame,
        descriptor: state.descriptor,
        update: null,
        catalog: [],
        source: 'static-poster',
        loading: false,
        error: null,
      }
    case 'failure':
      return {
        frame: null,
        semanticFrame: null,
        descriptor: null,
        update: null,
        catalog: [],
        source: 'failed',
        loading: false,
        error: state.message,
      }
  }
}

export function usePreviewSession(
  story: string,
  runtimeRequested: boolean,
): PreviewSession {
  const runtimeOwnerRef = useRef<RuntimeOwner | null>(null)
  const [posterState, setPosterState] = useState<PosterState>(() => ({
    status: 'loading',
    story,
  }))
  const [sessionState, setSessionState] = useState<PreviewSessionState>(() =>
    loadingState(story),
  )

  useEffect(() => {
    const controller = new AbortController()
    const nextLoading = { status: 'loading', story } as const
    setPosterState(nextLoading)
    setSessionState(loadingState(story))
    void loadPosterFrame(story, controller.signal)
      .then((frame) => {
        const nextPoster = { status: 'ready', story, frame } as const
        setPosterState((current) => (current.story === story ? nextPoster : current))
        setSessionState((current) =>
          current.story === story
            ? {
                status: 'poster-ready',
                story,
                frame,
                descriptor: posterDescriptor(frame),
              }
            : current,
        )
      })
      .catch((reason: unknown) => {
        if (isAbort(reason)) return
        const message = errorMessage(reason)
        const nextPoster = { status: 'failure', story, message } as const
        setPosterState((current) => (current.story === story ? nextPoster : current))
        setSessionState((current) =>
          current.story === story
            ? { status: 'poster-failure', story, message }
            : current,
        )
      })
    return () => controller.abort()
  }, [story])

  useEffect(() => {
    if (
      !runtimeRequested ||
      posterState.story !== story ||
      posterState.status === 'loading'
    ) {
      return
    }
    let cancelled = false
    let ownedRuntime: RuntimeOwner | null = null
    setSessionState((current) =>
      current.story === story
        ? { status: 'runtime-loading', story, poster: posterState }
        : current,
    )
    void loadPreviewRuntime()
      .then((runtime) => {
        if (cancelled) return
        const catalog = readCatalog(runtime)
        const descriptor = catalog.find((entry) => entry.id === story)
        if (!descriptor) throw new Error('unknown TermRock demo: ' + story)
        const handle = runtime.mount_demo(
          story,
          Math.max(8, descriptor.cols),
          Math.max(4, descriptor.rows),
        )
        if (cancelled) {
          runtime.unmount_demo(handle)
          return
        }
        ownedRuntime = {
          story,
          runtime,
          handle,
          mountedAt: performance.now(),
        }
        runtimeOwnerRef.current = ownedRuntime
        const initialFrame = readFrame(runtime, handle)
        const initialUpdate = dispatchRuntimeEvent(runtime, handle, {
          type: 'resize',
          cols: initialFrame.story_cols,
          rows: initialFrame.story_rows,
        } satisfies DemoEvent)
        const frame = initialUpdate.changed ? readFrame(runtime, handle) : initialFrame
        setSessionState((current) =>
          current.story === story
            ? {
                status: 'runtime-ready',
                story,
                frame,
                semanticFrame: frame,
                descriptor,
                update: initialUpdate,
                catalog,
                error: null,
              }
            : current,
        )
      })
      .catch((reason: unknown) => {
        if (cancelled) return
        if (posterState.status === 'ready') {
          setSessionState((current) =>
            current.story === story
              ? {
                  status: 'poster-only',
                  story,
                  frame: posterState.frame,
                  descriptor: posterDescriptor(posterState.frame),
                }
              : current,
          )
          return
        }
        setSessionState((current) =>
          current.story === story
            ? {
                status: 'failure',
                story,
                message: `${errorMessage(reason)}; ${posterState.message}`,
              }
            : current,
        )
      })
    return () => {
      cancelled = true
      if (ownedRuntime) {
        try {
          ownedRuntime.runtime.unmount_demo(ownedRuntime.handle)
        } catch {
          // A concurrent remount can already have released this owned handle.
        }
      }
      if (runtimeOwnerRef.current === ownedRuntime) runtimeOwnerRef.current = null
    }
  }, [posterState, runtimeRequested, story])

  const dispatch = useCallback(
    (
      event: DemoEvent,
      paintPolicy: PreviewPaintPolicy = 'changed',
    ): DemoUpdate | null => {
      const owner = runtimeOwnerRef.current
      if (!owner || owner.story !== story) return null
      try {
        const update = dispatchRuntimeEvent(owner.runtime, owner.handle, event)
        const shouldPaint = update.changed || paintPolicy === 'always'
        const frame = shouldPaint ? readFrame(owner.runtime, owner.handle) : null
        setSessionState((current) => {
          if (current.story !== story || current.status !== 'runtime-ready') {
            return current
          }
          const semanticChanged =
            update.semanticRevision !== current.update.semanticRevision
          return {
            ...current,
            frame: frame ?? current.frame,
            semanticFrame: frame && semanticChanged ? frame : current.semanticFrame,
            update,
            error: null,
          }
        })
        return update
      } catch (reason: unknown) {
        const message = errorMessage(reason)
        setSessionState((current) =>
          current.story === story && current.status === 'runtime-ready'
            ? { ...current, error: message }
            : current,
        )
        return null
      }
    },
    [story],
  )

  const reset = useCallback((): void => {
    const owner = runtimeOwnerRef.current
    if (!owner || owner.story !== story) return
    try {
      const resetFrame = JSON.parse(owner.runtime.reset_demo(owner.handle)) as TerminalFrame
      owner.mountedAt = performance.now()
      const update = dispatchRuntimeEvent(owner.runtime, owner.handle, {
        type: 'resize',
        cols: resetFrame.story_cols,
        rows: resetFrame.story_rows,
      } satisfies DemoEvent)
      const frame = update.changed
        ? readFrame(owner.runtime, owner.handle)
        : resetFrame
      setSessionState((current) =>
        current.story === story && current.status === 'runtime-ready'
          ? {
              ...current,
              frame,
              semanticFrame: frame,
              update,
              error: null,
            }
          : current,
      )
    } catch (reason: unknown) {
      const message = errorMessage(reason)
      setSessionState((current) =>
        current.story === story && current.status === 'runtime-ready'
          ? { ...current, error: message }
          : current,
      )
    }
  }, [story])

  const getMountedAt = useCallback((): number => {
    const owner = runtimeOwnerRef.current
    return owner?.story === story ? owner.mountedAt : 0
  }, [story])

  const currentState =
    sessionState.story === story ? sessionState : loadingState(story)
  return {
    ...projectSession(currentState, runtimeRequested),
    dispatch,
    reset,
    getMountedAt,
  }
}
