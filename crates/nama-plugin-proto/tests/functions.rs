//! Keeps the docs' lists of the Kernel's functions in step with `FUNCTIONS`.

use nama_plugin_proto::FUNCTIONS;

const PROTO: &str = include_str!("../../../proto/nama/plugin/v1/automation.proto");
const README: &str = include_str!("../README.md");

/// Each function's name and Permission, sorted.
fn sorted(mut listed: Vec<(String, String)>) -> Vec<(String, String)> {
    listed.sort();
    listed
}

fn table() -> Vec<(String, String)> {
    sorted(
        (FUNCTIONS.iter())
            .map(|(name, needs)| ((*name).to_owned(), format!("{needs:?}")))
            .collect(),
    )
}

#[test]
fn the_proto_comment_lists_every_function_with_its_permission_and_no_other() {
    let mut listed: Vec<(String, String)> = Vec::new();
    // A row is `//   name: Request -> Response PERMISSION`, its Permission maybe on the next line.
    for line in PROTO.lines().filter_map(|line| line.strip_prefix("//   ")) {
        for word in line.split_whitespace() {
            if let Some(name) = word.strip_suffix(':') {
                listed.push((name.to_owned(), String::new()));
            } else if word.starts_with("PERMISSION_")
                && let Some((_, needs)) = listed.last_mut()
            {
                word.clone_into(needs);
            }
        }
    }

    assert_eq!(sorted(listed), table());
}

#[test]
fn the_readmes_table_lists_every_function_with_its_permission_and_no_other() {
    let listed = (README.lines())
        .filter_map(|line| {
            let cells: Vec<_> = line
                .split('|')
                .map(|cell| cell.trim().trim_matches('`'))
                .collect();
            let [_, name, _, _, needs, _] = cells.as_slice() else {
                return None;
            };
            needs
                .starts_with("PERMISSION_")
                .then(|| ((*name).to_owned(), (*needs).to_owned()))
        })
        .collect();

    assert_eq!(sorted(listed), table());
}

#[test]
fn the_readmes_list_of_imports_names_every_function_and_no_other() {
    let sentence = (README.lines())
        .find_map(|line| line.strip_prefix("- **The Kernel's functions.** "))
        .and_then(|line| line.split(", also in the namespace").next())
        .expect("the README lists the Kernel's functions");
    let mut listed: Vec<_> = sentence.split('`').skip(1).step_by(2).collect();
    listed.sort_unstable();

    let mut names: Vec<_> = FUNCTIONS.iter().map(|(name, _)| *name).collect();
    names.sort_unstable();
    assert_eq!(listed, names);
}
