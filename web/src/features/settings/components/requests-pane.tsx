import { DefaultQualities } from "@/features/settings/components/default-qualities";
import { ManagedRequests } from "@/features/settings/components/managed-requests";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { SectionBoundary } from "@/features/settings/components/section-boundary";
import { YourRequests } from "@/features/settings/components/your-requests";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { useMay } from "@/shared/hooks/use-may";

/**
 * The Quality the signed-in User's Requests come in, for a User who may choose one, over the
 * Requests they see, so a long list never buries it. A Request manager acts on them here too.
 */
export function RequestsPane() {
  const choosesQuality = useMay(Permission.CHOOSE_QUALITY);
  const manages = useMay(Permission.MANAGE_REQUESTS);
  const list = manages ? <ManagedRequests /> : <YourRequests opening="all" />;
  // Each part shows as soon as it has loaded.
  const defaults = (
    <SectionBoundary what="your default Quality" rows={2}>
      <DefaultQualities />
    </SectionBoundary>
  );
  return (
    <PaneFrame section="requests">
      <div className="flex flex-col gap-8">
        {choosesQuality && defaults}
        <SectionBoundary what="your Requests" rows={3}>
          {list}
        </SectionBoundary>
      </div>
    </PaneFrame>
  );
}
