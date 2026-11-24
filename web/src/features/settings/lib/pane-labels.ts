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

/** What each of the Account section's rows says, at the Hub. */
export const accountLabels = {
  email: { label: "Email" },
  delete: { label: "Delete Account", words: "remove" },
} satisfies Record<string, SettingText>;

/** What each of the Account's Sign-in section's groups says, at the Hub. */
export const accountSigninLabels = {
  password: { label: "Password", words: "change set" },
  passkeys: { label: "Passkeys", words: "fingerprint face screen lock" },
} satisfies Record<string, SettingText>;

/** What the Account's Sessions section's group says, at the Hub. */
export const accountSessionLabels = {
  sessions: { label: "Where you’re signed in", words: "sign out everywhere else" },
} satisfies Record<string, SettingText>;

/** What each of the Users section's groups says, in its order, past the Users themselves. */
export const userLabels = {
  create: { label: "Create a User", words: "add new account" },
  defaults: { label: "Default Permissions", words: "new users" },
  invites: { label: "Invites", words: "invite link" },
} satisfies Record<string, SettingText>;
