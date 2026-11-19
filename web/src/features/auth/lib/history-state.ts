import type { FormErrors } from "@/shared/lib/forms";

/** A Sign-in Code this browser shows while it waits for approval. */
export interface WaitingCode {
  code: string;
  /** What polls for the session once it's approved, so it never goes in the address. */
  handle: string;
  deviceName: string;
  /** When it expires, in milliseconds since the epoch. */
  expiresAt: number;
}

// What the sign-in steps hand the next one. It's kept per history entry, so back, forward and
// reloading each find their own.
declare module "@tanstack/history" {
  interface HistoryState {
    /** For `/sign-in/code/waiting`. */
    waitingCode?: WaitingCode;
    /** For `/code/approve`: the code looked up, and the Device it signs in. */
    lookedUp?: { code: string; deviceName: string };
    /** For `/code`, after the Server refused to approve `code`: why. */
    refusedCode?: { code: string; errors: FormErrors };
    /** For `/code/done`: the Device just signed in. */
    signedInDevice?: string;
  }
}
