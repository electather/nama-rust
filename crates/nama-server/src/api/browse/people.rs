//! A Person's page as clients see it: `GetPerson`, and the People a search finds.

use connectrpc::{Response, ServiceResult};
use nama_proto::nama::v1::{self as proto, GetPersonRequest, GetPersonResponse, person_role};

use super::{BrowseServiceHandler, Fields, refused, stored_by};
use crate::{
    api::{
        database_failed, external_id_to_proto, required_id,
        search::{movie_hit, show_hit},
    },
    browse::{PersonPage, PersonTitle},
    library::{OnServer, ShownImages},
};

pub(super) async fn get_person(
    handler: &BrowseServiceHandler,
    request: GetPersonRequest,
) -> ServiceResult<GetPersonResponse> {
    let id = required_id(request.person_id.into_option(), "person_id", "Person")?;
    let page = handler.browse.person(&id).await;
    let page = page.map_err(refused(Fields {
        title: "person_id",
        ..Fields::NONE
    }))?;
    let images = handler.images.shown().await.map_err(database_failed)?;
    let titles = |titles: Vec<PersonTitle>| {
        (titles.into_iter())
            .map(|title| person_role(title, &images))
            .collect()
    };
    Response::ok(GetPersonResponse {
        headshot_url: headshot_url(&page, &images),
        name: page.name,
        cast: titles(page.cast),
        crew: titles(page.crew),
        ..Default::default()
    })
}

/// A Person a search found, as clients see them.
pub(in crate::api) fn person_hit(page: PersonPage, images: &ShownImages) -> proto::PersonHit {
    proto::PersonHit {
        headshot_url: headshot_url(&page, images),
        id: external_id_to_proto(&page.id).into(),
        name: page.name,
        ..Default::default()
    }
}

fn headshot_url(page: &PersonPage, images: &ShownImages) -> Option<String> {
    let (path, plugin) = page.headshot.as_ref()?;
    stored_by(plugin, images)(path)
}

fn person_role(title: PersonTitle, images: &ShownImages) -> proto::PersonRole {
    proto::PersonRole {
        title: Some(match title.title {
            OnServer::Movie(movie) => person_role::Title::Movie(movie_hit(movie, images).into()),
            OnServer::Show(show) => person_role::Title::Show(show_hit(show, images).into()),
        }),
        parts: title.parts,
        ..Default::default()
    }
}
