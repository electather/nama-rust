import { useQueryErrorResetBoundary } from "@tanstack/react-query";

/** Lets the reads that failed under an error boundary run again once it mounts afresh. */
export function useRetryReads() {
  return useQueryErrorResetBoundary().reset;
}
