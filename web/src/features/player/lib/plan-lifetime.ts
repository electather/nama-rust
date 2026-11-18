/** How often an open plan asks for its URL. The Server stops a plan a minute after it was last asked
 * for, and a paused player, its buffer full, asks for nothing. */
const KEEP_ALIVE_MS = 30_000;

/** The plans a player let go of, by id, each to be stopped once the task that let go ends. */
const letGo = new Map<bigint, ReturnType<typeof setTimeout>>();

/** The plans stopped while their player was still open, by id, which it then needn't stop. */
const stoppedEarly = new Set<bigint>();

/**
 * Tells the Server the plan `planId` stopped while its player is still open, as before asking for
 * the plan that replaces it, so a Transcode it runs no longer counts toward the Server's limit.
 */
export async function stopPlan(tellStopped: (planId: bigint) => Promise<void>, planId: bigint) {
  stoppedEarly.add(planId);
  await tellStopped(planId);
}

/**
 * Asks for the plan `planId`'s `url` every `KEEP_ALIVE_MS`, until the page goes or the cleanup it
 * returns, either of which tells the Server it stopped, so it ends at once rather than a minute
 * later.
 */
export function keepPlaying(
  tellStopped: (planId: bigint) => Promise<void>,
  planId: bigint,
  url: string,
) {
  // React may clean up an effect and set it up again for the same plan, as `StrictMode` does on
  // mounting: a stop sent at once would end the plan the `<video>` goes on playing.
  clearTimeout(letGo.get(planId));
  letGo.delete(planId);
  const asking = setInterval(() => {
    void ask(url);
  }, KEEP_ALIVE_MS);
  // Closing or reloading the tab unmounts nothing, and a reloaded one no longer knows this plan,
  // whose Transcode would count toward the limit for that minute. A page the browser keeps to go
  // back to plays on when it does. ponytail: best effort, as the page may go before it's sent.
  const leaving = ({ persisted }: PageTransitionEvent) => {
    if (!persisted) {
      void stopPlan(tellStopped, planId);
    }
  };
  globalThis.addEventListener("pagehide", leaving);
  return () => {
    clearInterval(asking);
    globalThis.removeEventListener("pagehide", leaving);
    if (stoppedEarly.delete(planId)) {
      return;
    }
    const stop = setTimeout(() => {
      letGo.delete(planId);
      void tellStopped(planId);
    }, 0);
    letGo.set(planId, stop);
  };
}

/**
 * `HEAD`, so a Direct Play's whole file isn't sent. A failed ask is left to the player, which
 * notices when it next needs a segment.
 */
async function ask(url: string) {
  try {
    await fetch(url, { method: "HEAD" });
  } catch {
    // Left to the player.
  }
}
