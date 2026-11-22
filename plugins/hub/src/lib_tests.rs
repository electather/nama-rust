use std::{
    collections::HashMap,
    future,
    sync::{Arc, Mutex},
};

use buffa::Message as _;
use nama_plugin_api::{
    BoxFuture, Registrar, Registration,
    automation::{
        Kernel, KernelFunction as _, KernelRefused,
        functions::{StoreDelete, StoreGet, StorePut},
    },
    contract::{
        SimilarAsked, SimilarTitle, SimilarTitles, StoreEntry, StoreKey, StoreValue, Stored,
        TitleId, TitleKind,
    },
    server_link::{ServerLink, ServerLinker},
    similar_source::SimilarSource,
};
use serde_json::json;

use super::*;
use crate::fake_hub::{EMAIL, FakeHub, USER_CODE};

/// A Kernel whose only functions are the Plugin's Store, kept in memory.
fn store() -> Arc<dyn Kernel> {
    let entries: Arc<Mutex<HashMap<String, Vec<u8>>>> = Arc::default();
    Arc::new(
        move |function: &str,
              request: Vec<u8>|
              -> BoxFuture<'static, Result<Vec<u8>, KernelRefused>> {
            let mut entries = entries.lock().unwrap();
            let answer = if function == StoreGet::NAME {
                let key = StoreKey::decode_from_slice(&request).unwrap();
                StoreValue {
                    value: entries.get(&key.key).cloned(),
                    ..Default::default()
                }
                .encode_to_vec()
            } else if function == StorePut::NAME {
                let entry = StoreEntry::decode_from_slice(&request).unwrap();
                entries.insert(entry.key, entry.value);
                Stored::default().encode_to_vec()
            } else if function == StoreDelete::NAME {
                let key = StoreKey::decode_from_slice(&request).unwrap();
                entries.remove(&key.key);
                Stored::default().encode_to_vec()
            } else {
                return Box::pin(future::ready(Err(KernelRefused(function.to_owned()))));
            };
            Box::pin(future::ready(Ok(answer)))
        },
    )
}

/// The Plugin enabled against `hub`, keeping its Store in `kernel`.
fn enabled(hub: &FakeHub, kernel: &Arc<dyn Kernel>) -> Arc<dyn ServerLinker> {
    provided(hub, kernel)
}

/// What the Plugin enabled against `hub`, keeping its Store in `kernel`, provides as `T`.
fn provided<T: ?Sized + Send + Sync + 'static>(hub: &FakeHub, kernel: &Arc<dyn Kernel>) -> Arc<T> {
    let mut registrar = Registrar::new().with_kernel(Arc::clone(kernel));
    Hub::at(hub.url()).register(&mut registrar).unwrap();
    (registrar.into_registrations().iter())
        .find_map(Registration::provider::<T>)
        .unwrap()
}

