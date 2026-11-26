import { type LinkOptions, useRouter } from "@tanstack/react-router";

/**
 * What leaves the player: back to the page that opened it, or, when its link opened it, to
 * `titlePage` in its place, so browser Back never opens the player again.
 */
export function useGoBack(titlePage: LinkOptions) {
  const router = useRouter();
  return () => {
    if (router.history.canGoBack()) {
      router.history.back();
    } else {
      void router.navigate({ ...titlePage, replace: true });
    }
  };
}
