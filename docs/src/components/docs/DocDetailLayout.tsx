import type { ReactElement, ReactNode } from 'react'
import { SeenInApplications } from '@/components/SeenInApplications'
import { TerminalPreview } from '@/components/TerminalPreview'
import { ImplementationPanel } from '@/components/docs/ImplementationPanel'
import {
  familyTitle,
  relatedDocs,
  type DocCatalogEntry,
} from '@/components/docs/model'
import './detail.css'

export type DocDetailLayoutProps = Readonly<{
  entry: DocCatalogEntry
  children?: ReactNode
}>

type BehaviorItem = Readonly<{
  title: string
  detail: string
}>

function behaviorItems(entry: DocCatalogEntry): readonly BehaviorItem[] {
  if (entry.hints.length > 0) {
    return entry.hints.slice(0, 6).map((hint) => ({
      title: hint,
      detail: `Runs against the same mounted ${entry.story} state.`,
    }))
  }

  return [
    {
      title: entry.storyTitle,
      detail: `Representative ${entry.dimensions.cols}×${entry.dimensions.rows} terminal state.`,
    },
    {
      title: 'Passive paint',
      detail: 'No keyboard or pointer action is claimed by this representative story.',
    },
    {
      title: 'Evidence stays explicit',
      detail: `${entry.coverage.covered} covered · ${entry.coverage.partial} partial · ${entry.coverage.missing} missing axes.`,
    },
  ]
}

function EntryFacts({ entry }: Readonly<{ entry: DocCatalogEntry }>): ReactElement {
  return (
    <dl className="doc-detail-facts">
      <div>
        <dt>Kind</dt>
        <dd>{entry.renderKind ?? entry.entryKind}</dd>
      </div>
      <div>
        <dt>Input</dt>
        <dd>{entry.interactive ? 'interactive' : 'paint only'}</dd>
      </div>
      <div>
        <dt>Canvas</dt>
        <dd>
          {entry.dimensions.cols}×{entry.dimensions.rows}
        </dd>
      </div>
    </dl>
  )
}

