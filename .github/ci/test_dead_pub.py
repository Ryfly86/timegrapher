"""Offline tests of dead_pub.py (#1522) on a made-up workspace."""

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
pub struct Shape { pub width: u32 }
pub struct Used { pub width: u32 }
fn private_one() {}

#[derive(serde::Serialize)]
struct S {
    #[serde(skip_serializing_if = "crate::named_by_serde")]
    x: u8,
}

fn product() -> String {
    format!("{named_in_a_format}")
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
        "use super::*;\n#[test]\nfn t() { only_in_a_test_file_module(); only_in_tests(); "
        "in_a_submodule(); }\n")
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
            "Shape",                      # built only in tests
            "UNUSED_CONST",               # used only in a tests/ file
            "in_a_submodule",             # used only in a test module file
            "only_in_a_comment",          # comments and other strings' words aside
            "only_in_a_test_file_module",
            "only_in_a_test_fn",          # a #[cfg(test)] fn
            "only_in_tests",              # a #[cfg(test)] mod and tests/
        ])
        self.assertIn("crates/timegrapher-core/src/parts/mod.rs::in_a_submodule", found)

    def test_the_repository_matches_its_baseline(self):
        # The check CI runs, so a stale baseline fails here first.
        self.assertEqual(dead_pub.main(["dead_pub.py", "check"]), 0)


class Tokens(unittest.TestCase):
    def test_comments_and_raw_strings(self):
        toks = dead_pub.tokens('a /* b /* c */ d */ e // f\nr#"g "h""# \'i\' j')
        self.assertEqual([t[1] for t in toks if t[0] == "word"], ["a", "e", "j"])
        self.assertEqual([t[1] for t in toks if t[0] == "str"], ['g "h"'])


if __name__ == "__main__":
    unittest.main()
