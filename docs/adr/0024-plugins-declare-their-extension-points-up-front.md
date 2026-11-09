# Every Plugin has a Manifest that names its Extension Points

The Plugins page shows what each Plugin does, such as Fulfiller for Movies or Notifier, including Plugins that are disabled. An Installed Plugin's Manifest already listed its Observers and Providers, but a Built-in Plugin only revealed them in `register()`, which runs when it is enabled. So every Plugin, Built-in or Installed, has a Manifest, read before it is enabled, that names its Extension Points along with its name, description and logo, and the Kernel refuses to enable a Plugin whose `register()` registers a point its Manifest doesn't name.

## Considered Options

- **Read Extension Points from the Registry of enabled Plugins**: rejected. A disabled Plugin would show nothing, which is exactly when an admin is deciding whether to enable it.
- **Declare without enforcing**: rejected. The Manifest would drift from what `register()` really does, and the page would lie.
- **Separate trait methods for each new field on Built-in Plugins**: rejected. One Manifest for both kinds keeps one word and one shape for "what a Plugin says about itself".
