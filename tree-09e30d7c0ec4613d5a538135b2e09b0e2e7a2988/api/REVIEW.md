# Public API review

The reviewed report confirms stable-ID interaction surfaces, borrowed render data, pure rendering and OSC encoding, empty default features, no executor dependency, and Crossterm isolation. Application nouns and policies are absent from public signatures. The post-`v0.6.0` migration deliberately moves dialog layout, focus/hover/modal lifecycle, scroll rendering, and the modal backdrop into their canonical `layout`, `interaction`, `scroll`, and `widgets` namespaces; the report must not reintroduce their former donor-shaped component submodules.
