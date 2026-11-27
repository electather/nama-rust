use super::trending_to_proto;
use crate::{titles::ExternalId, viewing::TrendingTitle};

#[test]
fn a_trending_movie_reaches_a_server_as_its_external_id_and_weight() {
    let trending = trending_to_proto(TrendingTitle {
        id: ExternalId::parse("imdb-tt0133093").unwrap(),
        weight: 0.5,
    });

    let id = trending.id.as_option().unwrap();
    assert_eq!(
        (id.namespace.as_str(), id.value.as_str()),
        ("imdb", "tt0133093")
    );
    assert!((trending.weight - 0.5).abs() < f32::EPSILON);
}
