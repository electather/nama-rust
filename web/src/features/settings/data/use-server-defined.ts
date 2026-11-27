import { useQuery } from "@connectrpc/connect-query";

import {
  type ServerDefinedSetting,
  type ServerDefinedSource,
  homeRowFindables,
  personalFindables,
  qualityFindables,
} from "@/features/settings/lib/server-defined";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { UserService } from "@/gen/nama/v1/user_pb";

/**
 * The settings the Server defines in `sources`, for Settings' search: none until `wanted`, so
 * opening Settings asks for nothing. Each joins as it loads, and one that fails adds nothing.
 */
export function useServerDefined(
  wanted: boolean,
  sources: ServerDefinedSource[],
): ServerDefinedSetting[] {
  const asks = (source: ServerDefinedSource) => wanted && sources.includes(source);
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the built-in results show at once, and these join them as they load
  const personal = useQuery(
    UserService.method.listPersonalSettings,
    {},
    { enabled: asks("personal-settings") },
  );
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the default Qualities join the built-in results as they load
  const qualities = useQuery(
    RequestService.method.listDefaultQualities,
    {},
    { enabled: asks("default-qualities") },
  );
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the Home rows join the built-in results as they load
  const home = useQuery(UserService.method.getHomeLayout, {}, { enabled: asks("home-rows") });
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the Default Home rows join the built-in results as they load
  const defaultHome = useQuery(
    ServerService.method.getDefaultHomeLayout,
    {},
    { enabled: asks("default-home-rows") },
  );
  return [
    ...personalFindables(personal.data),
    ...qualityFindables(qualities.data),
    ...homeRowFindables("home-rows", home.data),
    ...homeRowFindables("default-home-rows", defaultHome.data),
  ];
}
