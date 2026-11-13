use super::show_poster;

#[test]
fn a_shows_poster_is_its_first_seasons_after_the_specials_that_has_one() {
    let specials_and_first = [(0, Some("/specials.jpg")), (1, Some("/1.jpg"))];
    let first_without = [(2, Some("/2.jpg")), (1, None)];
    let specials_alone = [(0, Some("/specials.jpg"))];
    let none = [(0, None), (1, None)];

    assert_eq!(show_poster(specials_and_first), Some("/1.jpg"));
    assert_eq!(show_poster(first_without), Some("/2.jpg"));
    assert_eq!(show_poster(specials_alone), Some("/specials.jpg"));
    assert_eq!(show_poster(none), None);
}
