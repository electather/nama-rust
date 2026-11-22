import { type ComponentProps, Suspense, lazy, useCallback, useRef, useState } from "react";

import { TopBar } from "@/app/top-bar/top-bar";
import { useSearchFromUrl } from "@/app/top-bar/use-search-from-url";
import { useActionBarKeys } from "@/features/action-bar/hooks/use-action-bar-keys";
import type { Frame, Opening } from "@/features/action-bar/lib/rows";

// Not on screen until it's asked for, so it stays out of the first page's code.
const ActionBar = lazy(async () => {
  const module = await import("@/features/action-bar/components/action-bar");
  return { default: module.ActionBar };
});

/** What sits over every signed-in page: the top bar, and the action bar it and the keys open. */
export function MemberChrome(props: Omit<ComponentProps<typeof TopBar>, "onSearch">) {
  const asked = useSearchFromUrl();
  // How the action bar opened, while it's open. A link to the old search page loads the app with
  // the words to open on, so the first render reads them.
  const [opening, setOpening] = useState<Opening | undefined>(() =>
    asked === undefined ? undefined : { frame: "root", query: asked },
  );
  const dialog = useRef<HTMLDialogElement>(null);
  const open = useCallback((frame: Frame) => {
    setOpening({ frame, query: "" });
  }, []);
  useActionBarKeys({ dialog, adminSection: props.adminSection, open });
  return (
    <>
      <TopBar
        {...props}
        onSearch={() => {
          open("root");
        }}
      />
      {opening !== undefined && (
        <Suspense fallback={null}>
          <ActionBar
            ref={dialog}
            opening={opening}
            adminSection={props.adminSection}
            onClose={() => {
              setOpening(undefined);
            }}
          />
        </Suspense>
      )}
    </>
  );
}
