import { PageHeading } from "@hub/app/page-heading";
import { AccountOrSignInLink } from "@hub/features/account/components/account-or-sign-in-link";
import { Suspense } from "react";

/** What an address with nothing at it shows, with the way back. */
export function NotFound() {
  return (
    <>
      <PageHeading>Page not found</PageHeading>
      <p className="text-center text-base text-pretty">There’s nothing at this address.</p>
      {/* The heading shows at once, without waiting for the Hub to say who's signed in. */}
      <Suspense>
        <AccountOrSignInLink />
      </Suspense>
    </>
  );
}
