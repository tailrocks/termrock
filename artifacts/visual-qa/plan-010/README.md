# Plan 010 browser and designer review

Validated Callout, FullscreenViewer, PreviewCard, ImageSurface,
PromptComposer, and PromptQueue with `agent-browser` at 375×812, 768×1024,
and 1440×900 plus paper/reduced-motion desktop. Keyboard and pointer input
advanced each interactive preview. All pages had zero horizontal page
overflow; wide previews scrolled inside their own containers. Internal links
resolved, frame requests succeeded, and console/page errors stayed clean.
SourceCitation/CitationList share generated frames but have no standalone docs
route; their Unicode-width change is covered by focused tests and refreshed
frame artifacts.

- Designer verdict — Callout: **pass** — compact rail remains primary tone cue;
  section chrome now inherits theme border shape without adding box soup.
  [desktop](callout-desktop-1440x900.png)
- Designer verdict — FullscreenViewer: **pass** — elevated body, quiet border,
  focused chrome, metadata, and actions retain clear ownership hierarchy.
  [desktop](fullscreen-viewer-desktop-1440x900.png)
- Designer verdict — PreviewCard: **pass** — layered loading/content states read
  as a floating resource surface; pin focus remains distinct.
  [desktop](preview-card-desktop-1440x900.png)
- Designer verdict — ImageSurface: **pass** — protocol slot stays calm and
  product-neutral; shared border geometry frames without competing with media.
  [desktop](image-surface-desktop-1440x900.png)
- Designer verdict — PromptComposer: **pass** — composer remains scene focal
  point; Unicode-safe chip/history previews preserve attachment, editor, model,
  context, and action tiers under narrow clipping.
  [desktop](prompt-composer-desktop-1440x900.png)
- Designer verdict — PromptQueue: **pass** — status, selection, failure, and
  queued-copy tiers remain distinct; display-column truncation protects narrow
  and wide-glyph states. [desktop](prompt-queue-desktop-1440x900.png)

Each component has adjacent `mobile-375x812`, `tablet-768x1024`, and
`paper-reduced-motion-1440x900` screenshots in this directory.
