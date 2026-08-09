#!/usr/bin/env bash
# Structural gate for TermRock shadcn-TUI experience research SoTs.
# Fails if required landscape themes or actionable improvement concepts are missing.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PRIMARY="$ROOT/docs/design/experience-research-2026.md"
COMPETITIVE="$ROOT/docs/design/competitive-tui-research.md"

fail() {
  echo "check-experience-research: FAIL: $*" >&2
  exit 1
}

[[ -f "$PRIMARY" ]] || fail "missing $PRIMARY"
[[ -f "$COMPETITIVE" ]] || fail "missing $COMPETITIVE"

# Combined corpus so either SoT may host depth, but primary must exist and carry core thesis.
CORPUS=$(cat "$PRIMARY" "$COMPETITIVE")

require() {
  local label="$1"
  local pattern="$2"
  if ! printf '%s' "$CORPUS" | rg -q "$pattern"; then
    fail "required theme missing: $label (pattern: $pattern)"
  fi
}

# Acceptance: TermRock-as-shadcn framing
require "shadcn" 'shadcn'
require "TermRock thesis" 'TermRock'
require "hybrid / source-owned" 'source-owned|source.owned|registry'

# Acceptance: Grok Build + Amp agent references (substantive section titles / phrases)
require "Grok Build" 'Grok Build'
require "Amp" 'Amp \(ampcode|ampcode\.com|Amp is|Amp for|Amp \('
require "ampcode" 'ampcode\.com'
# Amp agent modes must match ampcode.com/manual (not deprecated smart/deep/rush as the four dials)
require "Amp modes low" 'low'
require "Amp modes medium" 'medium'
require "Amp modes high" 'high'
require "Amp modes ultra" 'ultra'
# Guard: competitive §9.2 must not list smart/free as the four capability dials
if rg -n 'Modes.*\(`smart`' "$COMPETITIVE" 2>/dev/null; then
  fail "competitive research lists deprecated Amp modes (smart) as capability dials; use low/medium/high/ultra"
fi

# Acceptance: hero / community consensus + multi-lang (need ≥2 hero names + ≥2 stacks)
require "awesome-tuis or awesometui" 'awesome-tuis|awesometui\.com'
require "lazygit" 'lazygit'
require "k9s" 'k9s'
require "btop" 'btop'
require "yazi" 'yazi'
require "Bubble Tea" 'Bubble Tea'
require "Textual" 'Textual'

# Acceptance: actionable improvement concepts + quality over compatibility
require "composer continuity or dual-kill/pre-1.0" 'Composer continuity|dual-kill|pre-1\.0|dual.authority'
require "blocks / experience packs" 'agent-workbench|experience pack|source-owned blocks|ModeRibbon|Studio'
require "breaking / quality over compatibility" 'breaking|quality over compatibility'

# Primary file must include a roadmap / concept catalog section
rg -q 'Concept catalog|Think-big roadmap|Roadmap' "$PRIMARY" \
  || fail "primary research missing concept catalog or roadmap heading"

# Primary must have dedicated Grok + Amp sections
rg -q 'Grok Build' "$PRIMARY" || fail "primary missing Grok Build discussion"
rg -q 'Amp' "$PRIMARY" || fail "primary missing Amp discussion"

echo "check-experience-research: OK"
echo "  primary: $PRIMARY ($(wc -l < "$PRIMARY") lines)"
echo "  competitive: $COMPETITIVE ($(wc -l < "$COMPETITIVE") lines)"
