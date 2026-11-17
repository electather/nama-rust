use nama_proto::nama::v1::PageRequest;

use super::Page;

fn asking(size: i32, token: &str) -> PageRequest {
    PageRequest {
        size,
        token: token.to_owned(),
        ..Default::default()
    }
}

fn size_of(size: i32) -> u32 {
    Page::<i64>::read(&asking(size, ""), b"").unwrap().size
}

#[test]
fn no_size_lists_50_and_more_than_100_lists_100() {
    assert_eq!([0, -1, 1, 100, 500].map(size_of), [50, 50, 1, 100, 100]);
}

#[test]
fn a_full_page_tells_the_next_where_to_start_and_a_short_one_ends_the_list() {
    let first = Page::<String>::read(&asking(2, ""), b"filter").unwrap();
    let mut rows = vec![("c", 3), ("b", 2), ("a", 1)];

    let page = first.finish(&mut rows, |&(key, id)| (key.to_owned(), id));
    let second = Page::<String>::read(&asking(2, &page.next_token), b"filter").unwrap();
    let mut rest = vec![("a", 1)];
    let last = second.finish(&mut rest, |&(key, id)| (key.to_owned(), id));

    assert!(first.is_first());
    assert_eq!(
        first.limit(),
        3,
        "one more than the page, to see if another follows"
    );
    assert_eq!(rows, [("c", 3), ("b", 2)]);
    assert_eq!(second.after, Some(("b".to_owned(), 2)));
    assert!(!second.is_first());
    assert_eq!(last.next_token, "", "the last page");
}

#[test]
fn a_token_read_under_another_filter_or_made_up_is_invalid() {
    let first = Page::<i64>::read(&asking(1, ""), b"pending").unwrap();
    let made = first.finish(&mut vec![2, 1], |&row| (row, row)).next_token;

    Page::<i64>::read(&asking(1, &made), b"pending").unwrap();
    for (token, filter) in [
        (made.as_str(), "all"),
        ("made up", "pending"),
        ("e30", "pending"),
    ] {
        assert!(
            Page::<i64>::read(&asking(1, token), filter.as_bytes()).is_err(),
            "{token} under {filter}"
        );
    }
}