function Composition({ entry }: Readonly<{ entry: DocCatalogEntry }>): ReactElement {
  if (entry.entryKind === 'pattern') {
    return (
      <div className="doc-detail-composition">
        <div>
          <h3>Public building blocks</h3>
          <p>{entry.uses.length > 0 ? entry.uses.join(' · ') : 'No canonical component dependency recorded.'}</p>
        </div>
        <div>
          <h3>Supporting types</h3>
          <p>
            {entry.supportingTypes.length > 0
              ? entry.supportingTypes.join(' · ')
              : 'No supporting type dependency recorded.'}
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="doc-detail-composition">
      <div>
        <h3>Variants</h3>
        <p>
          Use the preview Variant menu when alternate registered stories exist. Each
          selection mounts a fresh configuration.
        </p>
      </div>
      <div>
        <h3>Composition</h3>
        <p>
          {familyTitle(entry)} · {entry.tags.join(' · ')}
        </p>
      </div>
    </div>
  )
}

function Reference({ entry }: Readonly<{ entry: DocCatalogEntry }>): ReactElement {
  const sourceUrl = `https://github.com/tailrocks/termrock/blob/main/${entry.source}`
  return (
    <div className="doc-detail-reference-grid">
      <div>
        <p className="doc-detail-kicker">API</p>
        <strong>{entry.publicUi ?? entry.id}</strong>
        <a href={sourceUrl}>Open source ↗</a>
      </div>
      <div>
        <p className="doc-detail-kicker">Tokens</p>
        <strong>DesignSystem</strong>
        <span>Inspect exact story code for roles and capability projection.</span>
      </div>
      <div>
        <p className="doc-detail-kicker">Accessibility</p>
        <strong>{entry.interactive ? 'Input contract mounted' : 'Passive semantics'}</strong>
        <span>
          {entry.hints.length > 0
            ? entry.hints.join(' · ')
            : 'No input claim in the representative story.'}
        </span>
      </div>
      <div>
        <p className="doc-detail-kicker">Contract</p>
        <strong>{entry.coverage.complete ? 'Complete' : 'Evidence in progress'}</strong>
        <span>
          {entry.coverage.covered}/{entry.coverage.total} axes covered
        </span>
      </div>
    </div>
  )
}

function Related({ entry }: Readonly<{ entry: DocCatalogEntry }>): ReactElement {
  const related = relatedDocs(entry)
  return (
    <section id="related" className="doc-detail-section">
      <p className="doc-detail-kicker">Continue</p>
      <h2>Related</h2>
      {related.length > 0 ? (
        <div className="doc-detail-related">
          {related.map((item) => (
            <a key={item.href} href={item.href}>
              <span>{item.title}</span>
              <small>{item.relationship}</small>
            </a>
          ))}
        </div>
      ) : (
        <p className="doc-detail-note">No direct catalog relationship recorded.</p>
      )}
      {entry.entryKind === 'component' ? (
        <details className="doc-detail-details">
          <summary>Seen in application patterns</summary>
          <SeenInApplications component={entry.id} />
        </details>
      ) : null}
    </section>
  )
}

function DocDetailLayout({ entry, children }: DocDetailLayoutProps): ReactElement {
  const behavior = behaviorItems(entry)
  const kind = entry.entryKind === 'component' ? 'Component' : 'Pattern'

  return (
    <div className="doc-detail">
      <header className="doc-detail-header">
        <p className="doc-detail-eyebrow">
          {kind} · {familyTitle(entry)}
        </p>
        <h1>{entry.title}</h1>
        <p className="doc-detail-lede">{entry.purpose}</p>
        <EntryFacts entry={entry} />
        <nav aria-label="Page sections" className="doc-detail-nav">
          <a href="#purpose">Purpose</a>
          <a href="#behavior">Behavior</a>
          <a href="#implement">Implement</a>
          <a href="#reference">Reference</a>
          <a href="#related">Related</a>
        </nav>
      </header>

      <div className="doc-detail-story">
        <section aria-labelledby="preview-title" className="doc-detail-preview-column">
          <div className="doc-detail-preview-sticky">
            <p id="preview-title" className="doc-detail-kicker">
              Live preview
            </p>
            <TerminalPreview
              story={entry.story}
              interactive={entry.interactive}
              caption={`${entry.title} · exact mounted Rust story`}
            />
          </div>
        </section>

        <div className="doc-detail-narrative">
          <section id="purpose" className="doc-detail-section">
            <p className="doc-detail-kicker">01 · Purpose</p>
            <h2>What it is for</h2>
            <p>{entry.purpose}</p>
            <p className="doc-detail-fit">
              Best fit: <strong>{familyTitle(entry)}</strong>
              {entry.tags.length > 0 ? ` · ${entry.tags.join(' · ')}` : ''}
            </p>
          </section>

          <section id="behavior" className="doc-detail-section">
            <p className="doc-detail-kicker">02 · Behavior</p>
            <h2>What the mounted story proves</h2>
            <ol className="doc-detail-behaviors">
              {behavior.map((item, index) => (
                <li key={`${entry.id}-${index}-${item.title}`}>
                  <span>{String(index + 1).padStart(2, '0')}</span>
                  <div>
                    <h3>{item.title}</h3>
                    <p>{item.detail}</p>
                  </div>
                </li>
              ))}
            </ol>
          </section>
        </div>
      </div>

      <section id="implement" className="doc-detail-section doc-detail-section-wide">
        <p className="doc-detail-kicker">03 · Implement</p>
        <h2>Install, then start from exact code</h2>
        <ImplementationPanel story={entry.story} />
      </section>

      <section id="composition" className="doc-detail-section doc-detail-section-wide">
        <p className="doc-detail-kicker">04 · Adapt</p>
        <h2>Variants and composition</h2>
        <Composition entry={entry} />
      </section>

      <section id="reference" className="doc-detail-section doc-detail-section-wide">
        <p className="doc-detail-kicker">05 · Reference</p>
        <h2>API, tokens, accessibility</h2>
        <Reference entry={entry} />
      </section>

      <section className="doc-detail-section doc-detail-section-wide">
        <p className="doc-detail-kicker">06 · Go deeper</p>
        <h2>Advanced guidance</h2>
        <div className="doc-detail-disclosures">
          {entry.authoredGuidance ? (
            <details className="doc-detail-details">
              <summary>Authored implementation guidance</summary>
              <div className="doc-detail-authored">{children}</div>
            </details>
          ) : null}
          <details className="doc-detail-details">
            <summary>Ownership boundary</summary>
            <p>
              TermRock owns reusable terminal rendering and interaction state. The host
              owns domain data, policy, persistence, authorization, and side effects.
            </p>
          </details>
          <details className="doc-detail-details">
            <summary>Evidence status</summary>
            <p>
              {entry.coverage.covered} covered, {entry.coverage.partial} partial, and{' '}
              {entry.coverage.missing} missing contract axes. Missing evidence is not a
              behavior claim.
            </p>
          </details>
        </div>
      </section>

      <Related entry={entry} />
    </div>
  )
}

export function ComponentDocLayout(props: DocDetailLayoutProps): ReactElement {
  return <DocDetailLayout {...props} />
}

export function PatternDocLayout(props: DocDetailLayoutProps): ReactElement {
  return <DocDetailLayout {...props} />
}
