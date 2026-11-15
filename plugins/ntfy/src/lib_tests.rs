use super::topic;

#[test]
fn a_topic_of_letters_digits_underscores_and_dashes_up_to_64_long_is_taken() {
    let longest = "a".repeat(64);

    for taken in ["a", "nama_ada-7Xq", longest.as_str()] {
        assert_eq!(topic(Some(taken)).ok(), Some(taken));
    }
}

#[test]
fn an_empty_too_long_or_oddly_lettered_topic_is_refused_naming_the_setting() {
    let too_long = "a".repeat(65);

    for refused in [
        "",
        too_long.as_str(),
        "with space",
        "a/b",
        "ünïcode",
        "dot.ted",
    ] {
        let error = topic(Some(refused)).map(drop).unwrap_err().to_string();
        assert!(
            error.contains("Personal setting topic"),
            "{refused}: {error}"
        );
    }
}

#[test]
fn a_missing_topic_is_refused_naming_the_setting() {
    let error = topic(None).map(drop).unwrap_err().to_string();

    assert!(error.contains("Personal setting topic"), "{error}");
}
