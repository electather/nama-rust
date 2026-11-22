import { useQuery } from "@connectrpc/connect-query";

import type { Section, SectionGroup } from "@/features/settings/lib/sections";
import {
  homeRowFindables,
  personalFindables,
  qualityFindables,
} from "@/features/settings/lib/server-defined";
import type { Findable } from "@/features/settings/lib/setting-search";
import { RequestService } from "@/gen/nama/v1/request_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { UserService } from "@/gen/nama/v1/user_pb";

/**
 * The settings the Server defines in the Sections of `groups`, for Settings' search: none until
 * `wanted`, so opening Settings asks for nothing. Each joins as it loads, and one that fails adds
 * nothing.
 */
export function useServerDefined(wanted: boolean, groups: SectionGroup[]): Findable[] {
  const asks = (section: Section) =>
    wanted && groups.some((group) => group.items.includes(section));
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the built-in results show at once, and these join them as they load
  const personal = useQuery(
    UserService.method.listPersonalSettings,
    {},
    { enabled: asks("connections") },
  );
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- as above
  const qualities = useQuery(
    RequestService.method.listDefaultQualities,
    {},
    { enabled: asks("requests") },
  );
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- as above
  const home = useQuery(UserService.method.getHomeLayout, {}, { enabled: asks("home") });
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- as above
  const defaultHome = useQuery(
    ServerService.method.getDefaultHomeLayout,
    {},
    { enabled: asks("default-home") },
  );
  return [
    ...personalFindables(personal.data),
    ...qualityFindables(qualities.data),
    ...homeRowFindables("home", home.data),
    ...homeRowFindables("default-home", defaultHome.data),
  ];
}
