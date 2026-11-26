import { HubRowList } from "@/features/home/components/hub-row-list";
import { RowLoader } from "@/features/home/components/row-loader";

/**
 * Home's rows at the Hub, each kind merged across the Account's Servers, or nothing while its
 * Servers are listed or if they can't be: the Account's own section says so.
 */
export function HubRows() {
  return (
    <RowLoader loading={null}>
      <HubRowList />
    </RowLoader>
  );
}
