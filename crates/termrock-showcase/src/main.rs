// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! The showcase binary: a terminal session around [`termrock_showcase::app::App`].

use std::io;
use std::ops::ControlFlow;
use std::time::Duration;

use termrock::runtime::{Instant, RunOptions, run};

use termrock_showcase::app::App;

fn main() -> io::Result<()> {
    let mut app = App::new();
    run(
        &mut app,
        RunOptions::default(),
        |app, frame, _tick| {
            let area = frame.area();
            app.render(frame.buffer_mut(), area);
        },
        |app, event, tick| {
            app.update(event, tick);
            if app.quit {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        },
        |app| {
            // A scripted agent owes the next frame at its next scheduled
            // event; an idle one owes nothing and the loop blocks.
            app.next_due_ms()
                .map(|due| Instant::now() + Duration::from_millis(due.min(1_000)))
        },
    )
}
