import { join, relative } from 'node:path'
import {
  TERMROCK_CARGO_DEPENDENCY,
  TERMROCK_CROSSTERM_DEPENDENCY,
  TERMROCK_GIT_URL,
  TERMROCK_INSTALL_COMMAND,
  TERMROCK_REVIEWED_REVISION,
} from '../src/lib/install'

const root = join(import.meta.dir, '..', '..')
const docsRoot = join(root, 'docs')
const contentRoot = join(docsRoot, 'content', 'docs')
const errors: string[] = []
const retiredSlug = ['interaction', 'notes'].join('-')

function requireText(source: string, expected: string, file: string): void {
  if (!source.includes(expected)) {
    errors.push(`${file}: missing ${JSON.stringify(expected)}`)
  }
}

if (!/^[0-9a-f]{40}$/u.test(TERMROCK_REVIEWED_REVISION)) {
  errors.push('install source: reviewed revision must be a full lowercase Git SHA')
}
if (TERMROCK_INSTALL_COMMAND !== `cargo add termrock --git ${TERMROCK_GIT_URL} --rev ${TERMROCK_REVIEWED_REVISION}`) {
  errors.push('install source: cargo command is not derived from the canonical Git URL and revision')
}

const readmePath = join(root, 'README.md')
const installationPath = join(contentRoot, 'installation.mdx')
const readme = await Bun.file(readmePath).text()
const installation = await Bun.file(installationPath).text()

requireText(readme, TERMROCK_CARGO_DEPENDENCY, 'README.md')
requireText(installation, TERMROCK_INSTALL_COMMAND, 'docs/content/docs/installation.mdx')
requireText(installation, TERMROCK_CARGO_DEPENDENCY, 'docs/content/docs/installation.mdx')
requireText(installation, TERMROCK_CROSSTERM_DEPENDENCY, 'docs/content/docs/installation.mdx')

const implementationPanel = await Bun.file(join(docsRoot, 'src', 'components', 'docs', 'ImplementationPanel.tsx')).text()
const homePage = await Bun.file(join(docsRoot, 'src', 'components', 'home', 'HomePage.tsx')).text()
requireText(implementationPanel, "import { TERMROCK_INSTALL_COMMAND } from '@/lib/install'", 'docs/src/components/docs/ImplementationPanel.tsx')
requireText(homePage, "import { TERMROCK_INSTALL_COMMAND } from '@/lib/install'", 'docs/src/components/home/HomePage.tsx')

const textFiles: string[] = [readmePath]
for (const directory of [contentRoot, join(docsRoot, 'src')] as const) {
  for await (const name of new Bun.Glob('**/*.{mdx,json,ts,tsx}').scan({ cwd: directory })) {
    const path = join(directory, name)
    if (path !== join(docsRoot, 'src', 'lib', 'install.ts')) textFiles.push(path)
  }
}

for (const path of textFiles) {
  const source = await Bun.file(path).text()
  const file = relative(root, path)
  if (source.includes(retiredSlug)) errors.push(`${file}: retired handbook route remains`)
  for (const match of source.matchAll(/cargo add termrock[^\r\n]*/gu)) {
    if (match[0].trim() !== TERMROCK_INSTALL_COMMAND) {
      errors.push(`${file}: floating or non-repository cargo add command`)
    }
  }
}

if (await Bun.file(join(contentRoot, `${retiredSlug}.mdx`)).exists()) {
  errors.push(`docs/content/docs/${retiredSlug}.mdx: retired handbook page still exists`)
}
if (!(await Bun.file(join(contentRoot, 'advanced-composition.mdx')).exists())) {
  errors.push('docs/content/docs/advanced-composition.mdx: advanced composition guide is missing')
}

if (errors.length > 0) throw new Error(`docs content failures:\n${errors.join('\n')}`)
console.log(`docs content: pinned ${TERMROCK_REVIEWED_REVISION.slice(0, 12)} install + no retired handbook route`)
