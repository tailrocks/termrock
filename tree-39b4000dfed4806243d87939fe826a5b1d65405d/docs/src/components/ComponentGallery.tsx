'use client'

import { useEffect, useMemo, useRef, useState } from 'react'
import routes from '../../api/component-routes.json'
import { paintCanvas, type TerminalFrame } from '@/components/TerminalPreview'

type Route = (typeof routes)[number]

/**
 * The component index, grouped and shown rather than listed.
 *
 * A flat 165-row table asks the reader to already know the name of the thing
 * they are looking for. The families below are how the catalog is actually
 * organised, and each card carries the component's own poster, so browsing is
 * possible without knowing the vocabulary first (plans/018 Step 3).
 */
const FAMILIES: Array<{ title: string; note: string; members: string[] }> = [
  {
    title: 'Collections',
    note: 'Rows, trees and tables — anything with a cursor and a selection.',
    members: [
      'List',
      'NavigationList',
      'Table',
      'DataTable',
      'TreeTable',
      'Tree',
      'TreeNavigation',
      'VirtualList',
      'VirtualGrid',
      'DetailTable',
      'KeyValueTable',
      'KeyValueList',
      'Picker',
      'MultiSelect',
      'Pagination',
    ],
  },
  {
    title: 'Inputs and forms',
    note: 'Everything the operator types into, and the chrome that validates it.',
    members: [
      'TextInput',
      'TextArea',
      'PasswordInput',
      'NumberInput',
      'SearchInput',
      'PathInput',
      'TokenField',
      'Combobox',
      'Select',
      'Checkbox',
      'RadioGroup',
      'Switch',
      'Toggle',
      'ToggleGroup',
      'SegmentedControl',
      'Slider',
      'RangeSlider',
      'Stepper',
      'DateTimePicker',
      'Form',
      'Fieldset',
      'Field',
      'FieldRow',
      'FieldCaption',
      'FormWizard',
      'KeybindingRecorder',
    ],
  },
  {
    title: 'Overlays',
    note: 'Surfaces that take focus and give it back.',
    members: [
      'Dialog',
      'AlertDialog',
      'MessageDialog',
      'ChoiceDialog',
      'Drawer',
      'Sheet',
      'Popover',
      'Tooltip',
      'ContextMenu',
      'DropdownMenu',
      'Menu',
      'MenuBar',
      'CommandPalette',
      'QuickOpen',
      'CompletionMenu',
      'HistoryPicker',
      'FilePicker',
      'FullscreenViewer',
      'JumpOverlay',
      'JumpMode',
      'Backdrop',
      'LoadingOverlay / BusyBoundary',
    ],
  },
  {
    title: 'Chrome and layout',
    note: 'The frame around the work.',
    members: [
      'Panel',
      'Card',
      'Surface',
      'Section',
      'Sidebar',
      'Tabs',
      'Toolbar',
      'StatusBar',
      'Breadcrumbs',
      'Separator',
      'SeparatorLine',
      'Grid',
      'Stack / Inline',
      'Center',
      'SplitPane',
      'ResizablePanelGroup',
      'Collapsible',
      'Accordion',
      'Viewport',
      'ModeRibbon',
      'AccentRail',
      'PreviewCard',
      'ImageSurface',
    ],
  },
  {
    title: 'Feedback and status',
    note: 'What the application says back.',
    members: [
      'Toast',
      'Alert',
      'Banner',
      'Callout',
      'EmptyState',
      'ErrorState',
      'ErrorView / ErrorState',
      'LoadingView',
      'Skeleton',
      'Spinner',
      'Progress',
      'ProgressBar',
      'ProgressSteps',
      'StatusIndicator',
      'ActivityIndicator',
      'NotificationCenter',
      'Badge',
      'Tag',
      'Chip',
      'TokenMeter',
      'Offline / ReconnectingState',
      'OfflineBanner',
      'QuestionFlow',
    ],
  },
  {
    title: 'Data and visualization',
    note: 'Numbers, series and payloads.',
    members: [
      'Chart',
      'Sparkline',
      'Gauge',
      'Histogram',
      'BarSeries',
      'MetricRadar',
      'SegmentedMeter',
      'Timeline',
      'CheckpointTimeline',
      'LogStream',
      'LogPane',
      'EventStream',
      'ObjectInspector',
      'HexViewer',
      'DiffView',
      'DiffReview',
      'CodeBlock',
      'MarkdownView',
      'AnsiText',
      'TerminalOutput',
      'DiagnosticView',
    ],
  },
  {
    title: 'Agent surfaces',
    note: 'The vocabulary a coding agent needs.',
    members: [
      'Transcript',
      'PromptComposer',
      'PromptQueue',
      'TaskRail',
      'ThinkingBlock',
      'ToolCard',
      'PermissionPrompt',
      'ApprovalQueue',
      'AgentStatusHeader',
      'WorkingStateCard',
      'SessionPicker',
      'PlanReview',
      'IntegrationStatus',
      'Identity',
    ],
  },
  {
    title: 'Text and typography',
    note: 'The smallest pieces.',
    members: [
      'Text',
      'Paragraph',
      'Heading',
      'Label',
      'Description',
      'Link',
      'ActionLink',
      'Icon',
      'AvatarGlyph',
      'Kbd',
      'ShortcutHint',
      'HintBar',
      'HighlightedText',
      'Button',
      'ButtonGroup',
      'IconButton',
      'ActionBar',
      'KeyboardHelp',
      'FocusLens',
      'DesignInspector',
      'ThemePicker',
    ],
  },
]

