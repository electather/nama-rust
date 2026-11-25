//! `list_hub_similar`, `list_hub_recommended`, `list_hub_trending` and
//! `list_hub_most_watchlisted`: what the Hub suggests to the Linked Server (ADR 0105), which
//! `Linking` asks for.

use nama_plugin_api::{
    automation::KernelRefused,
    contract::{HubChartAsked, HubRecommendedAsked, HubTitles, SimilarAsked, SimilarTitles},
};

use super::Caller;
use crate::link::{self, Linking};

impl Caller {
    pub(super) async fn list_hub_similar(
        self,
        asked: SimilarAsked,
    ) -> Result<SimilarTitles, KernelRefused> {
        self.linking()
            .similar_titles(&asked)
            .await
            .map_err(hub_unreachable)
    }

    pub(super) async fn list_hub_recommended(
        self,
        asked: HubRecommendedAsked,
    ) -> Result<HubTitles, KernelRefused> {
        (self.linking().recommended_titles(asked).await).map_err(hub_unreachable)
    }

    pub(super) async fn list_hub_trending(
        self,
        _asked: HubChartAsked,
    ) -> Result<HubTitles, KernelRefused> {
        self.linking()
            .trending_titles()
            .await
            .map_err(hub_unreachable)
    }

    pub(super) async fn list_hub_most_watchlisted(
        self,
        _asked: HubChartAsked,
    ) -> Result<HubTitles, KernelRefused> {
        (self.linking().most_watchlisted_titles().await).map_err(hub_unreachable)
    }

    fn linking(&self) -> &Linking {
        self.automation.users.linking()
    }
}

/// The refusal of a Hub call that failed, which `Linking` logged.
fn hub_unreachable(_failed: link::Error) -> KernelRefused {
    KernelRefused("the Kernel couldn't reach the Hub".to_owned())
}
