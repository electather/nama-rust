//! The adapter between the Kernel's Extension Points and the Plugin contract's services: one row
//! per service method, which the calls into an Installed Plugin and the checks at load both read.

use std::{sync::Arc, time::Duration};

use buffa::Message;
use nama_plugin_api::{
    PluginError, PointKey, Priority, Registrar,
    automation::Automation,
    blob_store::BlobStore,
    connector::Connector,
    fulfiller::{Fulfiller, Movies, Shows},
    home_row::HomeRowSource,
    metadata::TitleSource,
    notifier::Notifier,
    points::Events,
    recogniser::Recogniser,
    segment_source::SegmentSource,
    subtitle_source::SubtitleSource,
};
use nama_plugin_proto::services;

use super::{
    Running,
    load::{BLOB_TIMEOUT, CALL_TIMEOUT, HOME_ROWS_TIMEOUT, TICK_TIMEOUT},
};

/// The export that checks a Plugin's settings, if it has one. No service's: a Plugin of any has it.
pub(super) const CHECK_SETTINGS: &str = "check_settings";

/// The export that checks a User's Personal settings when they save them, if the Plugin has one.
pub(super) const CHECK_PERSONAL_SETTINGS: &str = "check_personal_settings";

/// One method of a contract service, as the Kernel calls it in a Plugin.
pub(super) struct Method {
    /// The service's full name.
    pub(super) service: &'static str,
    /// The method's name in the service.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "names the row; only its test reads it")
    )]
    pub(super) name: &'static str,
    /// The Plugin's export the method calls.
    pub(super) export: &'static str,
    /// Whether a Plugin naming the service may lack the export: the Kernel then doesn't ask it.
    pub(super) optional: bool,
    /// How long one call of it may run, of the Plugin's own time; past it the call fails as a
    /// timeout, and counts as a crash.
    pub(super) limit: Duration,
}

/// One Extension Point an Installed Plugin can take part at: a contract service and its methods.
pub(super) struct Point {
    /// The service's full name, which a package's Manifest names it by.
    pub(super) service: &'static str,
    /// The points that count their crashes in a row together: those whose methods call the same
    /// exports, so one working between them doesn't hide the other's crashes.
    pub(super) crash_group: &'static str,
    pub(super) methods: &'static [Method],
    pub(super) key: fn() -> PointKey,
    pub(super) register: fn(&mut Registrar, Running),
}

impl Point {
    /// The methods whose exports a package naming this point must have.
    pub(super) fn required(&self) -> impl Iterator<Item = &'static Method> {
        self.methods.iter().filter(|method| !method.optional)
    }

    /// Whether a call of one of its methods may run longer than [`CALL_TIMEOUT`].
    pub(super) fn runs_long(&self) -> bool {
        self.methods
            .iter()
            .any(|method| method.limit > CALL_TIMEOUT)
    }
}

const fn required(service: &'static str, name: &'static str, export: &'static str) -> Method {
    Method {
        service,
        name,
        export,
        optional: false,
        limit: CALL_TIMEOUT,
    }
}

/// [`required`], for a method a Plugin built before it doesn't have.
const fn optional(service: &'static str, name: &'static str, export: &'static str) -> Method {
    Method {
        optional: true,
        ..required(service, name, export)
    }
}

pub(super) const MOVIE_FULFILL: Method = required(services::MOVIE_FULFILLER, "Fulfill", "fulfill");
pub(super) const MOVIE_STOP_MONITORING: Method = optional(
    services::MOVIE_FULFILLER,
    "StopMonitoring",
    "stop_monitoring",
);
pub(super) const MOVIE_PROGRESS: Method =
    required(services::MOVIE_FULFILLER, "Progress", "progress");
pub(super) const SHOW_FULFILL: Method = required(services::SHOW_FULFILLER, "Fulfill", "fulfill");
pub(super) const SHOW_STOP_MONITORING: Method = optional(
    services::SHOW_FULFILLER,
    "StopMonitoring",
    "stop_monitoring",
);
pub(super) const SHOW_PROGRESS: Method = required(services::SHOW_FULFILLER, "Progress", "progress");
pub(super) const SHOW_UNFOLLOW: Method = required(services::SHOW_FULFILLER, "Unfollow", "unfollow");
pub(super) const DELIVER: Method = required(services::NOTIFIER, "Deliver", "deliver");
pub(super) const START_CONNECTION: Method =
    required(services::CONNECTOR, "StartConnection", "start_connection");
pub(super) const POLL_CONNECTION: Method =
    required(services::CONNECTOR, "PollConnection", "poll_connection");
pub(super) const OBSERVE_EVENTS: Method =
    required(services::EVENT_OBSERVER, "ObserveEvents", "observe_events");
