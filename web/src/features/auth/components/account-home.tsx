import { HomeLayout } from "@/features/home/components/home-layout";
import { HubRows } from "@/features/home/components/hub-rows";
import { ServerNotices } from "@/features/servers/components/server-notices";
import { PageTitle } from "@/shared/components/page-title";

/** Home at the Hub: why any of the Account's Servers can't show, then their rows merged. */
export function AccountHome() {
  return (
    <HomeLayout hero={null}>
      <PageTitle name="Home" />
      <ServerNotices />
      <HubRows />
    </HomeLayout>
  );
}
