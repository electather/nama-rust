import { useQueryErrorResetBoundary } from "@tanstack/react-query";

/** Lets a failed search load again once its error boundary resets, rather than throw again. */
export function useResetSearchError() {
  return useQueryErrorResetBoundary().reset;
}
