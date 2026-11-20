//! The command echo (`command.echo`, the compatibility name `CMDECHO`): while it is on, each typed
//! line of a script is written to the echo stream before it runs. The setting is kept for the
//! process, so this test has a binary of its own.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

mod common;

use common::Harness;
use draftbuilder_commands::script;

/// The lines a script echoes: the `detail` of each echo record, in order.
fn echoed(h: &mut Harness, text: &str) -> Vec<String> {
    let mut sink = Vec::new();
    script::run(&h.document, &h.registry, &mut h.session, text, &mut sink);
    String::from_utf8(sink)
        .unwrap()
        .lines()
        .map(|line| {
            let record: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(record["event"], "script.echo");
            record["detail"].as_str().unwrap().to_owned()
        })
        .collect()
}

#[test]
fn the_command_echo_writes_each_typed_line_until_it_is_switched_off() {
    let mut h = Harness::new();
    // The echo is on by default: every line is echoed before it runs.
    let first = echoed(&mut h, "NEW unit=meter\nLINE\n0,0\n1,0\n\n");
    assert_eq!(first, ["NEW unit=meter", "LINE", "0,0", "1,0", ""]);

    // The line that switches it off is echoed; the lines after it are not.
    let second = echoed(&mut h, "SETVAR name=CMDECHO value=off\nLINE\n0,1\n1,1\n\n");
    assert_eq!(second, ["SETVAR name=CMDECHO value=off"]);

    // Switched back on, the echo returns (the setting is kept for the process). The line that
    // switches it on is not echoed: the echo is read before each line runs.
    let third = echoed(
        &mut h,
        "SETVAR name=CMDECHO value=on\nSETVAR name=CMDECHO\n",
    );
    assert_eq!(third, ["SETVAR name=CMDECHO"]);
    let fourth = echoed(&mut h, "; a comment is not echoed\n");
    assert_eq!(fourth, Vec::<String>::new());
}
