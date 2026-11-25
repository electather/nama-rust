import { useRouteContext } from "@tanstack/react-router";
import { useState } from "react";

import {
  type OwnChoice,
  type PlayFrom,
  type ServerChoice,
  choiceToPlay,
} from "@/features/title/lib/server-choices";

/**
 * Which Server Play plays from: the page's own, holding `own`, or one of `elsewhere`, by default
 * the one with the best Version until one is chosen. Nothing to play without `own`.
 */
export function usePlayFrom(
  own: OwnChoice,
  elsewhere: ServerChoice[],
): PlayFrom & { choice: ServerChoice };
export function usePlayFrom(own: OwnChoice | undefined, elsewhere: ServerChoice[]): PlayFrom;
export function usePlayFrom(own: OwnChoice | undefined, elsewhere: ServerChoice[]): PlayFrom {
  const { here } = useRouteContext({ from: "/_member/_server" });
  const [chosen, setChosen] = useState<string>();
  if (!own) {
    return { choices: [], choice: undefined, onChoose: setChosen };
  }
  const choices: [ServerChoice, ...ServerChoice[]] = [{ ...own, server: here }, ...elsewhere];
  return { choices, choice: choiceToPlay(choices, chosen), onChoose: setChosen };
}
