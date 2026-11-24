//! A Person's page as clients see it: `GetPerson`.

use connectrpc::{RequestContext, Response, ServiceResult};
use nama_proto::nama::v1::{self as proto, GetPersonRequest, GetPersonResponse, person_role};

use super::{BrowseServiceHandler, Fields, refused, seen_by};
use crate::{
    api::{
        caller, required_id,
        search::{movie_hit, show_hit},
        shown_images,
    },
    browse::{PersonPage, PersonTitle},
    library::{Downloaded, OnServer, ShownImages},
};

pub(super) async fn get_person(
    handler: &BrowseServiceHandler,
    ctx: &RequestContext,
    request: GetPersonRequest,
) -> ServiceResult<GetPersonResponse> {
    let id = required_id(request.person_id.into_option(), "person_id", "Person")?;
    let browse = seen_by(&handler.browse, caller(ctx)?.user.id).await?;
    let page = browse.person(&id).await;
    let page = page.map_err(refused(Fields {
        title: "person_id",
        ..Fields::NONE
    }))?;
    let images = shown_images(&handler.images, ctx).await?;
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

/// Where clients fetch `page`'s headshot, if it has one.
pub(in crate::api) fn headshot_url(page: &PersonPage, images: &ShownImages) -> Option<String> {
    images
        .downloaded(page.headshot.as_ref().map(Downloaded::stored))
        .url
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
