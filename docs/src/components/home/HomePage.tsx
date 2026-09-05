import { Link } from '@tanstack/react-router'
import type { ReactElement } from 'react'
import { CatalogPoster } from '@/components/catalog/CatalogPoster'
import { CopyInstallCommand } from '@/components/home/CopyInstallCommand'
import { TerminalPreview } from '@/components/TerminalPreview'
import {
  catalogComponentFamilies,
  catalogComponents,
  catalogPatterns,
} from '@/generated/catalog'
import { TERMROCK_INSTALL_COMMAND } from '@/lib/install'

type CatalogComponent = (typeof catalogComponents)[number]
type CatalogPattern = (typeof catalogPatterns)[number]
type CatalogEntry = CatalogComponent | CatalogPattern

function requireEntry<T extends CatalogEntry>(entries: readonly T[], slug: string): T {
  const entry = entries.find((candidate) => candidate.slug === slug)
  if (!entry) throw new Error(`Required home catalog entry is missing: ${slug}`)
  return entry
}

const flagship = requireEntry(catalogPatterns, 'agent-workbench')
const featuredComponents = [
  requireEntry(catalogComponents, 'command-palette'),
  requireEntry(catalogComponents, 'data-table'),
  requireEntry(catalogComponents, 'dialog'),
] as const
const featuredPatterns = [
  requireEntry(catalogPatterns, 'terminal-run-card'),
  requireEntry(catalogPatterns, 'error-recovery'),
] as const
const familyProof = catalogComponentFamilies.map((family) => ({
  ...family,
  representative: requireEntry(
    catalogComponents,
    catalogComponents.find((entry) => entry.family === family.id)?.slug ?? '',
  ),
}))

function FeaturedCatalogEntry({
  entry,
  kind,
}: Readonly<{ entry: CatalogEntry; kind: 'Component' | 'Pattern' }>): ReactElement {
  return (
    <article className="home-feature">
      <a href={entry.href}>
        <span className="home-feature__poster">
          <CatalogPoster
            story={entry.story}
            label={entry.title}
            cellWidth={kind === 'Pattern' ? 5 : 4}
            cellHeight={kind === 'Pattern' ? 10 : 8}
          />
        </span>
        <span className="home-feature__body">
          <span className="home-feature__kind">{kind}</span>
          <strong>{entry.title}</strong>
          <span>{entry.purpose}</span>
        </span>
      </a>
    </article>
  )
}

