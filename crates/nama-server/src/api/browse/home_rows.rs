//! The Home rows a User could see: the Kernel's and the Plugins', for `ListHomeRows` and the Home
//! layouts.

use connectrpc::ConnectError;
use nama_proto::nama::v1::{self as proto, home_row::KernelOrPlugin};

use super::{Fields, refusals::home_rows_refused, refused};
use crate::{
    api::libraries::kind_to_proto,
    browse::{Browse, Browsing},
    home_rows::HomeRows,
    library::Library,
    server_settings::HomeRowId,
    users::User,
};

/// Every Home row `reader` could see now, in no layout's order: Continue Watching, each
/// Library's Recently Added row, then the Plugins' rows, each linked to its Library only if
/// `reader` sees it.
pub(in crate::api) async fn home_rows_of(
    home_rows: &HomeRows,
    browse: &Browsing,
    reader: &User,
) -> Result<Vec<(HomeRowId, KernelOrPlugin)>, ConnectError> {
    let plugin_rows = home_rows.list(reader).await.map_err(home_rows_refused)?;
    let seen = super::seen_by(browse, reader.id).await?;
    let libraries = seen.libraries().await.map_err(refused(Fields::NONE))?;
    let is_seen = |id: &i64| libraries.iter().any(|library| library.id == *id);
    let plugin_rows = plugin_rows.into_iter().map(|(id, row)| {
        let row = proto::PluginHomeRow {
            plugin_id: id.plugin().map_or_default(ToString::to_string),
            key: row.key,
            heading: row.heading,
            library_id: row.library.filter(is_seen),
            ..Default::default()
        };
        (id, row.into())
    });
    let continue_watching = proto::KernelHomeRow::KERNEL_HOME_ROW_CONTINUE_WATCHING;
    let continue_watching = (
        HomeRowId::ContinueWatching,
        KernelOrPlugin::Kernel(continue_watching.into()),
    );
    let recently_added =
        (recently_added_of(&libraries).into_iter()).map(|(id, row)| (id, row.into()));
    Ok(std::iter::once(continue_watching)
        .chain(recently_added)
        .chain(plugin_rows)
        .collect())
}

/// Each Recently Added row of a Library `browse` sees, oldest Library first.
pub(in crate::api) async fn recently_added_rows(
    browse: &Browse,
) -> Result<Vec<(HomeRowId, proto::RecentlyAddedHomeRow)>, ConnectError> {
    let libraries = browse.libraries().await.map_err(refused(Fields::NONE))?;
    Ok(recently_added_of(&libraries))
}

/// The Recently Added row of each of `libraries`, in their order.
fn recently_added_of(libraries: &[Library]) -> Vec<(HomeRowId, proto::RecentlyAddedHomeRow)> {
    (libraries.iter())
        .map(|library| {
            let row = proto::RecentlyAddedHomeRow {
                library_id: library.id,
                library_name: library.name.clone(),
                library_kind: kind_to_proto(library.kind).into(),
                ..Default::default()
            };
            (
                HomeRowId::RecentlyAdded {
                    library: library.id,
                },
                row,
            )
        })
        .collect()
}
