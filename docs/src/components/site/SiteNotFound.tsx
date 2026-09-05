import { Link } from '@tanstack/react-router'
import { HomeLayout } from 'fumadocs-ui/layouts/home'
import { homeOptions } from '@/lib/layout.shared'

export function SiteNotFound() {
  return (
    <HomeLayout {...homeOptions()}>
      <main id="main-content" className="site-not-found site-main-anchor" tabIndex={-1}>
        <p className="site-not-found__code">404 · route not found</p>
        <h1>This path is outside the workshop.</h1>
        <p>
          The page may have moved during a breaking research migration. Search the current
          reference or return to the component catalog.
        </p>
        <div className="site-not-found__actions">
          <Link to="/docs/$" params={{ _splat: 'components' }} className="home-action home-action--primary">
            Browse components
          </Link>
          <Link to="/docs/$" params={{ _splat: '' }} className="home-text-link">
            Read the docs →
          </Link>
        </div>
      </main>
    </HomeLayout>
  )
}
