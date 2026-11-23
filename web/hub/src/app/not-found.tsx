import { PageHeading } from "@hub/app/page-heading";
import { AccountOrSignInLink } from "@hub/features/account/components/account-or-sign-in-link";

/** What an address with nothing at it shows, with the way back. */
export function NotFound() {
  return (
    <>
      <PageHeading>Page not found</PageHeading>
      <p className="text-center text-base text-pretty">There’s nothing at this address.</p>
      <AccountOrSignInLink />
    </>
  );
}
