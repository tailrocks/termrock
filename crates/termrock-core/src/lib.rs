//! Core foundations: ids, events, intents, responses, hit testing, focus, key
//! maps, list navigation, keyed collections and scroll state, with the motion
//! preference.
//!
//! `termrock::core::<module>` is the module of the same name here; the
//! `junie-tui` package re-exports each at that path. `termrock::motion` is
//! `motion`.

pub mod data;
pub mod edit;
pub mod event;
pub mod field;
pub mod focus;
pub mod hints;
pub mod hit;
pub mod id;
pub mod intent;
pub mod item;
pub mod keyboard;
pub mod keyed;
pub mod keymap;
pub mod list;
pub mod motion;
pub mod nav;
pub mod pointer;
pub mod refusal;
pub mod response;
pub mod scroll;
pub mod selection;
pub mod shape;
pub mod typeahead;
