# Users set a Plugin's Personal settings for themselves

Some of what a Plugin needs belongs to each User, not to the Admin, such as an ntfy topic or a Discord webhook. A Manifest declares these as `personal_settings`, beside its Plugin-wide `settings`. Only the TEXT, SECRET, BOOL and INTEGER kinds are allowed; URL and CHOICE are refused, because a User's value must not widen the hosts a Plugin reaches. A key may not repeat, nor clash with a key of the Plugin's own settings, and a Manifest breaking either rule is refused. The Kernel stores the values per User and Plugin in a table it owns, `personal_settings`, deleted with the User. A User reads and sets only their own values, through UserService `ListPersonalSettings`, `UpdatePersonalSettings` and `DeletePersonalSettings`. A secret value is never sent back to any client nor logged, and an Admin never sees another User's values. The values reach a Plugin only inside a call made for that User, never through Extism config. The first such call is a Notifier `Delivery` whose recipient is a `UserRecipient` (NotifierService 1.1.0), which carries that User's Personal settings for the Plugin. A `RecipientGone` answer for a User recipient makes the Kernel delete that User's Personal settings for the Plugin (NotifierService 1.2.0), as it deletes a Plugin Connection found gone.

## Considered Options

- **Personal settings in Extism config, set per call**: rejected. An Installed Plugin's config is Plugin-wide and outlives a call, so one User's values could leak into a call made for another.
- **Letting Admins see Personal settings**: rejected. A webhook or access token is the User's credential at an outside service, and the Admin has no need for it.
- **The URL kind for Personal settings**: rejected. A URL setting adds its host to the hosts a Plugin may reach, and that list is the Admin's to grant, not each User's. An ntfy server URL is a Plugin setting the Admin sets; a Discord webhook is a SECRET whose host the Manifest already names.

## Consequences

- A Plugin that needs per-User values declares them once; the Kernel owns storing, showing and deleting them, so no Plugin keeps its own per-User store for them.
- A Plugin can't read a User's Personal settings outside a call for that User, so nothing it does in the background can act for a User through them.
- A `RecipientGone` for a User recipient deletes their values without telling them; they find the settings empty and set them again.
