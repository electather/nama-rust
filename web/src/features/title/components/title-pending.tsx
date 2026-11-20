/**
 * A title's page while it loads: the shape of its top, with where its heading, facts and Play will
 * be, so nothing jumps when they come.
 */
export function TitlePending() {
  const bar = "animate-pulse rounded-full bg-muted motion-reduce:animate-none";
  return (
    <main aria-hidden className="-mt-18 flex min-h-184 flex-col justify-end bg-card pt-28">
      <div className="mx-auto flex w-full max-w-350 flex-col gap-3.5 px-5 pb-10">
        <span className={`h-10 w-80 max-w-full ${bar}`} />
        <span className={`h-4 w-56 ${bar}`} />
        <span className={`h-4 w-140 max-w-full ${bar}`} />
        <span className={`mt-2.5 h-11 w-32 ${bar}`} />
      </div>
    </main>
  );
}
