# Plan 004 visual QA

Validated with `agent-browser` against the docs dev server at 375×812,
768×1024, and 1440×900. Every page had viewport-width document geometry,
accepted terminal keyboard input, and reported no page errors or failed
requests. Desktop paper-theme captures verify the alternate light shell while
the terminal preview retains its deliberate phosphor identity.

| Component | Designer verdict | Evidence |
|---|---|---|
| Button | Pass — primary chip is focal; quiet/link hierarchy remains restrained. | [dark desktop](button-desktop-1440x900.png), [paper](button-paper-1440x900.png) |
| Badge | Pass — soft chips read as compact metadata without neon slabs. | [dark desktop](badge-desktop-1440x900.png), [paper](badge-paper-1440x900.png) |
| List | Pass — selected gutter plus wash is clear without overwhelming row text. | [dark desktop](list-desktop-1440x900.png), [paper](list-paper-1440x900.png) |
| Progress | Pass — sunken track and fractional fill improve hierarchy and precision. | [dark desktop](progress-desktop-1440x900.png), [paper](progress-paper-1440x900.png) |
| EmptyState | Pass, iterated once — bounded 56×12 card removed the first render's dead void. | [dark desktop](empty-state-desktop-1440x900.png), [paper](empty-state-paper-1440x900.png) |

Mobile and tablet captures for each component sit beside these files using the
`-mobile-375x812` and `-tablet-768x1024` suffixes.
