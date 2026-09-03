// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Handle-based WASM adapter over the canonical TermRock catalog.

use std::{cell::RefCell, collections::BTreeMap};

use termrock_catalog::host::{CatalogSession, DemoEvent, catalog};
use wasm_bindgen::prelude::*;

thread_local! {
    static SESSIONS: RefCell<SessionStore> = RefCell::new(SessionStore::default());
}

#[derive(Default)]
struct SessionStore {
    next: u32,
    sessions: BTreeMap<u32, CatalogSession>,
}

impl SessionStore {
    fn mount(&mut self, id: &str, cols: u16, rows: u16) -> Result<u32, String> {
        self.next = self
            .next
            .checked_add(1)
            .ok_or("demo handle space exhausted")?;
        let handle = self.next;
        let session = CatalogSession::mount(id, cols, rows)?;
        self.sessions.insert(handle, session);
        Ok(handle)
    }

    fn get_mut(&mut self, handle: u32) -> Result<&mut CatalogSession, String> {
        self.sessions
            .get_mut(&handle)
            .ok_or_else(|| format!("unknown demo handle: {handle}"))
    }
}

/// Serialize the one shared catalog.
#[wasm_bindgen]
pub fn catalog_json() -> Result<String, JsValue> {
    serde_json::to_string(&catalog()).map_err(js_error)
}

/// Mount one persistent catalog page and return its opaque handle.
#[wasm_bindgen]
pub fn mount_demo(id: &str, cols: u16, rows: u16) -> Result<u32, JsValue> {
    SESSIONS.with_borrow_mut(|store| store.mount(id, cols, rows).map_err(js_error))
}

/// Dispatch one normalized JSON event and serialize the resulting presentation update.
#[wasm_bindgen]
pub fn dispatch_demo(handle: u32, event_json: &str) -> Result<String, JsValue> {
    let event: DemoEvent = serde_json::from_str(event_json).map_err(js_error)?;
    SESSIONS
        .with_borrow_mut(|store| {
            let update = store.get_mut(handle)?.dispatch(event)?;
            serde_json::to_string(&update).map_err(|error| error.to_string())
        })
        .map_err(js_error)
}

/// Serialize the current truecolor frame for one mounted page.
#[wasm_bindgen]
pub fn demo_frame(handle: u32) -> Result<String, JsValue> {
    SESSIONS
        .with_borrow_mut(|store| {
            let frame = store.get_mut(handle)?.frame();
            serde_json::to_string(&frame).map_err(|error| error.to_string())
        })
        .map_err(js_error)
}

/// Reset one session to its initial state without remounting another page.
#[wasm_bindgen]
pub fn reset_demo(handle: u32) -> Result<String, JsValue> {
    SESSIONS
        .with_borrow_mut(|store| {
            let session = store.get_mut(handle)?;
            session.reset();
            serde_json::to_string(&session.frame()).map_err(|error| error.to_string())
        })
        .map_err(js_error)
}

/// Destroy one mounted session. Unknown handles are rejected.
#[wasm_bindgen]
pub fn unmount_demo(handle: u32) -> Result<(), JsValue> {
    SESSIONS.with_borrow_mut(|store| {
        store
            .sessions
            .remove(&handle)
            .map(|_| ())
            .ok_or_else(|| js_error(format!("unknown demo handle: {handle}")))
    })
}

fn js_error(error: impl ToString) -> JsValue {
    JsValue::from_str(&error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_isolate_and_reject_unknown_sessions() {
        let mut store = SessionStore::default();
        let first = store.mount("overview", 80, 24).unwrap();
        let second = store.mount("overview", 80, 24).unwrap();
        let event: DemoEvent =
            serde_json::from_str(r#"{"type":"key","key":"]","kind":"press"}"#).unwrap();
        store.get_mut(first).unwrap().dispatch(event).unwrap();
        let changed = store.get_mut(first).unwrap().frame();
        let untouched = store.get_mut(second).unwrap().frame();
        assert_ne!(changed.cells, untouched.cells);
        store.sessions.remove(&first);
        assert!(store.get_mut(first).is_err());
    }

    #[test]
    fn update_json_exposes_deadline_kind_and_semantic_revision() {
        let mut store = SessionStore::default();
        let handle = store.mount("buttons", 80, 24).unwrap();
        let tab: DemoEvent =
            serde_json::from_str(r#"{"type":"key","key":"Tab","kind":"press"}"#).unwrap();
        store.get_mut(handle).unwrap().dispatch(tab).unwrap();
        let enter: DemoEvent =
            serde_json::from_str(r#"{"type":"key","key":"Enter","kind":"press"}"#).unwrap();
        let update = store.get_mut(handle).unwrap().dispatch(enter).unwrap();
        let json = serde_json::to_value(update).unwrap();
        assert_eq!(json["deadlineKind"], "functional");
        assert!(
            json["semanticRevision"]
                .as_u64()
                .is_some_and(|value| value > 0)
        );
    }
}
