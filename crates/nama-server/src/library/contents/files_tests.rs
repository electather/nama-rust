use super::{
    super::{
        Decided, Item,
        testing::{ALIEN, Fixture, movie},
    },
    Change,
};

#[tokio::test]
async fn storing_a_changed_file_replaces_its_items_and_their_matches_go_with_them() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    let old = fx.item(ALIEN).await;
    fx.contents
        .record(vec![(old, Decided::Unmatched(None))])
        .await
        .unwrap();

    fx.store(ALIEN, 20, movie("Alien", 1979)).await;

    let items = fx.items(ALIEN).await;
    assert_eq!(
        items
            .iter()
            .map(|stored| (stored.id == old, stored.recorded.clone()))
            .collect::<Vec<_>>(),
        [(false, None)]
    );
}

#[tokio::test]
async fn removing_a_file_deletes_it_with_its_items_and_their_matches() {
    let fx = Fixture::new().await;
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    let item = fx.item(ALIEN).await;
    fx.contents
        .record(vec![(item, Decided::Unmatched(None))])
        .await
        .unwrap();
    let file = fx.contents.stored(fx.library).await.unwrap()[ALIEN].id;

    fx.contents
        .apply(fx.library, vec![Change::Remove(file)])
        .await
        .unwrap();

    assert_eq!(fx.contents.files(fx.library).await.unwrap(), []);
}

#[tokio::test]
async fn a_reparse_adds_and_removes_items_when_their_number_changes() {
    let fx = Fixture::new().await;
    let heat = "/media/Heat (1995).mkv";
    fx.store(ALIEN, 10, movie("Alien", 1979)).await;
    fx.store(heat, 10, movie("Heat", 1995)).await;
    let stored = fx.contents.stored(fx.library).await.unwrap();

    let changes = vec![
        Change::Reparse {
            file: stored[ALIEN].id,
            path: ALIEN.to_owned(),
            items: vec![movie("Alien", 1979), movie("Aliens", 1986)],
        },
        Change::Reparse {
            file: stored[heat].id,
            path: heat.to_owned(),
            items: vec![],
        },
    ];
    fx.contents.apply(fx.library, changes).await.unwrap();

    let items = fx.items(ALIEN).await;
    assert_eq!(
        items
            .iter()
            .map(|file_item| &file_item.item)
            .collect::<Vec<_>>(),
        [&movie("Alien", 1979), &movie("Aliens", 1986)]
    );
    assert_eq!(fx.items(heat).await, []);
}

#[tokio::test]
async fn a_reparse_keeps_each_episode_on_the_item_with_its_numbers() {
    let fx = Fixture::new().await;
    let dark = "/media/Dark (2017)/Season 01/Dark (2017) - S01E01-E02.mkv";
    let episode = |number| Item::Episode {
        show: "Dark".to_owned(),
        year: Some(2017),
        hint: None,
        season: 1,
        episode: number,
    };
    // The older parser read only the second Episode.
    fx.store(dark, 10, episode(2)).await;
    let second = fx.item(dark).await;

    let file = fx.contents.stored(fx.library).await.unwrap()[dark].id;
    let reparse = Change::Reparse {
        file,
        path: dark.to_owned(),
        items: vec![episode(1), episode(2)],
    };
    fx.contents.apply(fx.library, vec![reparse]).await.unwrap();

    let items = fx.items(dark).await;
    let second_now = items.iter().find(|stored| stored.item == episode(2));
    assert_eq!(second_now.map(|stored| stored.id), Some(second));
    assert_eq!(items.len(), 2);
}
