//! The Home rows a User could see: the Kernel's and the Plugins', for `ListHomeRows` and the Home
//! layouts.

use connectrpc::ConnectError;
use nama_proto::nama::v1::{self as proto, home_row::KernelOrPlugin};

use super::refusals::home_rows_refused;
use crate::{home_rows::HomeRows, server_settings::HomeRowId, users::User};

/// Every Home row `reader` could see now, in no layout's order: Continue Watching, then the
/// Plugins' rows.
pub(in crate::api) async fn home_rows_of(
    home_rows: &HomeRows,
    reader: &User,
) -> Result<Vec<(HomeRowId, KernelOrPlugin)>, ConnectError> {
    let plugin_rows = home_rows.list(reader).await.map_err(home_rows_refused)?;
    let plugin_rows = plugin_rows.into_iter().map(|(id, row)| {
        let row = proto::PluginHomeRow {
            plugin_id: id.plugin().map_or_default(ToString::to_string),
            key: row.key,
            heading: row.heading,
            ..Default::default()
        };
        (id, row.into())
    });
    let continue_watching = proto::KernelHomeRow::KERNEL_HOME_ROW_CONTINUE_WATCHING;
    let continue_watching = (
        HomeRowId::ContinueWatching,
        KernelOrPlugin::Kernel(continue_watching.into()),
    );
    Ok(std::iter::once(continue_watching)
        .chain(plugin_rows)
        .collect())
}
