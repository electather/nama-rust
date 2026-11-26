import type { Interceptor } from "@connectrpc/connect";

const prefix = "nama:server-session:v1:";

/** Where this browser keeps the session of the Account `accountId` on its Linked Server `serverId`. */
export function serverSessionKey(accountId: string, serverId: string) {
  // The Account is in the key, so another Account signed in here later never gets its session.
  return `${prefix}${accountId}:${serverId}`;
}

/** The Linked Server a `serverSessionKey` names, or `undefined` for a key it didn't make. */
function serverIdIn(key: string) {
  return /^nama:server-session:v1:[^:]+:(?<serverId>[^:]+)$/u.exec(key)?.groups?.["serverId"];
}

/**
 * Forgets the session this browser keeps on each of the Account's Servers, as signing out does.
 * Returns each one's Server and token, so its Device can be signed out there.
 */
export function forgetServerSessions() {
  const keys = Array.from({ length: localStorage.length }, (_, at) => localStorage.key(at));
  return keys.flatMap((key) => {
    if (key?.startsWith(prefix) !== true) {
      return [];
    }
    const token = localStorage.getItem(key);
    localStorage.removeItem(key);
    const serverId = serverIdIn(key);
    return serverId === undefined || token === null ? [] : [{ serverId, token }];
  });
}

/** Signs calls in with the Device session `token`. */
export function bearer(token: string): Interceptor {
  return (next) => async (request) => {
    request.header.set("Authorization", `Bearer ${token}`);
    const response = await next(request);
    return response;
  };
}