pub(super) const TITLE_SEARCH: Method = optional(services::TITLE_SOURCE, "Search", "title_search");
pub(super) const TITLE_MOVIE: Method = optional(services::TITLE_SOURCE, "Movie", "title_movie");
pub(super) const TITLE_SHOW: Method = optional(services::TITLE_SOURCE, "Show", "title_show");
pub(super) const TITLE_SEASON: Method = optional(services::TITLE_SOURCE, "Season", "title_season");
pub(super) const TITLE_EPISODE: Method =
    optional(services::TITLE_SOURCE, "Episode", "title_episode");
pub(super) const TITLE_IMAGE: Method = optional(services::TITLE_SOURCE, "Image", "title_image");
pub(super) const TITLE_ARTWORK: Method =
    optional(services::TITLE_SOURCE, "Artwork", "title_artwork");
pub(super) const TITLE_SEASON_ARTWORK: Method = optional(
    services::TITLE_SOURCE,
    "SeasonArtwork",
    "title_season_artwork",
);
pub(super) const RECOGNISE_TITLE: Method =
    optional(services::RECOGNISER, "RecogniseTitle", "recognise_title");
pub(super) const RECOGNISE_FILE: Method =
    optional(services::RECOGNISER, "RecogniseFile", "recognise_file");
pub(super) const TICK: Method = Method {
    limit: TICK_TIMEOUT,
    ..required(services::AUTOMATION, "Tick", "tick")
};

/// [`required`], for a blob store method, which carries a whole object.
const fn blob(name: &'static str, export: &'static str) -> Method {
    Method {
        limit: BLOB_TIMEOUT,
        ..required(services::BLOB_STORE, name, export)
    }
}

pub(super) const BLOB_PUT: Method = blob("Put", "blob_put");
pub(super) const BLOB_GET: Method = blob("Get", "blob_get");
pub(super) const BLOB_DELETE: Method = blob("Delete", "blob_delete");
pub(super) const BLOB_LIST: Method = blob("List", "blob_list");
pub(super) const BLOB_EXISTS: Method = blob("Exists", "blob_exists");
pub(super) const SHARED_ROWS: Method = Method {
    limit: HOME_ROWS_TIMEOUT,
    ..optional(services::HOME_ROW, "SharedRows", "shared_rows")
};
pub(super) const USER_ROWS: Method = Method {
    limit: HOME_ROWS_TIMEOUT,
    ..optional(services::HOME_ROW, "UserRows", "user_rows")
};
pub(super) const USER_FEATURED: Method = Method {
    limit: HOME_ROWS_TIMEOUT,
    ..optional(services::HOME_ROW, "UserFeatured", "user_featured")
};
pub(super) const SUBTITLE_SEARCH: Method =
    required(services::SUBTITLE_SOURCE, "Search", "subtitle_search");
pub(super) const SUBTITLE_DOWNLOAD: Method =
    required(services::SUBTITLE_SOURCE, "Download", "subtitle_download");
pub(super) const SEGMENT_SEARCH: Method =
    required(services::SEGMENT_SOURCE, "Search", "segment_search");

