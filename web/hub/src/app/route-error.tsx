import { PageHeading } from "@hub/app/page-heading";
import { useRouter } from "@tanstack/react-router";
import { Button } from "nama-ui/components/button";

/** What a Hub page shows instead when it fails, in plain words and never an error code. */
export function RouteError() {
  const router = useRouter();
  return (
    <>
      <PageHeading>Something went wrong</PageHeading>
      <p className="text-center text-base text-pretty">
        This page couldn’t load. Check your connection, then try again.
      </p>
      <Button
        size="lg"
        onClick={() => {
          void router.invalidate();
        }}
      >
        Try again
      </Button>
    </>
  );
}
