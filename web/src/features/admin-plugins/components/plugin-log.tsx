import { LogSection } from "@/features/admin-plugins/components/log-section";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { useMay } from "@/shared/hooks/use-may";

/**
 * For an Admin, the last lines the Plugin `id` logged, newest first, as the Server keeps them while
 * it's enabled.
 */
export function PluginLog({ id }: { id: string }) {
  return useMay(Permission.ADMIN) && <LogSection id={id} />;
}
