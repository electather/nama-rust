import { create } from "@bufbuild/protobuf";
import { Code, ConnectError, createRouterTransport } from "@connectrpc/connect";

import {
  AccountSchema,
  AccountService,
  type Passkey,
  PasskeySchema,
  type Session,
  SessionSchema,
} from "@/gen/nama/hub/v1/account_pb";
import { HubService } from "@/gen/nama/hub/v1/hub_pb";
import { RefusalReason, RefusalSchema } from "@/gen/nama/hub/v1/refusal_pb";
import { type LinksHub, serveLinks } from "@/test-hub-links";

/** The Account the fake Hubs know, whose password is "correct horse". */
export const ada = create(AccountSchema, { id: "0192-ada", email: "ada@example.com" });

/** A Connect error as the Hub refuses with one. */
export function hubRefused(code: Code, reason: RefusalReason, params: Record<string, string> = {}) {
  return new ConnectError("refused", code, undefined, [
    { desc: RefusalSchema, value: create(RefusalSchema, { reason, params }) },
  ]);
}

/** What a fake Hub's Account holds, which a test may read and change as it goes. */
export interface AccountHub extends LinksHub {
  /** Whether this browser's session is live; another browser may end it. */
  signedIn: boolean;
  hasPassword: boolean;
  /** Whether the Hub offers passkeys. */
  offersPasskeys: boolean;
  passkeys: Passkey[];
  sessions: Session[];
  /** Whether a change is refused for want of Recent Authentication. */
  stale: boolean;
  /** Seconds the Hub makes a new passkey wait after too many tries, or 0 to start one. */
  passkeyWait: number;
  /** Whether signing out fails, as when the Hub can't be reached. */
  signOutFails: boolean;
  deleted: boolean;
}

/**
 * A fake Hub where Ada, signed in, has a password, a passkey named "Laptop", and is signed in on
 * this browser and on "Safari on iPhone", with no Server, User, Invite or Viewing Event, over what
 * `state` says. Signing in, with "correct horse", gives Recent Authentication again.
 */
export function accountHub(state: Partial<AccountHub> = {}) {
  const hub: AccountHub = {
    signedIn: true,
    hasPassword: true,
    offersPasskeys: true,
    passkeys: [create(PasskeySchema, { id: "0193-laptop", name: "Laptop" })],
    sessions: [
      create(SessionSchema, { id: "s1", name: "Firefox on Linux", isCurrent: true }),
      create(SessionSchema, { id: "s2", name: "Safari on iPhone", lastUsedAt: 1n }),
    ],
    stale: false,
    passkeyWait: 0,
    signOutFails: false,
    deleted: false,
    servers: [],
    users: [],
    invites: [],
    events: [],
    pooling: true,
    ...state,
  };
  const signedInOnly = () => {
    if (!hub.signedIn) {
      throw hubRefused(Code.Unauthenticated, RefusalReason.SIGN_IN_REQUIRED);
    }
  };
  const recentOnly = () => {
    signedInOnly();
    if (hub.stale) {
      throw hubRefused(Code.FailedPrecondition, RefusalReason.RECENT_AUTHENTICATION_REQUIRED, {
        recentMinutes: "10",
      });
    }
  };
  const transport = createRouterTransport(({ service }) => {
    service(HubService, {
      getHubInfo: () => ({ version: "1.0.0", passkeys: hub.offersPasskeys }),
    });
    service(AccountService, {
      getAccount: () => {
        signedInOnly();
        return { account: ada, hasPassword: hub.hasPassword };
      },
      signIn: () => {
        Object.assign(hub, { signedIn: true, stale: false });
        return { account: ada };
      },
      signOut: () => {
        if (hub.signOutFails) {
          throw new ConnectError("unreachable", Code.Unavailable);
        }
        hub.signedIn = false;
        return {};
      },
      setPassword: ({ password }) => {
        recentOnly();
        if (password.length < 8) {
          throw hubRefused(Code.InvalidArgument, RefusalReason.PASSWORD_LENGTH, {
            minLength: "8",
            maxLength: "1024",
          });
        }
        hub.hasPassword = true;
        return {};
      },
      listPasskeys: () => {
        signedInOnly();
        return { passkeys: hub.passkeys };
      },
      startAddingPasskey: () => {
        recentOnly();
        if (hub.passkeyWait > 0) {
          throw hubRefused(Code.ResourceExhausted, RefusalReason.TOO_MANY_ATTEMPTS, {
            retryAfterSeconds: String(hub.passkeyWait),
          });
        }
        return { challenge: new Uint8Array(32), rpId: "hub.example", userId: new Uint8Array(16) };
      },
      addPasskey: ({ name }) => {
        const passkey = create(PasskeySchema, { id: `0193-${name}`, name });
        hub.passkeys = [...hub.passkeys, passkey];
        return { passkey };
      },
      removePasskey: ({ passkeyId }) => {
        recentOnly();
        hub.passkeys = hub.passkeys.filter((passkey) => passkey.id !== passkeyId);
        return {};
      },
      listSessions: () => {
        signedInOnly();
        return { sessions: hub.sessions };
      },
      signOutSession: ({ sessionId }) => {
        recentOnly();
        hub.sessions = hub.sessions.filter((session) => session.id !== sessionId);
        return {};
      },
      signOutOtherSessions: () => {
        recentOnly();
        hub.sessions = hub.sessions.filter((session) => session.isCurrent);
        return {};
      },
      deleteAccount: () => {
        recentOnly();
        Object.assign(hub, { signedIn: false, deleted: true });
        return {};
      },
    });
    serveLinks(service, hub);
  });
  return { hub, transport };
}
