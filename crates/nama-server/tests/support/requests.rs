//! Asking for Requests as a Requester, and approving, retrying and declining them as a Request
//! manager. A plain name fails the test on an error; a `try_` twin, there for tests that expect a
//! Refusal, returns the `Result`, and an `_at` twin names a Quality by id.

use connectrpc::{ConnectError, client::HttpClient};
use nama_proto::nama::v1::{
    ApproveRequestRequest, CreateFollowedShowRequest, CreateRequestRequest,
    CreateSeasonRequestsRequest, DeclineRequestRequest, FollowedShow, GetRequestRequest,
    ListRequestsRequest, Request, RequestServiceClient, RequestState, RetryRequestRequest,
    WithdrawRequestRequest,
};

use super::{SignedIn, TestServer};

/// What a Requester does, called on the Request service as them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
pub(crate) trait Requester {
    /// The Request service this Requester calls.
    fn service(&self) -> &RequestServiceClient<HttpClient>;

    /// Asks for the Movie `tmdb_id`, and returns the Request as asking left it.
    async fn ask(&self, tmdb_id: i64) -> Request {
        create_request(self.service(), tmdb_id, None).await.unwrap()
    }

    /// [`Self::ask`] at the Quality `quality`.
    async fn ask_at(&self, tmdb_id: i64, quality: i64) -> Request {
        self.try_ask_at(tmdb_id, quality).await.unwrap()
    }

    /// [`Self::ask_at`], returning its error, such as a Refusal.
    async fn try_ask_at(&self, tmdb_id: i64, quality: i64) -> Result<Request, ConnectError> {
        create_request(self.service(), tmdb_id, Some(quality)).await
    }

    /// Asks for the Seasons `seasons` of the Show `show`, and returns their Requests as asking
    /// left them.
    async fn ask_seasons(&self, show: i64, seasons: &[u32]) -> Vec<Request> {
        create_season_requests(self.service(), show, seasons, None)
            .await
            .unwrap()
    }

    /// [`Self::ask_seasons`] at the Quality `quality`.
    async fn ask_seasons_at(&self, show: i64, seasons: &[u32], quality: i64) -> Vec<Request> {
        self.try_ask_seasons_at(show, seasons, quality)
            .await
            .unwrap()
    }

    /// [`Self::ask_seasons_at`], returning its error, such as a Refusal.
    async fn try_ask_seasons_at(
        &self,
        show: i64,
        seasons: &[u32],
        quality: i64,
    ) -> Result<Vec<Request>, ConnectError> {
        create_season_requests(self.service(), show, seasons, Some(quality)).await
    }

    /// Asks for the whole Show `show`, and returns the Followed Show and its Season Requests.
    async fn follow(&self, show: i64) -> (FollowedShow, Vec<Request>) {
        create_followed_show(self.service(), show, None)
            .await
            .unwrap()
    }

    /// [`Self::follow`] at the Quality `quality`.
    async fn follow_at(&self, show: i64, quality: i64) -> (FollowedShow, Vec<Request>) {
        self.try_follow_at(show, quality).await.unwrap()
    }

    /// [`Self::follow_at`], returning its error, such as a Refusal.
    async fn try_follow_at(
        &self,
        show: i64,
        quality: i64,
    ) -> Result<(FollowedShow, Vec<Request>), ConnectError> {
        create_followed_show(self.service(), show, Some(quality)).await
    }

    /// The Requests this Requester may see.
    async fn list(&self) -> Vec<Request> {
        self.list_in(RequestState::Unspecified).await.0
    }

