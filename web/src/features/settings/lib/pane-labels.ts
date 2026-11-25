import type { SettingText } from "@/shared/lib/setting-text";

/** What each of the Sign-in section's rows says, in its order. */
export const signinLabels = {
  username: { label: "Username", hint: "You sign in with this." },
  password: { label: "Current password", hint: "Confirm it’s you." },
  newPassword: { label: "New password", hint: "At least 8 characters.", words: "change" },
  confirm: { label: "Confirm new password" },
  switchPin: {
    label: "New Switch PIN",
    hint: "4 to 6 digits asked for before switching to you from another User. It never signs you in.",
    words: "pin lock switch user picker",
  },
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

/** What each of the Account's Servers section's groups says, at the Hub. */
export const accountServerLabels = {
  servers: { label: "Your Servers", words: "open" },
  users: { label: "Your Users", words: "unlink" },
} satisfies Record<string, SettingText>;

/** What each of the Account's Viewing History section's rows says, at the Hub. */
export const accountHistoryLabels = {
  pooling: {
    label: "Count your finished Movies and Watchlists",
    hint: "Each Movie your Linked Users finish adds one to how many Accounts finished it, and each pair of Movies, which recommendations learn from. The counts hold nothing that leads back to you, and none is used until 20 Accounts are in it. The titles on your Linked Users’ Watchlists count toward Nama’s most watchlisted the same way. Turning this off takes your Watchlists back out and stops new counts, but the Movies already counted can’t be taken out.",
    words: "pooled counts recommendations",
  },
  plays: { label: "What your Linked Users played", words: "viewing events delete export json" },
} satisfies Record<string, SettingText>;

/** What each of the Users section's groups says, in its order, past the Users themselves. */
export const userLabels = {
  create: { label: "Create a User", words: "add new account" },
  defaults: { label: "Default Permissions", words: "new users" },
  invites: { label: "Invites", words: "invite link" },
} satisfies Record<string, SettingText>;
