import { create } from "@bufbuild/protobuf";
import type { ConnectRouter } from "@connectrpc/connect";

import {
  type LinkedServer,
  LinkedServerSchema,
  type LinkedUser,
  LinkedUserSchema,
  LinkService,
  type UserInvite,
} from "@/gen/nama/hub/v1/link_pb";
import { type ViewingEvent, ViewingEventService } from "@/gen/nama/hub/v1/viewing_pb";

/** What a fake Hub holds about the Account's Servers and plays, which a test may read and change. */
export interface LinksHub {
  servers: LinkedServer[];
  users: LinkedUser[];
  /** The Users Admins created from the Account, waiting for it to accept them. */
  invites: UserInvite[];
  /** What the Hub refuses accepting an Invite with, if it does. */
  acceptRefusal?: Error | undefined;
  events: ViewingEvent[];
  pooling: boolean;
}

/**
 * Serves `hub`'s Servers, Users, Invites and plays. Accepting an Invite Links its User on its
 * Server, and any code Links Ada's User on Basement.
 */
export function serveLinks(service: ConnectRouter["service"], hub: LinksHub) {
  service(LinkService, {
    listLinkedServers: () => ({ servers: hub.servers }),
    listLinkedUsers: () => ({ users: hub.users }),
    approveUserLink: () => {
      const user = create(LinkedUserSchema, {
        id: "link-ada",
        name: "Ada",
        serverName: "Basement",
      });
      hub.users = [...hub.users, user];
      return { user };
    },
    unlinkLinkedUser: ({ id }) => {
      hub.users = hub.users.filter((user) => user.id !== id);
      return {};
    },
    listUserInvites: () => ({ invites: hub.invites }),
    acceptUserInvite: ({ id }) => {
      if (hub.acceptRefusal) {
        throw hub.acceptRefusal;
      }
      const accepted = hub.invites.filter((invite) => invite.id === id);
      hub.invites = hub.invites.filter((invite) => invite.id !== id);
      const users = accepted.map(({ name, serverName }) =>
        create(LinkedUserSchema, { id: `link-${id}`, name, serverName }),
      );
      hub.users = [...hub.users, ...users];
      const servers = accepted.map(({ serverName }) =>
        create(LinkedServerSchema, { id: serverName, name: serverName }),
      );
      hub.servers = [...hub.servers, ...servers];
      return { user: users[0] };
    },
  });
  service(ViewingEventService, {
    listViewingEvents: () => ({ events: hub.events }),
    deleteViewingEvent: ({ id }) => {
      hub.events = hub.events.filter((event) => event.id !== id);
      return {};
    },
    getPooling: () => ({ pooling: hub.pooling }),
    setPooling: ({ pooling }) => {
      hub.pooling = pooling;
      return {};
    },
  });
}
