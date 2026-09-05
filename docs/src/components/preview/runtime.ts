import type { DemoDescriptor, DemoUpdate, TerminalFrame } from '@/components/preview/model'

export type PreviewRuntime = typeof import(
  '@/generated/termrock-preview/termrock_catalog_web.js'
)

let runtimePromise: Promise<PreviewRuntime> | undefined
let demoCodePromise: Promise<Record<string, string>> | undefined

/** WASM import stays cold until an explicit live or interaction action requests it. */
export function loadPreviewRuntime(): Promise<PreviewRuntime> {
  runtimePromise ??= import(
    '@/generated/termrock-preview/termrock_catalog_web.js'
  ).then(async (runtime) => {
    await runtime.default()
    return runtime
  })
  return runtimePromise
}

export function loadDemoCode(): Promise<Record<string, string>> {
  demoCodePromise ??= fetch('/demo-code.json').then((response) => {
    if (!response.ok) throw new Error(`demo code ${response.status}`)
    return response.json() as Promise<Record<string, string>>
  })
  return demoCodePromise
}

export function readCatalog(runtime: PreviewRuntime): DemoDescriptor[] {
  return JSON.parse(runtime.catalog_json()) as DemoDescriptor[]
}

export function readFrame(runtime: PreviewRuntime, handle: number): TerminalFrame {
  return JSON.parse(runtime.demo_frame(handle)) as TerminalFrame
}

export function dispatchRuntimeEvent(
  runtime: PreviewRuntime,
  handle: number,
  event: unknown,
): DemoUpdate {
  return JSON.parse(runtime.dispatch_demo(handle, JSON.stringify(event))) as DemoUpdate
}
