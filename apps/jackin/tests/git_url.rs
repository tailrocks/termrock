#![cfg(feature = "demo")]

mod common;

use common::*;
use jackin_console::fixtures::Scenario;
use termrock::core::event::Input;
use termrock::runtime::Application;

fn entered(url: &str) -> String {
    let mut app = app(Scenario::Returning);
    press(&mut app, "n g");
    Application::handle(&mut app, Input::Paste(url.to_owned()));
    press(&mut app, "enter");
    screen(&mut app)
}

#[test]
fn a_known_repository_url_in_any_spelling_is_accepted() {
    for url in [
        "github.com/chainargos/payments-platform",
        "https://github.com/chainargos/payments-platform",
        "https://GitHub.com/chainargos/payments-platform.git",
        "git@github.com:chainargos/payments-platform.git",
        "ssh://git@github.com/chainargos/payments-platform/",
        "git://github.com/chainargos/payments-platform",
        "https://github.com/ChainArgos/Payments-Platform",
    ] {
        let shown = entered(url);
        assert!(shown.contains("step 2 of 5"), "{url}:\n{shown}");
    }
}

#[test]
fn a_url_that_only_contains_a_known_repository_is_refused() {
    for url in [
        "https://evil.invalid/chainargos/payments-platform",
        "https://github.com/other/chainargos/payments-platform",
        "https://github.com/chainargos/payments-platform-extra",
        "https://github.com.evil.invalid/chainargos/payments-platform",
        "https://github.com@evil.invalid/chainargos/payments-platform",
        "ext::sh -c touch/tmp/x://github.com/chainargos/payments-platform",
        "file://github.com/chainargos/payments-platform",
        "ftp://github.com/chainargos/payments-platform",
        "-oProxyCommand=touch /tmp/x@github.com:chainargos/payments-platform",
        "-oProxyCommand=x@github.com:chainargos/payments-platform",
        "-x@github.com:chainargos/payments-platform",
        "--upload-pack=x@github.com:chainargos/payments-platform",
        "https://github.com/chainargos/payments-platform extra",
        "https://github.com/chainargos/payments-platform\u{7}",
        "a=b@github.com:chainargos/payments-platform",
        "user:pass@github.com:chainargos/payments-platform",
        "https://u:p@github.com/chainargos/payments-platform",
        "ext::github.com/chainargos/payments-platform",
    ] {
        let shown = entered(url);
        assert!(shown.contains("Could not resolve"), "{url}:\n{shown}");
        assert!(shown.contains("step 1 of 5"), "{url}:\n{shown}");
    }
}
