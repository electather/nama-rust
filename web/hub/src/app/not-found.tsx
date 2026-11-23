import { PageHeading } from "@hub/app/page-heading";

/** What an address with nothing at it shows. */
export function NotFound() {
  return (
    <>
      <PageHeading>Page not found</PageHeading>
      <p className="text-center text-base text-pretty">There’s nothing at this address.</p>
    </>
  );
}
