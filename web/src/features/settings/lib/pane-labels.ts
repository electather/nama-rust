import type { SettingText } from "@/shared/lib/setting-text";

/** What each of the Sign-in section's rows says, in its order. */
export const signinLabels = {
  username: { label: "Username", hint: "You sign in with this." },
  password: { label: "Current password", hint: "Confirm it’s you." },
  newPassword: { label: "New password", hint: "At least 8 characters.", words: "change" },
  confirm: { label: "Confirm new password" },
} satisfies Record<string, SettingText>;

/** What each of the Devices section's groups says, in its order. */
export const deviceLabels = {
  current: { label: "This Device" },
  others: { label: "Other Devices", words: "revoke sign out sign in tv phone code" },
} satisfies Record<string, SettingText>;

/** What each of the Users section's groups says, in its order, past the Users themselves. */
export const userLabels = {
  create: { label: "Create a User", words: "add new account" },
  defaults: { label: "Default Permissions", words: "new users" },
  invites: { label: "Invites", words: "invite link" },
} satisfies Record<string, SettingText>;
