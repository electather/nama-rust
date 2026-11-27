import type { DescMethod, DescService } from "@bufbuild/protobuf";
import {
  Code,
  ConnectError,
  type ServiceImpl,
  type Transport,
  createRouterTransport,
} from "@connectrpc/connect";

import type { BrowseService } from "@/gen/nama/v1/browse_pb";
import type { EventService } from "@/gen/nama/v1/event_pb";
import type { InboxService } from "@/gen/nama/v1/inbox_pb";
import type { JobService } from "@/gen/nama/v1/job_pb";
import type { LibraryService } from "@/gen/nama/v1/library_pb";
import type { MatchService } from "@/gen/nama/v1/match_pb";
import type { PlaybackService } from "@/gen/nama/v1/playback_pb";
import type { PluginService } from "@/gen/nama/v1/plugin_pb";
import type { RequestService } from "@/gen/nama/v1/request_pb";
import type { SearchService } from "@/gen/nama/v1/search_pb";
import type { ServerService } from "@/gen/nama/v1/server_pb";
import type { SignInService } from "@/gen/nama/v1/sign_in_pb";
import type { UserService } from "@/gen/nama/v1/user_pb";
import type { WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import type { WatchlistService } from "@/gen/nama/v1/watchlist_pb";
import type { WebPushService } from "@/gen/nama/v1/web_push_pb";
import { traceCallsTo } from "@/shared/lib/diagnostics";

/** How a test answers some methods of the Server's API, by service and method name. */
export interface Answers {
  BrowseService?: Partial<ServiceImpl<typeof BrowseService>>;
  EventService?: Partial<ServiceImpl<typeof EventService>>;
  InboxService?: Partial<ServiceImpl<typeof InboxService>>;
  JobService?: Partial<ServiceImpl<typeof JobService>>;
  LibraryService?: Partial<ServiceImpl<typeof LibraryService>>;
  MatchService?: Partial<ServiceImpl<typeof MatchService>>;
  PlaybackService?: Partial<ServiceImpl<typeof PlaybackService>>;
  PluginService?: Partial<ServiceImpl<typeof PluginService>>;
  RequestService?: Partial<ServiceImpl<typeof RequestService>>;
  SearchService?: Partial<ServiceImpl<typeof SearchService>>;
  ServerService?: Partial<ServiceImpl<typeof ServerService>>;
  SignInService?: Partial<ServiceImpl<typeof SignInService>>;
  UserService?: Partial<ServiceImpl<typeof UserService>>;
  WatchStateService?: Partial<ServiceImpl<typeof WatchStateService>>;
  WatchlistService?: Partial<ServiceImpl<typeof WatchlistService>>;
  WebPushService?: Partial<ServiceImpl<typeof WebPushService>>;
}

// Only `nama/v1`: a Server answers the Hub's API as Unimplemented, which is how the app knows it's
// at a Server (ADR 0105).
const generated = import.meta.glob<Record<string, unknown>>("./gen/nama/v1/*_pb.ts", {
  eager: true,
});

/** Every service of the Server's generated API. */
const serverApi = Object.values(generated)
  .flatMap((module) => Object.values(module))
  .filter((exported): exported is DescService => isService(exported));

function isService(exported: unknown): exported is DescService {
  return (
    typeof exported === "object" &&
    exported !== null &&
    "kind" in exported &&
    exported.kind === "service"
  );
}

/**
 * A transport to a Server answering each method of its API with the last of `layers` that has it,
 * else an empty message, or, when `strict`, an error naming it.
 */
export function serveServerApi(layers: Answers[], strict: boolean): Transport {
  return createRouterTransport(
    (router) => {
      for (const service of serverApi) {
        const methods = Object.fromEntries<unknown>([
          ...service.methods.map(
            (method) => [method.localName, unanswered(method, strict)] as const,
          ),
          ...layers.flatMap((layer) => Object.entries(answersOf(layer, service.name))),
        ]);
        // Every method at once: Connect's router answers one a registration leaves out as
        // Unimplemented, and a later registration of the service replaces it whole.
        // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- Connect can't type the methods of a service found at runtime
        router.service(service, methods as Partial<ServiceImpl<DescService>>);
      }
    },
    // The app's transport traces calls while the Server exports Diagnostics, so the fake's does too.
    { transport: { interceptors: [traceCallsTo("")] } },
  );
}

function answersOf(layer: Answers, service: string) {
  // A spread copy, since an interface such as `Answers` can't be indexed by a runtime name.
  const byName: Record<string, object | undefined> = { ...layer };
  return byName[service] ?? {};
}

function unanswered(method: DescMethod, strict: boolean) {
  if (strict) {
    return () => {
      throw new ConnectError(
        `nothing answers ${method.parent.name}.${method.name}`,
        Code.Unimplemented,
      );
    };
  }
  // A streaming method sends nothing and ends.
  return method.methodKind === "unary" ? () => ({}) : sendNothing;
}

async function* sendNothing() {}
