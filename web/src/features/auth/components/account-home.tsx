import { AccountSection } from "@/features/auth/components/account-section";
import { HomeLayout } from "@/features/home/components/home-layout";
import { HubRows } from "@/features/home/components/hub-rows";
import { PageTitle } from "@/shared/components/page-title";

/** Home at the Hub for the Account with `email` signed in: its Servers' rows merged, then it. */
export function AccountHome({ email }: { email: string }) {
  return (
    <HomeLayout hero={null}>
      <PageTitle name="Home" />
      <HubRows />
      <AccountSection email={email} />
    </HomeLayout>
  );
}
