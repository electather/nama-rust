declare module "react" {
  interface CSSProperties {
    /** How far through a title the User is, as a poster's bar draws it. */
    "--through"?: string;
  }
}

/** A bar along a poster's foot of how far through it the User is, once started. */
export function PosterProgress({ percent }: { percent: number }) {
  if (percent <= 0) {
    return null;
  }
  return (
    <span
      aria-hidden
      className="absolute inset-x-3 bottom-3 h-1 overflow-hidden rounded-full bg-foreground/25 before:block before:h-full before:w-(--through) before:rounded-full before:bg-primary"
      style={{ "--through": `${percent}%` }}
    />
  );
}
