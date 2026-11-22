//! The help a newcomer reads first (#2099): `timegrapher --help` ends with
//! example commands, every argument of every command says what it is, and
//! the developer commands come last, marked as such.
use std::process::Command;

/// `timegrapher <words> --help`.
fn help(words: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_timegrapher"))
        .args(words)
        .arg("--help")
        .output()
        .expect("run timegrapher");
    assert!(out.status.success(), "{words:?}");
    String::from_utf8(out.stdout).expect("utf-8 help")
}

/// The lines of a help section ("Commands:", "Arguments:"), up to the next.
fn section<'a>(text: &'a str, title: &str) -> Vec<&'a str> {
    text.lines()
        .skip_while(|l| *l != title)
        .skip(1)
        .take_while(|l| l.is_empty() || l.starts_with(' '))
        .filter(|l| !l.trim().is_empty())
        .collect()
}

/// The names in a "Commands:" section, `help` left out.
fn commands(text: &str) -> Vec<String> {
    section(text, "Commands:")
        .iter()
        .filter_map(|l| l.split_whitespace().next())
        .filter(|n| *n != "help")
        .map(str::to_string)
        .collect()
}

#[test]
fn the_help_ends_with_examples() {
    let text = help(&[]);
    let tail: Vec<&str> = text.trim_end().lines().rev().take(4).collect();
    assert_eq!(tail[3], "Examples:", "{text}");
    for (line, command) in tail[..3].iter().rev().zip([
        "timegrapher analyze my_take/take_part01-001.flac",
        "timegrapher session my_take",
        "timegrapher doctor",
    ]) {
        assert!(line.trim_start().starts_with(command), "{line}");
    }
}

#[test]
fn the_developer_commands_come_last_and_say_so() {
    let text = help(&[]);
    let names = commands(&text);
    assert_eq!(names[names.len() - 2..], ["regress", "synth"], "{names:?}");
    for name in ["regress", "synth"] {
        let line = section(&text, "Commands:")
            .into_iter()
            .find(|l| l.split_whitespace().next() == Some(name))
            .unwrap();
        assert!(line.contains("For developers:"), "{line}");
    }
}

/// Every positional argument of every command and subcommand has a
/// description beside it.
#[test]
fn every_argument_is_described() {
    let mut todo: Vec<Vec<String>> = vec![Vec::new()];
    let mut seen = 0;
    while let Some(words) = todo.pop() {
        let args: Vec<&str> = words.iter().map(String::as_str).collect();
        let text = help(&args);
        for line in section(&text, "Arguments:") {
            seen += 1;
            let mut parts = line.split_whitespace();
            let name = parts.next().unwrap();
            assert!(parts.next().is_some(), "{args:?} {name} has no description");
        }
        for name in commands(&text) {
            let mut next = words.clone();
            next.push(name);
            todo.push(next);
        }
    }
    assert!(seen > 20, "only {seen} arguments found");
}

#[test]
fn analyze_takes_one_sound_file() {
    let text = help(&["analyze"]);
    let line = section(&text, "Arguments:")[0];
    assert!(line.contains("one sound file"), "{line}");
}
