import { type ReactNode, Suspense, lazy, useCallback, useRef, useState } from "react";

import { useSearchFromUrl } from "@/app/top-bar/use-search-from-url";
import { useActionBarKeys } from "@/features/action-bar/hooks/use-action-bar-keys";
import type { Frame, Opening } from "@/features/action-bar/lib/rows";

// Not on screen until it's asked for, so it stays out of the first page's code.
const ActionBar = lazy(async () => {
  const module = await import("@/features/action-bar/components/action-bar");
  return { default: module.ActionBar };
});

/**
 * The action bar, which its keys open from anywhere on a signed-in page, under what `children`
 * renders with the way to open it.
 */
export function ActionBarHost({ children }: { children: (onSearch: () => void) => ReactNode }) {
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
  useActionBarKeys({ dialog, open });
  return (
    <>
      {children(() => {
        open("root");
      })}
      {opening !== undefined && (
        <Suspense fallback={null}>
          <ActionBar
            ref={dialog}
            opening={opening}
            onClose={() => {
              setOpening(undefined);
            }}
          />
        </Suspense>
      )}
    </>
  );
}
