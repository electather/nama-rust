import { lazy, type ReactNode, Suspense, useState } from "react";

import { Brand } from "@/shared/components/sign-in-layout/brand";
import { WarpContext } from "@/shared/components/sign-in-layout/warp";

// WebGL and its shaders load after the card, which needs none of it.
const Lightfall = lazy(async () => {
  const module = await import("@/shared/components/sign-in-layout/lightfall");
  return { default: module.Lightfall };
});

/**
 * Every signed-out page: the mark over the page's card, on falling light. The light stays put
 * from one page to the next, rushing while a page says so with `useWarp`.
 */
export function SignedOutLayout({ children }: { children: ReactNode }) {
  const [warp, setWarp] = useState(false);
  return (
    <div className="relative isolate flex min-h-dvh flex-col items-center justify-center px-5 py-12">
      <Suspense>
        <Lightfall warp={warp} />
      </Suspense>
      <div className="relative flex w-full max-w-110 flex-col gap-7.5">
        <Brand />
        <WarpContext value={setWarp}>{children}</WarpContext>
      </div>
    </div>
  );
}
