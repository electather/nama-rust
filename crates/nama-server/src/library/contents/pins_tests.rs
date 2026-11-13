use nama_plugin_api::metadata::{ExternalId, IMDB};

use super::{
    super::{
        Change, Decided, Item,
        testing::{ALIEN, Fixture, movie},
    },
    Pin,
};
use crate::libraries::Kind;

#[tokio::test]
async fn a_pin_still_applies_after_its_items_are_replaced() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    let named = fx
        .contents
        .named_like(fx.item(ALIEN).await)
        .await
        .unwrap()
        .unwrap();
    let pin = Pin::Title(ExternalId::Tmdb(949));
    fx.contents
        .pin(named.library, named.name, pin.clone(), vec![])
        .await
        .unwrap();

    fx.store(ALIEN, 20, movie("Alien", 1979)).await;

    let undecided = fx.contents.undecided(fx.library).await.unwrap();
    assert_eq!(
        undecided
            .into_iter()
            .map(|(_, _, pinned)| pinned)
            .collect::<Vec<_>>(),
        [Some(pin)]
    );
}

#[tokio::test]
async fn a_pin_still_applies_after_a_reparse_gives_its_items_a_new_name() {
    let fx = Fixture::new().await;
    let heat = "/media/Heat (1995) {tmdb-949}/Heat (1995).mkv";
    let knight = "/media/A Knight of the Seven Kingdoms (2026) [tvdbid-433631]/Season 01/\
                  A Knight of the Seven Kingdoms - S01E01.mkv";
    let knight_pilot = |hint| Item::Episode {
        show: "A Knight of the Seven Kingdoms".to_owned(),
        year: Some(2026),
        hint,
        season: 1,
        episode: 1,
    };
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    fx.store(heat, 10, movie("Heat", 1995)).await;
    fx.store(knight, 10, knight_pilot(None)).await;
    let (alien_item, heat_item) = (fx.item(ALIEN).await, fx.item(heat).await);
    let knight_item = fx.item(knight).await;
    let pins = [
        (alien_item, Pin::Title(ExternalId::Tmdb(348))),
        (heat_item, Pin::Unmatched),
        (knight_item, Pin::Title(ExternalId::Tmdb(224_372))),
    ];
    for (item, pin) in pins.clone() {
        let named = fx.contents.named_like(item).await.unwrap().unwrap();
        let decided = vec![(item, Decided::Unmatched(None))];
        fx.contents
            .pin(named.library, named.name, pin, decided)
            .await
            .unwrap();
    }

    // A newer parser reads ID Hints the old one missed.
    let hinted = |title: &str, year, hint| Item::Movie {
        title: title.to_owned(),
        year: Some(year),
        hint: Some(hint),
    };
    let stored = fx.contents.stored(fx.library).await.unwrap();
    let reparse = |path: &str, item| Change::Reparse {
        file: stored[path].id,
        path: path.to_owned(),
        items: vec![item],
    };
    let changes = vec![
        reparse(
            ALIEN,
            hinted("Alien", 1979, ExternalId::new(IMDB, "tt0078748").unwrap()),
        ),
        reparse(heat, hinted("Heat", 1995, ExternalId::Tmdb(949))),
        reparse(knight, knight_pilot(Some(ExternalId::Tvdb(433_631)))),
    ];
    fx.contents.apply(fx.library, changes).await.unwrap();

    let undecided = fx.contents.undecided(fx.library).await.unwrap();
    assert_eq!(
        undecided
            .into_iter()
            .map(|(id, item, pin)| (id, item.name().hint, pin))
            .collect::<Vec<_>>(),
        [
            (
                alien_item,
                Some(ExternalId::new(IMDB, "tt0078748").unwrap()),
                Some(pins[0].1.clone())
            ),
            (heat_item, Some(ExternalId::Tmdb(949)), Some(Pin::Unmatched)),
            (
                knight_item,
                Some(ExternalId::Tvdb(433_631)),
                Some(pins[2].1.clone())
            ),
        ]
    );
}

#[tokio::test]
async fn pin_refuses_and_stores_no_decisions_when_the_library_is_gone() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    let item = fx.item(ALIEN).await;
    let named = fx.contents.named_like(item).await.unwrap().unwrap();
    // An Item of another Library, which outlives the deleted one, so a decision stored
    // despite the refusal would show.
    let elsewhere = tempfile::tempdir().unwrap();
    let root = elsewhere.path().canonicalize().unwrap();
    let other = fx
        .libraries
        .create(
            "Other Movies",
            Kind::Movies,
            vec![root.to_str().unwrap().to_owned()],
        )
        .await
        .unwrap()
        .id;
    let store_heat = Change::Store {
        path: root.join("Heat (1995).mkv").to_str().unwrap().to_owned(),
        size: 10,
        modified_ns: 0,
        items: vec![movie("Heat", 1995)],
    };
    fx.contents.apply(other, vec![store_heat]).await.unwrap();
    let heat = fx.contents.files(other).await.unwrap()[0].items[0].id;
    fx.libraries.delete(fx.library).await.unwrap();

    let pinned = fx
        .contents
        .pin(
            named.library,
            named.name,
            Pin::Unmatched,
            vec![
                (item, Decided::Unmatched(None)),
                (heat, Decided::Unmatched(None)),
            ],
        )
        .await;

    assert!(matches!(pinned, Ok(None)), "{pinned:?}");
    let undecided: Vec<i64> = fx
        .contents
        .undecided(other)
        .await
        .unwrap()
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    assert_eq!(undecided, [heat]);
}
