use super::edit_distance;

#[test]
fn edit_distance_counts_insertions_deletions_and_replacements() {
    assert_eq!(edit_distance("money", "money"), 0);
    assert_eq!(edit_distance("mony", "money"), 1);
    assert_eq!(edit_distance("money", "mony"), 1);
    assert_eq!(edit_distance("honey", "money"), 1);
    assert_eq!(edit_distance("kitten", "sitting"), 3);
    assert_eq!(edit_distance("", "abc"), 3);
}

#[test]
fn edit_distance_counts_characters_not_bytes() {
    assert_eq!(edit_distance("amélie", "amelie"), 1);
}
