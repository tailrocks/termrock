# Engineering

Public components borrow render data, use stable caller IDs, separate state
from rendering, and expose logical outcomes rather than effects. Base modules
must not depend on Tokio. Crossterm is an optional adapter only.
