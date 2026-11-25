import { useRouteContext } from "@tanstack/react-router";
import { useState } from "react";

import { type Copy, copyToPlay, type PlayFrom } from "@/features/title/lib/copies";

/**
 * Which Server Play plays from: the page's own, holding `here`, or one of `elsewhere`, by default
 * the one with the best Version until one is chosen. Nothing to play without `here`.
 */
export function usePlayFrom(here: Omit<Copy, "server"> | undefined, elsewhere: Copy[]): PlayFrom {
  const { here: server } = useRouteContext({ from: "/_member/_server" });
  const [chosen, setChosen] = useState<string>();
  if (!here) {
    return { copies: [], copy: undefined, onChoose: setChosen };
  }
  const copies: [Copy, ...Copy[]] = [{ ...here, server }, ...elsewhere];
  return { copies, copy: copyToPlay(copies, chosen), onChoose: setChosen };
}
