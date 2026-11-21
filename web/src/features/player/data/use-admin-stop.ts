import { useQuery } from "@tanstack/react-query";

import { adminStopQuery } from "@/shared/hooks/admin-stop-query";

/** How an Admin stopped the plan `planId`, once an Event says one did. */
export function useAdminStop(planId: bigint) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- only Events fill it, so there's no fetch to wait for
  return useQuery(adminStopQuery(planId)).data;
}
