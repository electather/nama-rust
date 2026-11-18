import { BackToLibraries } from "@/app/route-error/back-to-libraries";
import { Page } from "@/shared/components/page";

/** What the player shows once what it plays was deleted from the Server. */
export function DeletedWhilePlaying() {
  return (
    <Page heading="Deleted from the Server">
      <p className="mt-2 max-w-prose text-muted-foreground">
        Someone deleted what was playing, so it can’t play any more.
      </p>
      <BackToLibraries />
    </Page>
  );
}
