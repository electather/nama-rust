import { Page } from "@/shared/components/page";

/** What an address with nothing at it shows. */
export function NotFound() {
  return (
    <Page heading="Page not found">
      <p className="mt-2 text-muted-foreground">There’s nothing at this address.</p>
    </Page>
  );
}
