# Admins grant Plugin Permissions when enabling a Plugin

_Amended by [ADR 0079](0079-plugins-declare-jobs-and-the-kernel-runs-them-on-cron.md): Jobs replace the Tick, so a Job's Run is what calls the Kernel's functions when no Admin is watching, and a Built-in Plugin always holds every Plugin Permission it requests, so only an Installed Plugin waits for an Admin's grants._

A Plugin in the Automation lane calls the Kernel's functions (ADR 0048), and each needs a Plugin Permission. The Plugin's Manifest requests the Plugin Permissions it wants in `permissions`; the Admin grants some or all of them in `EnablePlugin`'s `granted_permissions`, which replaces what was granted before. `PluginSummary` lists both, `requested_permissions` and `granted_permissions`, so the Plugins page can show what a Plugin asks for before it is enabled. Granting one the Manifest doesn't request is refused, naming the field and the Permission, and saves nothing. A Kernel function whose Plugin Permission wasn't granted answers `refused` in plain words, which the Plugin handles. The Plugin Permissions are Store, Read Libraries, Read Watch States, Delete media, Read Users and Read Requests. Read Users lets a Plugin list every User's id and display name and nothing more: who the Users are is a grant apart from what they watched. Read Requests likewise lets it see only which Users Requested each title, never the Requests themselves.

Plugin Permissions are not a User's Permissions (ADR 0020). A User's say what that person may do through the client API; a Plugin's say what the Plugin may do in the Server, whoever its work is for. The client API has its own `PluginPermission` enum, since it imports nothing from the Plugin contract (ADR 0041), and the API maps it to the contract's `Permission`.

A Built-in Plugin is Rust compiled into the Server, and nothing limits what it reaches. An Installed Plugin's HTTP reaches only its Manifest's allowed hosts and the hosts of its URL settings (ADR 0006), and its Automation-lane calls need the Plugin Permissions an Admin granted. Nothing else limits what it reaches; its call timeouts and memory cap only keep a stuck Plugin from stalling the Server. So an Installed Plugin is not sandboxed, and no doc or UI text says it is; they say it can only call what an Admin granted.

Grants are saved with the Plugin's settings, in `plugin_settings.granted_permissions`, in the same write that saves it as enabled, after it is enabled with them (ADR 0028's order). They survive a restart, which enables the Plugin with them, and disabling a Plugin keeps them; enabling it again replaces them, and enabling it with none clears them. A Plugin already running with other grants is disabled and enabled again with the new ones, as a settings change does, so nothing it registered with the old grants stays.

## Considered Options

- **Granting everything the Manifest requests, implicitly, on enable**: rejected. An Admin would let a downloaded Plugin delete media just by turning it on, and couldn't run it with less.
- **Asking the Admin on each call**: rejected. The Kernel's functions run during a Tick, when no Admin is watching, so a prompt would block the Plugin or be answered blindly.
- **Granting at enable, saved beside the settings**: chosen.

## Consequences

- `PluginSettings::enable` grants nothing; the API calls `enable_granting`. A client that enables a Plugin without naming its Plugin Permissions clears them.
- A Plugin upgraded to request fewer Plugin Permissions keeps its stored grants, but the Kernel only grants those its Manifest still requests.
