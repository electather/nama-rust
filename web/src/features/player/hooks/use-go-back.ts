import { type LinkOptions, useRouter } from "@tanstack/react-router";

/**
 * What leaves the player: back to the page that opened it, or to `back`, the title page, in the
 * player's place when it was opened by its link, so browser Back never opens the player again.
 */
export function useGoBack(back: LinkOptions) {
  const router = useRouter();
  return () => {
    if (router.history.canGoBack()) {
      router.history.back();
    } else {
      void router.navigate({ ...back, replace: true });
    }
  };
}
