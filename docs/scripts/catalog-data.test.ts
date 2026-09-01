import { describe, expect, test } from 'bun:test'
import {
  REQUIRED_AXES,
  REQUIRED_LINTS,
  contractSchemaSource,
  hardcodedKeyMatches,
  parseContractDocument,
} from './catalog-data'

function contractDocument(expires: unknown): unknown {
  const axes = Object.fromEntries(REQUIRED_AXES.map((axis) => [axis, {
    applicability: axis === 'visual_states' ? 'required' : 'conditional',
    status: axis === 'visual_states' ? 'partial' : 'missing',
    evidence: {
      stories: axis === 'visual_states' ? ['fixture/basic'] : [],
      tests: [],
      recordings: [],
      benches: [],
    },
    reason: 'Focused parser fixture.',
    ...(axis === 'keyboard' ? { waiver: { ticket: 'TEST-1', expires } } : {}),
  }]))
  const lints = Object.fromEntries(REQUIRED_LINTS.map((lint) => [lint, 'not_run']))
  return {
    schema: 2,
    version: 'test',
    entries: [{
      schema: 2,
      id: 'Fixture',
      entryKind: 'component',
      complete: false,
      axes,
      lints,
    }],
  }
}

describe('contract parser authority', () => {
  test('accepts a real ISO calendar date and publishes the date format', () => {
    expect(() => parseContractDocument(contractDocument('2028-02-29'))).not.toThrow()
    expect(contractSchemaSource()).toContain('"format": "date"')
  })

  test('rejects a calendar rollover that matches only the date shape', () => {
    expect(() => parseContractDocument(contractDocument('2027-02-29'))).toThrow(
      'invalid ISO 8601 calendar date',
    )
  })

  test('publishes exact source, poster, check, and applicability fields without snapshot aliases', () => {
    const schema = contractSchemaSource()
    expect(schema).toContain('"applicability"')
    expect(schema).toContain('"posters"')
    expect(schema).toContain('"sources"')
    expect(schema).toContain('"checks"')
    expect(schema).not.toContain('"snapshots"')
  })
})

describe('source-aware hardcoded key evidence', () => {
  test('reports exact production lines and excludes the test module', () => {
    const source = [
      'fn handle(key: KeyCode) {',
      '  if matches!(key, KeyCode::Enter) {}',
      '}',
      '#[cfg(test)]',
      'mod tests {',
      '  const KEY: KeyCode = KeyCode::Esc;',
      '}',
    ].join('\n')
    expect(hardcodedKeyMatches(source)).toEqual([{ literal: 'KeyCode::Enter', line: 2 }])
  })
})
