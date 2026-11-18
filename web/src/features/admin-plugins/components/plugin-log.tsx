import { useRouteContext } from "@tanstack/react-router";

import { LogSection } from "@/features/admin-plugins/components/log-section";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";

/**
 * For an Admin, the last lines the Plugin `id` logged, newest first, as the Server keeps them while
 * it's enabled.
 */
export function PluginLog({ id }: { id: string }) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  return may(signedIn.user, Permission.ADMIN) && <LogSection id={id} />;
}
