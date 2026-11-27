import { HomeLayout } from "@/features/home/components/home-layout";
import { HubFeatured } from "@/features/home/components/hub-featured";
import { HubRows } from "@/features/home/components/hub-rows";
import { RowLoader } from "@/features/home/components/row-loader";
import { ServerNotices } from "@/features/servers/components/server-notices";
import { PageTitle } from "@/shared/components/page-title";

/**
 * Home at the Hub: its Servers' Featured, why any of them can't show, then their rows merged.
 */
export function AccountHome() {
  return (
    <HomeLayout
      hero={
        // Nothing while its Servers are listed, as for the rows.
        <RowLoader loading={null}>
          <HubFeatured />
        </RowLoader>
      }
    >
      <PageTitle name="Home" />
      <ServerNotices />
      <HubRows />
    </HomeLayout>
  );
}