function Poster({ demo, label }: { demo: string; label: string }) {
  const ref = useRef<HTMLCanvasElement>(null)
  useEffect(() => {
    let cancelled = false
    const slug = demo.replaceAll('/', '-')
    void fetch(`/preview-posters/${slug}.json`)
      .then((response) => {
        if (!response.ok) throw new Error(`poster ${response.status}`)
        return response.json() as Promise<TerminalFrame>
      })
      .then((frame) => {
        if (cancelled || !ref.current) return
        paintCanvas(ref.current, frame, 4, 8, 1)
        ref.current.style.width = '100%'
        ref.current.style.height = 'auto'
      })
      .catch(() => undefined)
    return () => {
      cancelled = true
    }
  }, [demo])
  return <canvas ref={ref} role="img" aria-label={`${label} preview`} />
}

function Card({ route }: { route: Route }) {
  return (
    <a
      href={`/docs/components/${route.slug}`}
      style={{
        display: 'grid',
        gridTemplateRows: '92px auto',
        overflow: 'hidden',
        border: '1px solid #263126',
        borderRadius: 10,
        color: 'inherit',
        textDecoration: 'none',
        background: '#090c09',
      }}
    >
      <div style={{ overflow: 'hidden', background: '#050705' }}>
        <Poster demo={route.demo} label={route.component} />
      </div>
      <span style={{ display: 'grid', gap: 4, padding: 10 }}>
        <strong style={{ color: '#d8ffd8' }}>{route.component}</strong>
        <code style={{ color: '#39ff14', fontSize: 11 }}>{route.demo}</code>
      </span>
    </a>
  )
}

export function ComponentGallery() {
  const [query, setQuery] = useState('')
  const bySlug = useMemo(() => new Map(routes.map((route) => [route.component, route])), [])

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase()
    if (!needle) return routes
    return routes.filter((route) =>
      `${route.component} ${route.slug} ${route.demo}`.toLowerCase().includes(needle),
    )
  }, [query])
  const visible = useMemo(() => new Set(filtered.map((route) => route.component)), [filtered])

  const grouped = FAMILIES.map((family) => ({
    ...family,
    entries: family.members
      .map((member) => bySlug.get(member))
      .filter((route): route is Route => Boolean(route) && visible.has(route!.component)),
  })).filter((family) => family.entries.length > 0)

  const claimed = new Set(FAMILIES.flatMap((family) => family.members))
  const rest = filtered.filter((route) => !claimed.has(route.component))

  return (
    <div className="not-prose" style={{ display: 'grid', gap: 24 }}>
      <label style={{ display: 'grid', gap: 7, maxWidth: 520 }}>
        <span style={{ color: '#a8b8a8', fontSize: 13 }}>Find a component</span>
        <input
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search list, dialog, input…"
          style={{
            border: '1px solid #334033',
            borderRadius: 8,
            background: '#080b08',
            color: '#d8e8d8',
            padding: '10px 12px',
            font: 'inherit',
          }}
        />
      </label>
      <span aria-live="polite" style={{ color: '#91a091', fontSize: 13 }}>
        {filtered.length} of {routes.length} components
      </span>
      {[...grouped, ...(rest.length ? [{ title: 'More', note: '', members: [], entries: rest }] : [])].map(
        (family) => (
          <section key={family.title} style={{ display: 'grid', gap: 12 }}>
            <div style={{ display: 'grid', gap: 3 }}>
              <h2 style={{ color: '#d8ffd8', margin: 0, fontSize: 18 }}>{family.title}</h2>
              {family.note ? (
                <span style={{ color: '#91a091', fontSize: 13 }}>{family.note}</span>
              ) : null}
            </div>
            <div
              style={{
                display: 'grid',
                gap: 12,
                gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))',
              }}
            >
              {family.entries.map((route) => (
                <Card key={route.slug} route={route} />
              ))}
            </div>
          </section>
        ),
      )}
    </div>
  )
}
