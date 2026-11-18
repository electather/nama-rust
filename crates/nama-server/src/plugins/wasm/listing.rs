//! Listing the Installed Plugins the Server read beside the Built-in ones.

use std::path::{Path, PathBuf};
#[cfg(feature = "testing")]
use std::sync::Arc;

#[cfg(feature = "testing")]
use nama_plugin_api::Plugin;
use nama_plugin_api::PluginId;
use nama_proto::nama::v1::PluginKind;
use tracing::{info, warn};

use super::{Installed, Listable, Refused, load::read_plugins, printable};
use crate::plugins::{Registry, registry::Described};

/// The bundled Installed Plugins and those in `<data_dir>/plugins/`, compiled but not yet listed.
pub(crate) struct Found(Vec<(PathBuf, Result<Installed, Refused>)>);

/// Reads the bundled Installed Plugins and those in `<data_dir>/plugins/`, for
/// [`Registry::list_installed`] to list.
///
/// Blocks while it compiles each Plugin. wasmtime caches what it compiles under
/// `<data_dir>/wasmtime/`, not in the home directory.
pub(crate) fn read(data_dir: &Path) -> Found {
    Found(read_plugins(data_dir))
}

impl Registry {
    /// Reads and lists the bundled Installed Plugins and those in `<data_dir>/plugins/` at start,
    /// as listing the Plugins does, and logs each one's id and version, or why the Server refused
    /// it. Enables none.
    pub(crate) fn load_installed(&self, data_dir: &Path) {
        let found = read(data_dir);
        for (file, plugin) in &found.0 {
            match plugin {
                Ok(plugin) => info!(
                    file = %file.display(),
                    id = %plugin.manifest.id,
                    version = %plugin.manifest.version,
                    "found Installed Plugin"
                ),
                Err(refused) => {
                    let refused = printable(&refused.to_string());
                    warn!(file = %file.display(), "refused Installed Plugin: {refused}");
                }
            }
        }
        self.list_installed(found);
    }

    /// Lists the Installed Plugins `found` beside the Built-in ones, in place of those listed
    /// before. An enabled Plugin stays listed as it was enabled, even if its file is gone or
    /// changed, until it is disabled. Enables none.
    pub(crate) fn list_installed(&self, found: Found) {
        let mut taken: Vec<PluginId> = self.read(|state| {
            let builtin = |(id, known): (&PluginId, &Described)| {
                (known.kind == PluginKind::PLUGIN_KIND_BUILT_IN).then(|| id.clone())
            };
            state.known.iter().filter_map(builtin).collect()
        });
        let found = found.0.into_iter();
        let found = found.map(|(file, plugin)| (file, plugin.map(Listable::from)));
        #[cfg(feature = "testing")]
        let found = found.chain(self.made_as_installed());
        let mut described = Vec::new();
        for (file, plugin) in found {
            let (id, known) = describe(&file, plugin, &taken);
            if known.plugin.is_ok() {
                taken.push(id.clone());
            }
            described.push((id, known));
        }
        self.write(|state| {
            let enabled = &state.plugins;
            state.known.retain(|id, known| {
                known.kind == PluginKind::PLUGIN_KIND_BUILT_IN || enabled.contains_key(id)
            });
            for (id, known) in described {
                state.known.entry(id).or_insert(known);
            }
        });
    }

    /// Lists the in-process Plugins `plugins` make as Installed ones beside the Packages found on
    /// disk, from the next listing on and in place of those given before, so a test can try what
    /// holds for an Installed Plugin without building a Package.
    #[cfg(feature = "testing")]
    pub fn list_as_installed(&self, plugins: Vec<fn() -> Arc<dyn Plugin>>) {
        self.write(|state| state.as_installed = plugins);
    }

    /// The Plugins [`Registry::list_as_installed`] lists, each as if found in a file named after
    /// its id. One that panics saying what it is is logged and left out.
    #[cfg(feature = "testing")]
    fn made_as_installed(&self) -> Vec<(PathBuf, Result<Listable, Refused>)> {
        let plugins = self.read(|state| state.as_installed.clone());
        let listable = |make: fn() -> Arc<dyn Plugin>| {
            let (manifest, checks_settings) = crate::plugins::what_it_says(&make)?;
            // Not the id itself, so one refused for its id taken is listed apart from the holder.
            let file = PathBuf::from(format!("{}.in-process", manifest.id));
            let make = Arc::new(make);
            Some((
                file,
                Ok(Listable {
                    make,
                    manifest,
                    checks_settings,
                }),
            ))
        };
        plugins.into_iter().filter_map(listable).collect()
    }
}

/// The Installed Plugin found in `file`, or listed in-process under a made-up one, as the registry
/// lists it: by its id, or by the file's name if the Server refused it, since it may not know the
/// id. One whose id is `taken` is refused.
fn describe(
    file: &Path,
    plugin: Result<Listable, Refused>,
    taken: &[PluginId],
) -> (PluginId, Described) {
    let plugin = plugin.and_then(|plugin| match &plugin.manifest.id {
        id if taken.contains(id) => Err(Refused::IdTaken(id.clone())),
        _ => Ok(plugin),
    });
    match plugin {
        Ok(Listable {
            make,
            manifest,
            checks_settings,
        }) => {
            let id = manifest.id.clone();
            let described = Described {
                kind: PluginKind::PLUGIN_KIND_INSTALLED,
                plugin: Ok((make, manifest)),
                checks_settings,
            };
            (id, described)
        }
        Err(refused) => {
            let name = file.file_name().unwrap_or_default().to_string_lossy();
            let described = Described {
                kind: PluginKind::PLUGIN_KIND_INSTALLED,
                plugin: Err(refused.to_string()),
                checks_settings: false,
            };
            (PluginId::from(name.into_owned()), described)
        }
    }
}
