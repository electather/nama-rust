use nama_plugin_api::{
    contract::{SimilarTitle, SimilarTitles, TitleId},
    metadata::ExternalId,
};

use super::kept;

#[test]
fn an_answer_keeps_its_100_heaviest_well_formed_titles_with_clamped_weights_ties_in_order() {
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
    // A light one, a malformed one and one without a weight, then 1 to 105 at 0.5, then one at
    // 1.5, which clamps to 1.0 and comes first; the light one is cut.
    let answer = SimilarTitles {
        titles: [
            title("tmdb", 0, -0.5),
            title("no such namespace!", 1, 0.5),
            title("tmdb", 1, f32::NAN),
        ]
        .into_iter()
        .chain((1..=105).map(|value| title("tmdb", value, 0.5)))
        .chain([title("tmdb", 106, 1.5)])
        .collect(),
        ..SimilarTitles::default()
    };

    let kept = kept(answer);

    let ids: Vec<ExternalId> = kept.iter().map(|(id, _)| id.clone()).collect();
    let heaviest = std::iter::once(106).chain(1..=99);
    assert_eq!(ids, heaviest.map(ExternalId::tmdb).collect::<Vec<_>>());
    assert!(kept[0].1.total_cmp(&1.0).is_eq());
}