    /// The Requests and Followed Shows in `state` this Requester may see, or all of them for
    /// `RequestState::Unspecified`.
    async fn list_in(&self, state: RequestState) -> (Vec<Request>, Vec<FollowedShow>) {
        let listed = self
            .service()
            .list_requests(ListRequestsRequest {
                state: state.into(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned();
        (listed.requests, listed.followed_shows)
    }

    /// The Request `id`.
    async fn get(&self, id: i64) -> Request {
        let got = self
            .service()
            .get_request(GetRequestRequest {
                id,
                ..Default::default()
            })
            .await
            .unwrap();
        got.into_owned().request.into_option().unwrap()
    }

    /// Withdraws this Requester from the Request `id`.
    async fn withdraw(&self, id: i64) {
        self.try_withdraw(id).await.unwrap();
    }

    /// [`Self::withdraw`], returning its error, such as a Refusal.
    async fn try_withdraw(&self, id: i64) -> Result<(), ConnectError> {
        self.service()
            .withdraw_request(WithdrawRequestRequest {
                id,
                ..Default::default()
            })
            .await
            .map(drop)
    }
}

impl Requester for RequestServiceClient<HttpClient> {
    fn service(&self) -> &Self {
        self
    }
}

/// Request management as the Admin, or as another Request manager with [`Self::approve_as`]. Each
/// returns the Request as the change left it: an approval or retry returns once the Hand-off ran
/// (ADR 0029), so nothing waits for it.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl TestServer {
    /// The Request service as `who` calls it, to ask as a [`Requester`].
    pub(crate) fn requests(&self, who: &SignedIn) -> RequestServiceClient<HttpClient> {
        RequestServiceClient::new(HttpClient::plaintext(), self.client_config_as(&who.token))
    }

    /// Approves the Request `id` as the Admin.
    pub(crate) async fn approve(&self, id: i64) -> Request {
        approve_request(&self.admin_requests(), id, None)
            .await
            .unwrap()
    }

    /// [`Self::approve`], changing its Quality to `quality`.
    pub(crate) async fn approve_at(&self, id: i64, quality: i64) -> Request {
        self.try_approve_at(id, quality).await.unwrap()
    }

    /// [`Self::approve_at`], returning its error, such as a Refusal.
    pub(crate) async fn try_approve_at(
        &self,
        id: i64,
        quality: i64,
    ) -> Result<Request, ConnectError> {
        approve_request(&self.admin_requests(), id, Some(quality)).await
    }

    /// Approves the Request `id` as the Request manager `who`.
    pub(crate) async fn approve_as(&self, who: &SignedIn, id: i64) -> Request {
        approve_request(&self.requests(who), id, None)
            .await
            .unwrap()
    }

    /// [`Self::approve_as`], changing its Quality to `quality`.
    pub(crate) async fn approve_as_at(&self, who: &SignedIn, id: i64, quality: i64) -> Request {
        approve_request(&self.requests(who), id, Some(quality))
            .await
            .unwrap()
    }

    /// Retries the Failed Request `id` as the Admin.
    pub(crate) async fn retry(&self, id: i64) -> Request {
        self.try_retry(id).await.unwrap()
    }

    /// [`Self::retry`], returning its error, such as a Refusal.
    pub(crate) async fn try_retry(&self, id: i64) -> Result<Request, ConnectError> {
        retry_request(&self.admin_requests(), id, None).await
    }

    /// [`Self::retry`], changing its Quality to `quality`.
    pub(crate) async fn retry_at(&self, id: i64, quality: i64) -> Request {
        self.try_retry_at(id, quality).await.unwrap()
    }

    /// [`Self::retry_at`], returning its error, such as a Refusal.
    pub(crate) async fn try_retry_at(
        &self,
        id: i64,
        quality: i64,
    ) -> Result<Request, ConnectError> {
        retry_request(&self.admin_requests(), id, Some(quality)).await
    }

    /// Declines the Request `id` as the Admin, giving `reason`.
    pub(crate) async fn decline(&self, id: i64, reason: &str) -> Request {
        let declined = self
            .admin_requests()
            .decline_request(DeclineRequestRequest {
                id,
                reason: reason.to_owned(),
                ..Default::default()
            })
            .await
            .unwrap();
        declined.into_owned().request.into_option().unwrap()
    }

    /// The Request service as the Admin calls it, to ask as a [`Requester`] or call an RPC
    /// under test.
    pub(crate) fn admin_requests(&self) -> RequestServiceClient<HttpClient> {
        RequestServiceClient::new(HttpClient::plaintext(), self.client_config())
    }
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn create_request(
    service: &RequestServiceClient<HttpClient>,
    tmdb_id: i64,
    quality: Option<i64>,
) -> Result<Request, ConnectError> {
    let asked = service
        .create_request(CreateRequestRequest {
            movie_tmdb_id: tmdb_id,
            quality,
            ..Default::default()
        })
        .await?;
    Ok(asked.into_owned().request.into_option().unwrap())
}

async fn create_season_requests(
    service: &RequestServiceClient<HttpClient>,
    show: i64,
    seasons: &[u32],
    quality: Option<i64>,
) -> Result<Vec<Request>, ConnectError> {
    let asked = service
        .create_season_requests(CreateSeasonRequestsRequest {
            show_tmdb_id: show,
            season_numbers: seasons.to_vec(),
            quality,
            ..Default::default()
        })
        .await?;
    Ok(asked.into_owned().requests)
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn create_followed_show(
    service: &RequestServiceClient<HttpClient>,
    show: i64,
    quality: Option<i64>,
) -> Result<(FollowedShow, Vec<Request>), ConnectError> {
    let followed = service
        .create_followed_show(CreateFollowedShowRequest {
            show_tmdb_id: show,
            quality,
            ..Default::default()
        })
        .await?
        .into_owned();
    Ok((
        followed.followed_show.into_option().unwrap(),
        followed.requests,
    ))
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn approve_request(
    service: &RequestServiceClient<HttpClient>,
    id: i64,
    quality: Option<i64>,
) -> Result<Request, ConnectError> {
    let approved = service
        .approve_request(ApproveRequestRequest {
            id,
            quality_id: quality,
            ..Default::default()
        })
        .await?;
    Ok(approved.into_owned().request.into_option().unwrap())
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn retry_request(
    service: &RequestServiceClient<HttpClient>,
    id: i64,
    quality: Option<i64>,
) -> Result<Request, ConnectError> {
    let retried = service
        .retry_request(RetryRequestRequest {
            id,
            quality_id: quality,
            ..Default::default()
        })
        .await?;
    Ok(retried.into_owned().request.into_option().unwrap())
}
