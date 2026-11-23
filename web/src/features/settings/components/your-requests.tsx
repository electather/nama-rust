import { useState, useTransition } from "react";

import { RequestList } from "@/features/requests/components/request-list";
import { StateFilterMenu } from "@/features/requests/components/state-filter-menu";
import type { StateFilter } from "@/features/requests/lib/state-filter";
import { PaneSection } from "@/features/settings/components/pane-section";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { useMay } from "@/shared/hooks/use-may";

/**
 * The Requests and Followed Shows the signed-in User sees, everyone's if they may view them all,
 * under the menu that picks a state, `opening` on the one it starts on.
 */
export function YourRequests({ opening }: { opening: StateFilter }) {
  const [filter, setFilter] = useState(opening);
  // The list stays while another state's loads, rather than the pane falling back to its skeleton.
  const [, startTransition] = useTransition();
  const title = useMay(Permission.VIEW_REQUESTS) ? "All Requests" : "Your Requests";
  const menu = (
    <StateFilterMenu
      chosen={filter}
      choose={(chosen) => {
        startTransition(() => {
          setFilter(chosen);
        });
      }}
    />
  );
  return (
    <PaneSection title={title} action={menu}>
      <RequestList label={title} filter={filter} />
    </PaneSection>
  );
}
