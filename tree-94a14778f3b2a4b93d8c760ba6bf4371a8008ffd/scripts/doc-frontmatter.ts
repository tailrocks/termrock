export function frontmatter(body: string): string {
  const match = body.match(/^---\n([\s\S]*?)\n---/)
  if (!match) throw new Error('missing frontmatter')
  return match[1]!
}

export function scalar(block: string, key: string): string | undefined {
  return block
    .match(new RegExp(`^${key}:\\s*(.+)$`, 'm'))?.[1]
    ?.trim()
    .replace(/^['"]|['"]$/g, '')
    .replaceAll("''", "'")
}

export function list(block: string, key: string): string[] {
  const line = block.match(new RegExp(`^${key}:(.*)$`, 'm'))
  if (!line) return []
  if (line[1]?.trim() === '[]') return []
  const start = line.index! + line[0].length
  const tail = block.slice(start)
  const values: string[] = []
  for (const row of tail.split('\n')) {
    if (values.length === 0 && row.trim() === '') continue
    const match = row.match(/^\s+-\s+(.+)$/)
    if (!match) break
    values.push(match[1]!.trim().replace(/^['"]|['"]$/g, '').replaceAll("''", "'"))
  }
  return values
}
