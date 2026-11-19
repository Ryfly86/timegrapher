//! A size that waits while a take records into its folder: the watch
//! list's and Settings' Watches Folder's, one rule and one wording.

use std::path::Path;

/// A size and its hover `what` (with its full stop): a dash, the hover
/// saying why, while the Bench records a take into `dir` (`writing`), of
/// which the library holds only a first write; else `size()` (#2034,
/// #2039; Hugh on #2037). The list and Settings both show it.
pub fn size_or_wait(
    dir: &Path,
    writing: Option<&Path>,
    what: &str,
    size: impl FnOnce() -> String,
) -> (String, String) {
    match writing.is_some_and(|w| w.starts_with(dir)) {
        true => (
            "—".into(),
            format!("{what} Its size is given once the take being recorded stops."),
        ),
        false => (size(), what.to_string()),
    }
}
