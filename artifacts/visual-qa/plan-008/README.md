# Plan 008 visual QA

## Transcript actor rails

Validated the generated Transcript component page and live actor-rail preview
with `agent-browser` at 375×812, 768×1024, and 1440×900 plus paper/reduced-
motion desktop. The preview accepted keyboard focus/navigation; no page
overflow, console errors, failed requests, or broken links occurred.

Designer verdict: **iterated 1 time, pass** — browser review exposed that the
live interactor still projected a stale two-line fixture, so it now shows a
complete user/assistant/folded-thinking/active-tool turn. Actor rails create
clear grouping without box soup; three-line thinking preview is deliberately
subordinate; the active tool closes the sequence; Unicode, narrow, paper, and
reduced-motion paths remain legible and deterministic. Evidence:
[desktop](transcript-desktop-1440x900.png),
[paper](transcript-paper-1440x900.png),
[mobile](transcript-mobile-375x812.png), and
[tablet](transcript-tablet-768x1024.png).
