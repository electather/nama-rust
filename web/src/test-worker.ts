/**
 * A Worker, which happy-dom lacks, running the ticker the player's keep-alive counts in
 * (`usePlanLifetime`) on the page's timers, which a test can fake.
 */
export function tickWorker() {
  const worker = new EventTarget();
  let ticking: ReturnType<typeof setInterval> | undefined;
  return Object.assign(worker, {
    postMessage: (ms: number) => {
      ticking = setInterval(() => {
        worker.dispatchEvent(new MessageEvent("message"));
      }, ms);
    },
    terminate: () => {
      clearInterval(ticking);
    },
  });
}
