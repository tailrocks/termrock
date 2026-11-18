use crate::keyhint::{Hint, hint};

pub const EDIT: Hint = hint(EDIT_PAIR.0, EDIT_PAIR.1);

pub const EDIT_PAIR: (&str, &str) = ("Enter", "Edit");
