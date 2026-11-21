use nama_plugin_api::{
    contract::{SimilarTitle, SimilarTitles, TitleId},
    metadata::ExternalId,
};

use super::kept;

#[test]
fn an_answer_keeps_its_first_20_well_formed_titles_in_order_with_clamped_weights() {
    let title = |namespace: &str, value: i64, weight: f32| SimilarTitle {
        id: TitleId {
            namespace: namespace.to_owned(),
            value: value.to_string(),
            ..TitleId::default()
        }
        .into(),
        weight,
        ..SimilarTitle::default()
    };
    let malformed = title("no such namespace!", 1, 0.5);
    let answer = SimilarTitles {
        titles: [malformed, title("tmdb", 1, f32::NAN)]
            .into_iter()
            .chain((1..=25).map(|value| title("tmdb", value, 1.5)))
            .collect(),
        ..SimilarTitles::default()
    };

    let kept = kept(answer);

    let ids: Vec<ExternalId> = kept.iter().map(|(id, _)| id.clone()).collect();
    assert_eq!(ids, (1..=20).map(ExternalId::tmdb).collect::<Vec<_>>());
    assert!(
        kept.iter()
            .all(|(_, weight)| weight.total_cmp(&1.0).is_eq())
    );
}
