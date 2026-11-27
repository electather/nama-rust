use super::trending_to_proto;
use crate::viewing::TrendingTitle;

#[test]
fn a_trending_movie_reaches_a_server_as_its_external_id_and_weight() {
    let trending = trending_to_proto(&TrendingTitle {
        external_id: "imdb-tt0133093".to_owned(),
        weight: 0.5,
    })
    .unwrap();

    let id = trending.id.as_option().unwrap();
    assert_eq!(
        (id.namespace.as_str(), id.value.as_str()),
        ("imdb", "tt0133093")
    );
    assert!((trending.weight - 0.5).abs() < f32::EPSILON);
}

#[test]
fn a_trending_id_without_a_namespace_is_left_out() {
    let trending = trending_to_proto(&TrendingTitle {
        external_id: "603".to_owned(),
        weight: 1.0,
    });

    assert!(trending.is_none());
}
