"""Offline tests of dead_pub.py (#1522) on a made-up workspace."""

import contextlib
import io
import json
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import dead_pub  # noqa: E402

CORE_LIB = '''
pub mod parts;
pub fn used_by_app() {}
pub fn only_in_tests() {}
pub fn named_in_a_format() {}
pub fn named_by_serde(_: &u8) -> bool { false }
pub fn only_on_windows() {}
pub fn only_in_a_comment() {}
pub fn only_in_a_test_file_module() {}
pub fn only_in_a_test_fn() {}
pub const UNUSED_CONST: u32 = 3;
pub(crate) fn crate_only() {}
pub const fn dead_const_fn() -> u8 { 0 }
pub fn in_an_all_test_fn() {}
pub fn only_in_a_nested_test_module() {}
pub trait DeadTrait {}
pub type DeadType = u8;
pub static DEAD_STATIC: u8 = 0;
pub union DeadUnion { a: u8 }
pub fn in_a_test_field_value() -> u32 { 1 }
pub fn after_a_test_field_value() -> u32 { 1 }

pub struct Keeper {
    pub a: u32,
    #[cfg(test)]
    pub(crate) in_step: bool,
    #[cfg(test)]
    map: std::collections::HashMap<u8, OnlyInATestFieldType>,
}

pub struct AfterAField;
pub struct OnlyInATestFieldType;
pub struct Shape { pub width: u32 }
pub struct Used { pub width: u32 }
fn private_one() {}

#[derive(serde::Serialize)]
struct S {
    #[serde(skip_serializing_if = "crate::named_by_serde")]
    x: u8,
}

fn product() -> String {
    let _ = Used { height: 2, #[cfg(test)] width: in_a_test_field_value() };
    let _ = after_a_test_field_value();
    format!("{named_in_a_format}")
}

#[cfg(all(test, unix))]
fn unix_helper() {
    in_an_all_test_fn();
}

// only_in_a_comment is named here, which doesn't count.
/* nor here: only_in_a_comment */

#[cfg(test)]
fn helper() {
    only_in_a_test_fn();
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;

#[cfg(test)]
mod tests {
    #[test]
    fn t() {
        super::only_in_tests();
        let s = super::Shape { width: 1 };
    }
}
'''

APP_MAIN = '''
fn main() {
    timegrapher_core::used_by_app();
    let raw = r#"a "quoted" string"#;
    let _ = timegrapher_core::Used { width: 1 };
    #[cfg(windows)]
    timegrapher_core::only_on_windows();
}
'''


def workspace(tmp):
    root = pathlib.Path(tmp)
    core = root / "crates/timegrapher-core"
    (core / "src/parts").mkdir(parents=True)
    (core / "tests").mkdir()
    (root / "crates/timegrapher-app/src").mkdir(parents=True)
    (core / "src/lib.rs").write_text(CORE_LIB)
    (core / "src/parts/mod.rs").write_text("pub fn in_a_submodule() {}\n")
    (core / "src/lib_tests.rs").write_text(
        "use super::*;\nmod more;\n#[test]\nfn t() { only_in_a_test_file_module(); "
        "only_in_tests(); in_a_submodule(); }\n")
    (core / "src/lib_tests").mkdir()
    (core / "src/lib_tests/more.rs").write_text(
        "#[test]\nfn t() { crate::only_in_a_nested_test_module(); }\n")
    (core / "tests/it.rs").write_text("#[test]\nfn t() { timegrapher_core::only_in_tests(); "
                                      "let _ = timegrapher_core::UNUSED_CONST; }\n")
    (root / "crates/timegrapher-app/src/main.rs").write_text(APP_MAIN)
    return root


class Scan(unittest.TestCase):
    def test_what_counts_as_used(self):
        with tempfile.TemporaryDirectory() as tmp:
            found = dead_pub.scan(workspace(tmp))
        names = sorted(f.split("::")[1] for f in found)
        self.assertEqual(names, [
            "AfterAField",                # after a #[cfg(test)] field, still checked
            "DEAD_STATIC",                # every item kind
            "DeadTrait",
            "DeadType",
            "DeadUnion",
            "Keeper",
            "OnlyInATestFieldType",       # in a #[cfg(test)] field's type
            "Shape",                      # built only in tests
            "UNUSED_CONST",               # used only in a tests/ file
            "dead_const_fn",              # a `const fn`, not an item named `fn`
            "in_a_submodule",             # used only in a test module file
            "in_a_test_field_value",      # a #[cfg(test)] struct-literal field
            "in_an_all_test_fn",          # #[cfg(all(test, ...))]
            "only_in_a_comment",          # comments and other strings' words aside
            "only_in_a_nested_test_module",  # a module a test module declares
            "only_in_a_test_file_module",
            "only_in_a_test_fn",          # a #[cfg(test)] fn
            "only_in_tests",              # a #[cfg(test)] mod and tests/
        ])
        self.assertIn("crates/timegrapher-core/src/parts/mod.rs::in_a_submodule", found)

    def test_the_repository_matches_its_baseline(self):
        # The check CI runs, so a stale baseline fails here first.
        self.assertEqual(dead_pub.main(["dead_pub.py", "check"]), 0)


class Check(unittest.TestCase):
    """`check` fails on a new item, a gone one and a grown baseline."""

    def run_check(self, baseline, base=None):
        with tempfile.TemporaryDirectory() as tmp:
            root = workspace(tmp)
            found = dead_pub.scan(root)
            path = root / "baseline.json"
            path.write_text(json.dumps(baseline(found)))
            with contextlib.redirect_stdout(io.StringIO()) as out:
                code = dead_pub.main(["dead_pub.py", "check"], root, path,
                                     None if base is None else base(found))
            return code, out.getvalue()

    def test_the_same_list_passes(self):
        self.assertEqual(self.run_check(lambda f: f)[0], 0)
        self.assertEqual(self.run_check(lambda f: f, lambda f: f + ["x::y"])[0], 0)

    def test_a_new_dead_item_fails(self):
        code, out = self.run_check(lambda f: f[1:])
        self.assertEqual(code, 1)
        self.assertIn("nothing outside tests uses it", out)

    def test_a_gone_item_fails(self):
        code, out = self.run_check(lambda f: f + ["crates/x.rs::gone"])
        self.assertEqual(code, 1)
        self.assertIn("is no longer dead", out)

    def test_a_grown_baseline_fails(self):
        # Dead code added with `update` in the same PR.
        code, out = self.run_check(lambda f: f, lambda f: f[1:])
        self.assertEqual(code, 1)
        self.assertIn("may only shrink", out)


class Tokens(unittest.TestCase):
    def test_comments_and_raw_strings(self):
        toks = dead_pub.tokens('a /* b /* c */ d */ e // f\nr#"g "h""# \'i\' j')
        self.assertEqual([t[1] for t in toks if t[0] == "word"], ["a", "e", "j"])
        self.assertEqual([t[1] for t in toks if t[0] == "str"], ['g "h"'])


if __name__ == "__main__":
    unittest.main()
