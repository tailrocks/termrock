# Plan 005 visual QA

Validated with `agent-browser` at 375×812, 768×1024, and 1440×900, plus
paper-shell desktop captures. Each page stayed viewport-width, its live terminal
accepted keyboard input, and browser errors/failed requests were empty.

| Component | Designer verdict | Evidence |
|---|---|---|
| Dialog | Pass — dim field, elevated body, focused single-line border establish clear modal depth. | [dark](dialog-desktop-1440x900.png), [paper](dialog-paper-1440x900.png) |
| Toast | Pass — muted frame keeps severity on icon/rail; compact hierarchy remains calm. | [dark](toast-desktop-1440x900.png), [paper](toast-paper-1440x900.png) |
| StatusBar | Pass — filled band and restrained dot separators clarify zones without noise. | [dark](status-bar-desktop-1440x900.png), [paper](status-bar-paper-1440x900.png) |
| Popover | Pass — elevated shell floats cleanly; focus uses color, never weight. | [dark](popover-desktop-1440x900.png), [paper](popover-paper-1440x900.png) |
| DropdownMenu | Pass — shared overlay geometry and selected state remain legible. | [dark](dropdown-menu-desktop-1440x900.png), [paper](dropdown-menu-paper-1440x900.png) |
| CompletionMenu | Pass — overlay layering, row hierarchy, loading/empty tour remain distinct. | [dark](completion-menu-desktop-1440x900.png), [paper](completion-menu-paper-1440x900.png) |
| NotificationCenter | Pass — drawer elevation separates archive from canvas with measured density. | [dark](notification-center-desktop-1440x900.png), [paper](notification-center-paper-1440x900.png) |
| Drawer | Pass — shared frame retains the resize handle as a deliberate top layer. | [dark](drawer-desktop-1440x900.png), [paper](drawer-paper-1440x900.png) |

Mobile/tablet evidence for every row uses the matching `-mobile-375x812` and
`-tablet-768x1024` suffixes.
