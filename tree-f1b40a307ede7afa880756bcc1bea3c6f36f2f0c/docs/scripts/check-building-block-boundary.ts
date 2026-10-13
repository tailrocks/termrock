/**
 * Structural gate: product-noun composites must not be first-class
 * `termrock::widgets` pub-use exports. Example home is `termrock::patterns`.
 *
 * Usage: bun run docs/scripts/check-building-block-boundary.ts
 * Exit 0 = clean; exit 1 = leaky export found.
 */

const root = `${import.meta.dir}/../..`
const widgetsMod = `${root}/crates/termrock/src/widgets/mod.rs`
const patternsMod = `${root}/crates/termrock/src/patterns/mod.rs`
const catalog = `${root}/crates/termrock/src/registry/catalog.rs`

/** Surfaces that must NOT appear in widgets pub-use lists. */
const FORBIDDEN_WIDGET_EXPORTS = [
  'ConnectionManager',
  'SessionPicker',
  'MetricsDashboard',
  'QueryEditor',
  'SchemaBrowser',
  'ResultGrid',
  'AgentStatusHeader',
  'IntegrationStatus',
  'ApprovalQueue',
  'WorkingStateCard',
  'PlanReview',
  'PromptQueue',
  'ActivityShelf',
  'BackgroundTaskPanel',
  'SubagentCard',
  'TerminalRunCard',
  'ProcessTable',
  'TaskRail',
  'AuthEntryState',
  'AuthEntryMode',
  'OpsDashboardState',
  'OpsDashboardOutcome',
  'OpsRegion',
  'ResourceBrowserState',
  'ResourceBrowserOutcome',
  'FileManagerState',
  'GitWorkbenchState',
  'DatabaseWorkbenchState',
  'ObservabilityDashboardState',
  'ProjectLauncherState',
  'HelpCenterState',
  'ErrorRecoveryState',
  'AppDashboardState',
  'AgentWorkbenchState',
  'SettingsScreenState',
  'SetupWizardState',
  'example_agent_workbench_nav',
  'example_database_nav',
  'workbench_panels',
  'dashboard_panels',
  'modes_to_workbench',
] as const

/** Must remain on patterns for demos. */
const REQUIRED_PATTERN_EXPORTS = [
  'ConnectionManager',
  'AuthEntryState',
  'example_agent_workbench_nav',
  'example_database_nav',
] as const

async function main() {
  const wmod = await Bun.file(widgetsMod).text()
  const pmod = await Bun.file(patternsMod).text()
  const cat = await Bun.file(catalog).text()
  const errors: string[] = []

  const pubUseExported = (modText: string, name: string): boolean => {
    const brace = new RegExp(
      String.raw`pub use [^{;]*\{[^}]*\b${name}\b`,
      's',
    )
    const direct = new RegExp(String.raw`pub use \w+::${name}\b`)
    const fn = new RegExp(String.raw`\bpub fn ${name}\b`)
    return brace.test(modText) || direct.test(modText) || fn.test(modText)
  }

  for (const name of FORBIDDEN_WIDGET_EXPORTS) {
    if (pubUseExported(wmod, name)) {
      errors.push(`widgets exports forbidden composite/product surface: ${name}`)
    }
  }

  for (const name of REQUIRED_PATTERN_EXPORTS) {
    if (!pubUseExported(pmod, name) && !pmod.includes(name)) {
      errors.push(`patterns missing expected example surface: ${name}`)
    }
  }

  // Inverted deps: widgets must not code-import patterns
  const widgetsDir = `${root}/crates/termrock/src/widgets`
  const glob = new Bun.Glob('**/*.rs')
  for await (const rel of glob.scan(widgetsDir)) {
    const path = `${widgetsDir}/${rel}`
    const text = await Bun.file(path).text()
    for (const [i, line] of text.split('\n').entries()) {
      if (/^\s*use crate::patterns/.test(line)) {
        errors.push(`inverted dep ${path}:${i + 1}: ${line.trim()}`)
      }
    }
  }

  // Catalog: no removed widgets path for connection_manager
  if (cat.includes('widgets/connection_manager.rs')) {
    errors.push(
      'catalog still references widgets/connection_manager.rs (must be patterns/)',
    )
  }
  if (!cat.includes('patterns/connection_manager.rs')) {
    errors.push('catalog missing patterns/connection_manager.rs primary/provenance')
  }

  if (errors.length) {
    console.error('building-block boundary FAILED:')
    for (const e of errors) console.error(`  - ${e}`)
    process.exit(1)
  }
  console.log('building-block boundary PASS')
  console.log(`  checked ${FORBIDDEN_WIDGET_EXPORTS.length} forbidden widgets exports`)
  console.log('  inverted deps: clean')
  console.log('  catalog connection_manager path: patterns')
}

main().catch((e) => {
  console.error(e)
  process.exit(1)
})