/// What a Similar source is asked about a Movie known by `ids`.
fn movie(ids: &[(&str, &str)]) -> SimilarAsked {
    SimilarAsked {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        ids: ids
            .iter()
            .map(|&(namespace, value)| TitleId {
                namespace: namespace.to_owned(),
                value: value.to_owned(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

/// `similar`'s titles, each as its id's namespace and value and its weight.
fn answered(similar: &SimilarTitles) -> Vec<(&str, &str, f32)> {
    (similar.titles.iter())
        .map(|title: &SimilarTitle| {
            (
                title.id.namespace.as_str(),
                title.id.value.as_str(),
                title.weight,
            )
        })
        .collect()
}

/// Links the Server through `linker` to [`EMAIL`]'s Account on `hub`.
async fn link(hub: &FakeHub, linker: &dyn ServerLinker) {
    linker.start_link("Living room").await.unwrap();
    hub.enter_code();
    assert_eq!(
        linker.link().await.unwrap(),
        ServerLink::Linked {
            account: EMAIL.to_owned()
        }
    );
}

#[tokio::test]
async fn the_server_shows_a_code_and_is_linked_once_someone_enters_it() {
    let hub = FakeHub::start().await;
    let kernel = store();
    let linker = enabled(&hub, &kernel);

    let started = linker.start_link("Living room").await.unwrap();

    let waiting = ServerLink::Waiting {
        code: USER_CODE.to_owned(),
        url: hub.url(),
    };
    assert_eq!(started, waiting);
    assert_eq!(hub.starting().as_deref(), Some("Living room"));
    assert_eq!(linker.link().await.unwrap(), waiting);
    hub.enter_code();
    let linked_to_ada = ServerLink::Linked {
        account: EMAIL.to_owned(),
    };
    assert_eq!(linker.link().await.unwrap(), linked_to_ada);
    assert!(hub.linked());
    // Enabled again, such as after a restart, it is still Linked.
    assert_eq!(enabled(&hub, &kernel).link().await.unwrap(), linked_to_ada);
}

#[tokio::test]
async fn a_server_that_isnt_linked_asks_the_hub_nothing() {
    let hub = FakeHub::start().await;
    hub.stop();

    let link = enabled(&hub, &store()).link().await.unwrap();

    assert_eq!(link, ServerLink::NotLinked);
}

#[tokio::test]
async fn the_code_keeps_showing_while_the_hub_is_unreachable() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    let started = linker.start_link("Living room").await.unwrap();
    hub.stop();

    let link = linker.link().await.unwrap();

    assert_eq!(link, started);
}

#[tokio::test]
async fn a_linked_server_cant_start_linking_again() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;
    let refused = linker.start_link("Living room").await.unwrap_err();

    assert!(refused.to_string().contains("Unlink it first"), "{refused}");
}

#[tokio::test]
async fn unlinking_revokes_the_servers_credential_on_the_hub() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;

    linker.unlink().await.unwrap();

    assert!(!hub.linked());
    assert_eq!(linker.link().await.unwrap(), ServerLink::NotLinked);
}

#[tokio::test]
async fn unlinking_while_the_hub_is_unreachable_keeps_the_link() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    link(&hub, linker.as_ref()).await;
    hub.stop();

    linker.unlink().await.unwrap_err();

    assert!(matches!(
        linker.link().await.unwrap(),
        ServerLink::Linked { .. }
    ));
}

#[tokio::test]
async fn a_credential_whose_token_cant_be_checked_is_kept_and_can_be_unlinked() {
    let hub = FakeHub::start().await;
    let linker = enabled(&hub, &store());
    linker.start_link("Living room").await.unwrap();
    hub.enter_code();
    hub.break_key_set();

    linker.link().await.unwrap_err();

    assert_eq!(
        linker.link().await.unwrap(),
        ServerLink::Linked {
            account: "an Account the Hub didn't confirm".to_owned()
        }
    );
    linker.unlink().await.unwrap();
    assert!(!hub.linked());
}

#[tokio::test]
async fn a_hub_signed_token_verifies_against_the_kept_key_set_while_the_hub_is_unreachable() {
    let hub = FakeHub::start().await;
    let kernel = store();
    link(&hub, enabled(&hub, &kernel).as_ref()).await;
    assert_eq!(hub.key_set_fetches(), 1);
    let token = hub.sign(&json!({ "sub": "server", "email": "bea@example.com", "exp": u64::MAX }));
    hub.stop();

    let linker = Linker::new(HubApi::new(&hub.url()).unwrap(), kernel);
    let claims = linker.verify(&token).await.unwrap();

    assert_eq!(claims.email, "bea@example.com");
    assert_eq!(hub.key_set_fetches(), 1);
}

#[test]
fn without_the_hubs_address_the_plugin_refuses_to_start() {
    let mut registrar = Registrar::new().with_kernel(store());

    let refused = Hub { url: None }.register(&mut registrar).unwrap_err();

    assert!(refused.to_string().contains("NAMA_HUB_URL"), "{refused}");
}

#[tokio::test]
async fn a_linked_server_gets_the_titles_the_hub_finds_alike() {
    let hub = FakeHub::start().await;
    let kernel = store();
    link(&hub, enabled(&hub, &kernel).as_ref()).await;
    let similar = provided::<dyn SimilarSource>(&hub, &kernel);

    let the_matrix = similar
        .similar(&movie(&[("imdb", "tt0133093"), ("tmdb", "603")]))
        .await
        .unwrap();

    assert_eq!(
        answered(&the_matrix),
        [("imdb", "tt0234215", 0.9), ("imdb", "tt0078748", 0.35)]
    );
}

#[tokio::test]
async fn a_title_the_hub_doesnt_know_gets_no_similar_titles() {
    let hub = FakeHub::start().await;
    let kernel = store();
    link(&hub, enabled(&hub, &kernel).as_ref()).await;

    let unknown = provided::<dyn SimilarSource>(&hub, &kernel)
        .similar(&movie(&[("tmdb", "1")]))
        .await;

    assert_eq!(answered(&unknown.unwrap()), []);
}

#[tokio::test]
async fn a_server_that_isnt_linked_gets_no_similar_titles_and_asks_the_hub_nothing() {
    let hub = FakeHub::start().await;
    hub.stop();

    let similar = provided::<dyn SimilarSource>(&hub, &store())
        .similar(&movie(&[("tmdb", "603")]))
        .await;

    assert_eq!(answered(&similar.unwrap()), []);
}

#[tokio::test]
async fn a_server_whose_credential_the_hub_forgot_gets_no_similar_titles() {
    let hub = FakeHub::start().await;
    let kernel = store();
    link(&hub, enabled(&hub, &kernel).as_ref()).await;
    hub.forget_server();

    let similar = provided::<dyn SimilarSource>(&hub, &kernel)
        .similar(&movie(&[("tmdb", "603")]))
        .await;

    assert_eq!(answered(&similar.unwrap()), []);
}

#[tokio::test]
async fn similar_titles_fail_while_the_hub_is_unreachable_so_the_last_answer_stays() {
    let hub = FakeHub::start().await;
    let kernel = store();
    link(&hub, enabled(&hub, &kernel).as_ref()).await;
    hub.stop();

    let similar = provided::<dyn SimilarSource>(&hub, &kernel)
        .similar(&movie(&[("tmdb", "603")]))
        .await;

    similar.unwrap_err();
}
