//! A User's Home rows and a Home layout as the API has them, for `ListHomeRows` and the Home
//! layouts.

use nama_proto::nama::v1::{
    self as proto, HomeLayoutRow, home_layout_row::Row as LayoutRow, home_row::KernelOrPlugin,
};

use crate::{
    api::libraries::kind_to_proto,
    browse::{HomeRow, SeenHomeLayout, ShownRow},
    library::Library,
};

/// A row of a User's Home as `ListHomeRows` has it: `None` for an entry of a Plugin's Users' own
/// rows, which only the default Home layout lists.
pub(in crate::api) fn home_row_to_proto((id, shown): HomeRow) -> Option<KernelOrPlugin> {
    let plugin_id = id.plugin().map_or_default(ToString::to_string);
    Some(match shown {
        ShownRow::ContinueWatching => KernelOrPlugin::Kernel(continue_watching()),
        ShownRow::RecentlyAdded(library) => recently_added_to_proto(&library).into(),
        ShownRow::Plugin {
            key,
            heading,
            library,
        } => proto::PluginHomeRow {
            plugin_id,
            key,
            heading,
            library_id: library,
            ..Default::default()
        }
        .into(),
        ShownRow::UsersOwn => return None,
    })
}

/// `layout` as the API has it.
pub(in crate::api) fn home_layout_to_proto(layout: SeenHomeLayout) -> proto::HomeLayout {
    proto::HomeLayout {
        featured_hidden: Some(!layout.featured_shown),
        rows: (layout.rows.into_iter())
            .map(|(row, shown)| HomeLayoutRow {
                row: Some(layout_row_to_proto(row)),
                hidden: !shown,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

/// A row of a Home layout as the API has it. A Plugin's row leaves its Library link off, as saving
/// reads only its Plugin and key (#2443).
fn layout_row_to_proto((id, shown): HomeRow) -> LayoutRow {
    let plugin_id = id.plugin().map_or_default(ToString::to_string);
    match shown {
        ShownRow::ContinueWatching => LayoutRow::Kernel(continue_watching()),
        ShownRow::RecentlyAdded(library) => recently_added_to_proto(&library).into(),
        ShownRow::Plugin { key, heading, .. } => proto::PluginHomeRow {
            plugin_id,
            key,
            heading,
            ..Default::default()
        }
        .into(),
        ShownRow::UsersOwn => LayoutRow::PluginUsersRows(plugin_id),
    }
}

fn continue_watching() -> buffa::EnumValue<proto::KernelHomeRow> {
    proto::KernelHomeRow::KERNEL_HOME_ROW_CONTINUE_WATCHING.into()
}

/// The Recently Added row of `library`.
fn recently_added_to_proto(library: &Library) -> proto::RecentlyAddedHomeRow {
    proto::RecentlyAddedHomeRow {
        library_id: library.id,
        library_name: library.name.clone(),
        library_kind: kind_to_proto(library.kind).into(),
        ..Default::default()
    }
}
