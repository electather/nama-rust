//! The Extension Points the Plugins page shows: which ones an admin sees, by the client API's
//! names, and their failures.

use buffa::EnumValue;
use nama_plugin_api::{
    PluginId, PointKey,
    auth::Authenticator,
    blob_store::BlobStore,
    connector::Connector,
    fulfiller::{Fulfiller, Movies, Shows},
    home_row::HomeRowSource,
    job::JobRunner,
    metadata::TitleSource,
    notifier::Notifier,
    points::{Events, ServerStarted},
    recogniser::Recogniser,
    search_source::SearchSource,
    segment_source::SegmentSource,
    similar_source::SimilarSource,
    subtitle_source::SubtitleSource,
};
use nama_proto::nama::v1 as proto;

use crate::plugins::{ExtensionPoints, Failing};

/// Each of `plugin`'s Extension Points this API names whose last call failed.
pub(super) fn failures(
    points: &ExtensionPoints,
    plugin: &PluginId,
) -> Vec<proto::ExtensionPointFailure> {
    public_points()
        .into_iter()
        .filter_map(|(key, point)| Some(failure_to_proto(point, points.failing(plugin, key)?)))
        .collect()
}

fn failure_to_proto(
    point: proto::ExtensionPoint,
    failing: Failing,
) -> proto::ExtensionPointFailure {
    proto::ExtensionPointFailure {
        point: point.into(),
        last_error: failing.last_error,
        at_ms: failing.at_ms,
        failed_in_a_row: failing.in_a_row,
        ..Default::default()
    }
}

/// The Extension Points an admin is shown for `points`, leaving out any this API doesn't name,
/// such as a test's own.
pub(super) fn extension_points_to_proto(
    points: &[PointKey],
) -> Vec<EnumValue<proto::ExtensionPoint>> {
    let public = public_points();
    points
        .iter()
        .filter_map(|point| public.iter().find(|(key, _)| key == point))
        .map(|&(_, public)| public.into())
        .collect()
}

/// Each Extension Point this API names, by its key, in the order its enum lists them.
pub(super) fn public_points() -> [(PointKey, proto::ExtensionPoint); 16] {
    use proto::ExtensionPoint as Public;
    [
        (
            PointKey::provide::<dyn Fulfiller<Movies>>(),
            Public::EXTENSION_POINT_MOVIE_FULFILLER,
        ),
        (
            PointKey::provide::<dyn Fulfiller<Shows>>(),
            Public::EXTENSION_POINT_SHOW_FULFILLER,
        ),
        (
            PointKey::provide::<dyn Notifier>(),
            Public::EXTENSION_POINT_NOTIFIER,
        ),
        (
            PointKey::provide::<dyn Connector>(),
            Public::EXTENSION_POINT_CONNECTOR,
        ),
        (
            PointKey::provide::<dyn Authenticator>(),
            Public::EXTENSION_POINT_AUTHENTICATOR,
        ),
        (
            PointKey::provide::<dyn TitleSource>(),
            Public::EXTENSION_POINT_TITLE_SOURCE,
        ),
        (
            PointKey::observe::<Events>(),
            Public::EXTENSION_POINT_EVENTS_OBSERVER,
        ),
        (
            PointKey::observe::<ServerStarted>(),
            Public::EXTENSION_POINT_SERVER_STARTED_OBSERVER,
        ),
        (
            PointKey::provide::<dyn Recogniser>(),
            Public::EXTENSION_POINT_RECOGNISER,
        ),
        (
            PointKey::provide::<dyn BlobStore>(),
            Public::EXTENSION_POINT_BLOB_STORE,
        ),
        (
            PointKey::provide::<dyn JobRunner>(),
            Public::EXTENSION_POINT_AUTOMATION,
        ),
        (
            PointKey::provide::<dyn SubtitleSource>(),
            Public::EXTENSION_POINT_SUBTITLE_SOURCE,
        ),
        (
            PointKey::provide::<dyn HomeRowSource>(),
            Public::EXTENSION_POINT_HOME_ROW,
        ),
        (
            PointKey::provide::<dyn SegmentSource>(),
            Public::EXTENSION_POINT_SEGMENT_SOURCE,
        ),
        (
            PointKey::provide::<dyn SearchSource>(),
            Public::EXTENSION_POINT_SEARCH_SOURCE,
        ),
        (
            PointKey::provide::<dyn SimilarSource>(),
            Public::EXTENSION_POINT_SIMILAR_SOURCE,
        ),
    ]
}
