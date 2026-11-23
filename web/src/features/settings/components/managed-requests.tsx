import { YourRequests } from "@/features/settings/components/your-requests";
import { usePendingRequests } from "@/features/settings/data/use-pending-requests";

/** A Request manager's Requests, opening on Pending while anything is, so they see what waits on them. */
export function ManagedRequests() {
  return <YourRequests opening={usePendingRequests() > 0 ? "pending" : "all"} />;
}
