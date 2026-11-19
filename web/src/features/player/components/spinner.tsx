/** A ring that turns while something loads. Decoration: what loads is said in words beside it. */
export function Spinner() {
  return (
    <svg
      viewBox="0 0 44 44"
      fill="none"
      strokeWidth="4"
      aria-hidden
      className="size-11 animate-spin motion-reduce:animate-spin-slow"
    >
      <circle cx="22" cy="22" r="18" className="stroke-muted" />
      <path d="M22 4a18 18 0 0 1 18 18" strokeLinecap="round" className="stroke-primary" />
    </svg>
  );
}