/// The Extension Points an Installed Plugin can serve, one per service of the Plugin contract.
pub(super) const POINTS: &[Point] = &[
    Point {
        service: services::MOVIE_FULFILLER,
        crash_group: "fulfiller",
        methods: &[MOVIE_FULFILL, MOVIE_STOP_MONITORING, MOVIE_PROGRESS],
        key: PointKey::provide::<dyn Fulfiller<Movies>>,
        register: |registrar, running| {
            registrar.provide::<dyn Fulfiller<Movies>>(Priority::DEFAULT, Arc::new(running));
        },
    },
    Point {
        service: services::SHOW_FULFILLER,
        crash_group: "fulfiller",
        methods: &[
            SHOW_FULFILL,
            SHOW_STOP_MONITORING,
            SHOW_PROGRESS,
            SHOW_UNFOLLOW,
        ],
        key: PointKey::provide::<dyn Fulfiller<Shows>>,
        register: |registrar, running| {
            registrar.provide::<dyn Fulfiller<Shows>>(Priority::DEFAULT, Arc::new(running));
        },
    },
    Point {
        service: services::NOTIFIER,
        crash_group: services::NOTIFIER,
        methods: &[DELIVER],
        key: PointKey::provide::<dyn Notifier>,
        register: |registrar, running| {
            registrar.provide::<dyn Notifier>(Priority::DEFAULT, Arc::new(running));
        },
    },
    Point {
        service: services::CONNECTOR,
        crash_group: services::CONNECTOR,
        methods: &[START_CONNECTION, POLL_CONNECTION],
        key: PointKey::provide::<dyn Connector>,
        register: |registrar, running| {
            registrar.provide::<dyn Connector>(Priority::DEFAULT, Arc::new(running));
        },
    },
    Point {
        service: services::EVENT_OBSERVER,
        crash_group: services::EVENT_OBSERVER,
        methods: &[OBSERVE_EVENTS],
        key: PointKey::observe::<Events>,
        register: |registrar, running| registrar.observe::<Events>(running),
    },
    Point {
        service: services::TITLE_SOURCE,
        crash_group: services::TITLE_SOURCE,
        methods: &[
            TITLE_SEARCH,
            TITLE_MOVIE,
            TITLE_SHOW,
            TITLE_SEASON,
            TITLE_EPISODE,
            TITLE_IMAGE,
            TITLE_ARTWORK,
            TITLE_SEASON_ARTWORK,
        ],
        key: PointKey::provide::<dyn TitleSource>,
        register: |registrar, running| {
            registrar.provide::<dyn TitleSource>(Priority::DEFAULT, Arc::new(running));
        },
    },
    Point {
        service: services::AUTOMATION,
        crash_group: services::AUTOMATION,
        methods: &[TICK],
        key: PointKey::provide::<dyn Automation>,
        register: |registrar, running| {
            registrar.provide::<dyn Automation>(Priority::DEFAULT, Arc::new(running));
        },
    },
    Point {
        service: services::RECOGNISER,
        crash_group: services::RECOGNISER,
        methods: &[RECOGNISE_TITLE, RECOGNISE_FILE],
        key: PointKey::provide::<dyn Recogniser>,
        register: |registrar, running| {
            registrar.provide::<dyn Recogniser>(Priority::DEFAULT, Arc::new(running));
        },
    },
    Point {
        service: services::BLOB_STORE,
        crash_group: services::BLOB_STORE,
        methods: &[BLOB_PUT, BLOB_GET, BLOB_DELETE, BLOB_LIST, BLOB_EXISTS],
        key: PointKey::provide::<dyn BlobStore>,
        register: |registrar, running| {
            registrar.provide::<dyn BlobStore>(Priority::DEFAULT, Arc::new(running));
        },
    },
    Point {
        service: services::HOME_ROW,
        crash_group: services::HOME_ROW,
        methods: &[SHARED_ROWS, USER_ROWS, USER_FEATURED],
        key: PointKey::provide::<dyn HomeRowSource>,
        register: |registrar, running| {
            registrar.provide::<dyn HomeRowSource>(Priority::DEFAULT, Arc::new(running));
        },
    },
    Point {
        service: services::SUBTITLE_SOURCE,
        crash_group: services::SUBTITLE_SOURCE,
        methods: &[SUBTITLE_SEARCH, SUBTITLE_DOWNLOAD],
        key: PointKey::provide::<dyn SubtitleSource>,
        register: |registrar, running| {
            registrar.provide::<dyn SubtitleSource>(Priority::DEFAULT, Arc::new(running));
        },
    },
    Point {
        service: services::SEGMENT_SOURCE,
        crash_group: services::SEGMENT_SOURCE,
        methods: &[SEGMENT_SEARCH],
        key: PointKey::provide::<dyn SegmentSource>,
        register: |registrar, running| {
            registrar.provide::<dyn SegmentSource>(Priority::DEFAULT, Arc::new(running));
        },
    },
];

/// Every export the Server may call, which loading a Plugin probes for.
pub(super) fn known_exports() -> impl Iterator<Item = &'static str> {
    POINTS
        .iter()
        .flat_map(|point| point.methods)
        .map(|method| method.export)
        .chain([CHECK_SETTINGS, CHECK_PERSONAL_SETTINGS])
}

impl Running {
    /// Whether the Plugin serves `method`: a required one always, an optional one if it has the
    /// export.
    pub(super) fn serves(&self, method: &Method) -> bool {
        !method.optional || self.exports.contains(method.export)
    }

    /// Calls `method` with `request` on a fresh instance and decodes its answer. A failure is
    /// counted and handed back as [`Self::call_counted`] says, and logged by no one here: an
    /// Observer's by the Registry, a Provider's by its caller.
    pub(super) async fn call_method<Req: Message, Resp: Message>(
        &self,
        method: &Method,
        request: &Req,
    ) -> Result<Resp, PluginError> {
        let input = request.encode_to_vec();
        let output = self
            .call_counted(method.export, input, method.limit)
            .await?;
        Resp::decode_from_slice(&output)
            .map_err(|err| format!("its answer didn't decode: {err}").into())
    }
}

#[cfg(test)]
#[path = "services_tests.rs"]
mod tests;
