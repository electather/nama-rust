import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, ConnectError } from "@connectrpc/connect";

import type { SubscribeResponse } from "@/gen/nama/v1/event_pb";
import type { LibrarySchema } from "@/gen/nama/v1/library_pb";
import { SubtitleBackground, SubtitleSize } from "@/gen/nama/v1/playback_pb";
import type { PluginSchema } from "@/gen/nama/v1/plugin_pb";
import type {
  GetServerInfoResponseSchema,
  ImageSettingsSchema,
  PlaybackSettingsSchema,
  ServerSettingsSchema,
} from "@/gen/nama/v1/server_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { type Answers, serveServerApi } from "@/test-server-api";

/** What a new User gets: the Permissions to make Requests, which manage nothing. */
export const newUser = [Permission.REQUEST, Permission.VIEW_REQUESTS];

/** What the first Admin has. */
export const admin = [Permission.ADMIN];

/** What a fake Server holds, which its reads answer from and a test may read and change as it goes. */
export interface ServerModel {
  /** What `getServerInfo` answers: by default the Server "Living Room", set up. */
  info: MessageInitShape<typeof GetServerInfoResponseSchema>;
  /** The Permissions of Sam, the User signed in, or `undefined` while nobody is. */
  signedIn: Permission[] | undefined;
  libraries: MessageInitShape<typeof LibrarySchema>[];
  /** Each Plugin's one record: `listPlugins` lists its summary and `getPlugin` answers it whole. */
  plugins: MessageInitShape<typeof PluginSchema>[];
  serverSettings: MessageInitShape<typeof ServerSettingsSchema>;
  playbackSettings: MessageInitShape<typeof PlaybackSettingsSchema>;
  /** By default, images are resized. */
  imageSettings: MessageInitShape<typeof ImageSettingsSchema>;
  /** The Events the app's subscription gets: by default, none ever. */
  events: ReadableStream<SubscribeResponse>;
}

/**
 * A fake Server holding `facts` over the defaults, whose reads answer from the model and whose
 * other methods answer empty, except where `answers` answers them. An answer may change the model,
 * and later reads see it. `strict` fails each call nothing answers, naming its method, instead.
 */
export function fakeServer(
  facts: Partial<ServerModel> = {},
  answers: Answers = {},
  { strict = false } = {},
) {
  const server: ServerModel = {
    signedIn: undefined,
    libraries: [],
    plugins: [],
    serverSettings: {},
    playbackSettings: {},
    imageSettings: { resizeImages: true },
    events: new ReadableStream(),
    ...facts,
    info: { name: "Living Room", ...facts.info },
  };
  return { server, transport: serveServerApi([derived(server), answers], strict) };
}

/** How the model answers the reads it holds, and today's User and Playback defaults. */
function derived(server: ServerModel): Answers {
  return {
    ServerService: {
      getServerInfo: () => server.info,
      getServerSettings: () => ({ settings: server.serverSettings }),
      getPlaybackSettings: () => ({ settings: server.playbackSettings }),
      getImageSettings: () => ({ settings: server.imageSettings }),
    },
    SignInService: {
      whoAmI: () => {
        if (server.signedIn === undefined) {
          throw new ConnectError("signed out", Code.Unauthenticated);
        }
        return { user: { id: 1n, name: "Sam", permissions: server.signedIn } };
      },
    },
    LibraryService: {
      listLibraries: () => ({ libraries: server.libraries }),
      getLibrary: ({ id }) => ({
        library: found(server.libraries, (library) => library.id === id, `Library ${id}`),
      }),
    },
    PluginService: {
      listPlugins: () => ({ summaries: server.plugins.map((plugin) => plugin.summary ?? {}) }),
      getPlugin: ({ id }) => ({
        plugin: found(server.plugins, (plugin) => plugin.summary?.id === id, `Plugin ${id}`),
      }),
    },
    EventService: { subscribe: () => server.events },
    PlaybackService: {
      getSubtitleAppearance: () => ({
        size: SubtitleSize.DEFAULT,
        background: SubtitleBackground.OUTLINE,
      }),
      getAutoplay: () => ({ on: true }),
    },
    UserService: { getDefaultPermissions: () => ({ permissions: newUser }) },
  };
}

/** The one of `all` that `is`, as the Server finds it, or its refusal naming `what`. */
function found<T>(all: T[], is: (each: T) => boolean, what: string) {
  const match = all.find((each) => is(each));
  if (match === undefined) {
    throw new ConnectError(`there is no ${what}`, Code.NotFound);
  }
  return match;
}
