import { Suspense } from "react";

import { InboxList } from "@/features/inbox/components/inbox-list";
import { ErrorBoundary } from "@/shared/components/error-boundary";

const note = "px-2.5 py-2 text-sm text-muted-foreground";

/** The bell menu's notifications, once they load, or why they didn't. */
export function InboxLoader() {
  return (
    <ErrorBoundary failed={<p className={note}>Your notifications couldn’t load just now.</p>}>
      <Suspense fallback={<p className={note}>Loading…</p>}>
        <InboxList />
      </Suspense>
    </ErrorBoundary>
  );
}
