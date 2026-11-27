import { AccountSection } from "@/features/auth/components/account-section";
import { HomeLayout } from "@/features/home/components/home-layout";
import { HubFeatured } from "@/features/home/components/hub-featured";
import { HubRows } from "@/features/home/components/hub-rows";
import { RowLoader } from "@/features/home/components/row-loader";
import { PageTitle } from "@/shared/components/page-title";

/**
 * Home at the Hub for the Account with `email` signed in: its Servers' Featured and rows merged,
 * then it.
 */
export function AccountHome({ email }: { email: string }) {
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
      <HubRows />
      <AccountSection email={email} />
    </HomeLayout>
  );
}