export function HomePage(): ReactElement {
  return (
    <main id="main-content" className="home-page site-main-anchor" tabIndex={-1}>
      <section className="home-hero" aria-labelledby="home-title">
        <div className="home-hero__copy">
          <p className="home-kicker">Rust · Ratatui · product-neutral UI</p>
          <h1 id="home-title">Build terminal software that feels finished.</h1>
          <p className="home-hero__lead">
            TermRock pairs terminal-native components with focus, interaction,
            adaptive layout, and deterministic previews—so your application can
            stay focused on its domain.
          </p>
          <div className="home-actions" aria-label="Start with TermRock">
            <Link
              to="/docs/$"
              params={{ _splat: 'getting-started' }}
              className="home-action home-action--primary"
            >
              Get started
            </Link>
            <Link
              to="/docs/$"
              params={{ _splat: 'components' }}
              className="home-action home-action--secondary"
            >
              Explore components
            </Link>
          </div>
          <nav className="home-journey" aria-label="TermRock learning journey">
            <a href="#preview">Preview</a>
            <a href="#understand">Understand</a>
            <a href="#install">Install</a>
            <a href="#implement">Implement</a>
            <a href="#customize">Customize</a>
            <a href="#advanced">Advanced</a>
          </nav>
        </div>

        <div id="preview" className="home-hero__preview">
          <p className="home-preview-label">
            <span>Flagship task story</span>
            {flagship.title}
          </p>
          <TerminalPreview
            story={flagship.story}
            interactive
            maxHeight={460}
            caption="A real Rust-owned agent workflow: panes, prompt input, overlays, and Esc behavior."
          />
        </div>
      </section>

      <section id="understand" className="home-section home-understand" aria-labelledby="understand-title">
        <div className="home-section__intro">
          <div>
            <p className="home-kicker">Understand</p>
            <h2 id="understand-title">A UI capability layer, not an application framework.</h2>
          </div>
          <p>
            TermRock owns the reusable terminal mechanics. You keep the decisions
            that make your product yours.
          </p>
        </div>
        <div className="home-ownership">
          <section aria-labelledby="termrock-owns">
            <h3 id="termrock-owns">TermRock owns</h3>
            <ul>
              <li>Rendering, hit geometry, and narrow-terminal contraction</li>
              <li>Focus, keyboard, pointer, and overlay interaction contracts</li>
              <li>Semantic roles, density, Unicode, and non-color cues</li>
            </ul>
          </section>
          <section aria-labelledby="your-app-owns">
            <h3 id="your-app-owns">Your application owns</h3>
            <ul>
              <li>Domain state, language, permissions, and process policy</li>
              <li>Async effects, persistence, networking, and secrets</li>
              <li>Projection into components and handling typed outcomes</li>
            </ul>
          </section>
        </div>
      </section>

      <section id="install" className="home-section home-install" aria-labelledby="install-title">
        <div className="home-section__intro">
          <div>
            <p className="home-kicker">Install</p>
            <h2 id="install-title">Add the crate from Git.</h2>
          </div>
          <p>
            TermRock is pre-stable research software. Evaluate the current line,
            then pin the exact revision your team reviewed.
          </p>
        </div>
        <CopyInstallCommand command={TERMROCK_INSTALL_COMMAND} />
        <Link to="/docs/$" params={{ _splat: 'installation' }} className="home-text-link">
          Installation and revision pinning →
        </Link>
      </section>

      <section id="implement" className="home-section" aria-labelledby="implement-title">
        <div className="home-section__intro">
          <div>
            <p className="home-kicker">Implement</p>
            <h2 id="implement-title">Choose the smallest capable primitive.</h2>
          </div>
          <ol className="home-implementation-steps">
            <li><span>01</span> Project domain data into stable component IDs.</li>
            <li><span>02</span> Keep component state beside the owning screen.</li>
            <li><span>03</span> Translate typed outcomes into application effects.</li>
          </ol>
        </div>

        <div className="home-feature-grid home-feature-grid--components">
          {featuredComponents.map((entry) => (
            <FeaturedCatalogEntry key={entry.id} entry={entry} kind="Component" />
          ))}
        </div>

        <div className="home-family-proof">
          <div className="home-family-proof__heading">
            <h3>Component families</h3>
            <p>Browse by the terminal job, then inspect one canonical public identity.</p>
          </div>
          <ul>
            {familyProof.map(({ representative, ...family }) => (
              <li key={family.id}>
                <a href={`/docs/components?family=${family.id}`}>
                  <strong>{family.title}</strong>
                  <span>{family.description}</span>
                  <small>See {representative.title} →</small>
                </a>
              </li>
            ))}
          </ul>
        </div>
      </section>

      <section id="customize" className="home-section home-customize" aria-labelledby="customize-title">
        <div className="home-section__intro">
          <div>
            <p className="home-kicker">Customize</p>
            <h2 id="customize-title">Change the system, not every widget.</h2>
          </div>
          <p>
            Semantic roles, density, glyph capability, motion, and contrast flow
            through one design system. Components consume the result consistently.
          </p>
        </div>
        <pre className="home-code" aria-label="TermRock design system example"><code>{`use termrock::style::{Density, DesignSystem};

let system = DesignSystem::phosphor()
    .density(Density::Compact)
    .ascii();`}</code></pre>
        <Link to="/docs/$" params={{ _splat: 'customization' }} className="home-text-link">
          Customize themes and capabilities →
        </Link>
      </section>

      <section id="advanced" className="home-section" aria-labelledby="advanced-title">
        <div className="home-section__intro">
          <div>
            <p className="home-kicker">Advanced</p>
            <h2 id="advanced-title">Compose screens without surrendering ownership.</h2>
          </div>
          <p>
            Application patterns prove how public components fit together. They
            remain recipes: your application still owns data and effects.
          </p>
        </div>
        <div className="home-feature-grid home-feature-grid--patterns">
          {featuredPatterns.map((entry) => (
            <FeaturedCatalogEntry key={entry.id} entry={entry} kind="Pattern" />
          ))}
        </div>
        <div className="home-advanced-links">
          <Link to="/docs/$" params={{ _splat: 'patterns' }} className="home-action home-action--primary">
            Browse application patterns
          </Link>
          <Link to="/docs/$" params={{ _splat: 'advanced-composition' }} className="home-text-link">
            Advanced composition →
          </Link>
          <Link to="/docs/$" params={{ _splat: 'interaction' }} className="home-text-link">
            Interaction contracts →
          </Link>
        </div>
      </section>
    </main>
  )
}
