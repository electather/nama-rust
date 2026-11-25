const prefix = "nama:server-session:v1:";

/** Where this browser keeps the session of the Account `accountId` on its Linked Server `serverId`. */
export function serverSessionKey(accountId: string, serverId: string) {
  // The Account is in the key, so another Account signed in here later never gets its session.
  return `${prefix}${accountId}:${serverId}`;
}

/** Forgets the session this browser keeps on each of the Account's Servers, as signing out does. */
export function forgetServerSessions() {
  const keys = Array.from({ length: localStorage.length }, (_, at) => localStorage.key(at));
  for (const key of keys) {
    if (key?.startsWith(prefix) === true) {
      localStorage.removeItem(key);
    }
  }
}
