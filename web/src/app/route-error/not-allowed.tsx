import { BackToLibraries } from "@/app/route-error/back-to-libraries";
import { Page } from "@/shared/components/page";

/** What a page needing a Permission the User lacks shows instead. */
export function NotAllowed() {
  return (
    <Page heading="You can’t open this page">
      <p className="mt-2 max-w-prose text-muted-foreground">
        It needs a Permission you don’t have. If you need something changed here, ask whoever runs
        the Server.
      </p>
      <BackToLibraries />
    </Page>
  );
}
