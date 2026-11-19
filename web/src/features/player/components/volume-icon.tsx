// Lucide's Volume2 and VolumeX in one drawing, so its waves and cross fade between levels rather
// than one icon swapping for the other.
const FADE = "transition duration-200 ease-out-expo motion-reduce:transition-none";

/** A wave's classes: lit while `heard` is above `from`, dimmed below, and gone at silence. */
function wave(heard: number, from: number) {
  if (heard === 0) {
    return `${FADE} opacity-0`;
  }
  return heard > from ? FADE : `${FADE} text-muted-foreground`;
}

/**
 * The speaker at volume `heard`, from 0 to 1: both waves lit above half, the outer one dimmed at
 * half or below, and crossed out at none.
 */
export function VolumeIcon({ heard }: { heard: number }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M11 4.702a.705.705 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.416a1.4 1.4 0 0 1 .997.413l3.383 3.384A.705.705 0 0 0 11 19.298z" />
      <path d="M16 9a5 5 0 0 1 0 6" className={wave(heard, 0)} />
      <path d="M19.364 18.364a9 9 0 0 0 0-12.728" className={wave(heard, 0.5)} />
      <path
        d="M22 9l-6 6M16 9l6 6"
        className={`${FADE} ${heard === 0 ? "opacity-100" : "opacity-0"}`}
      />
    </svg>
  );
}
