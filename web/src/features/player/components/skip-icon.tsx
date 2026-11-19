import { SEEK_STEP_SECONDS } from "@/features/player/lib/video-actions";

/** A circling arrow with the seconds it skips inside: Lucide has none with a number. */
export function SkipIcon({ forward }: { forward: boolean }) {
  return (
    <svg
      aria-hidden
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      strokeLinejoin="round"
      // The button sizes icons at 20px; these carry a number, so they're drawn larger.
      className="size-6.5!"
    >
      <path d={forward ? FORWARD_ARC : BACK_ARC} />
      <path d={forward ? "M21 3v5h-5" : "M3 3v5h5"} />
      <text
        x={forward ? 11.5 : 12.5}
        y="15.2"
        textAnchor="middle"
        fill="currentColor"
        stroke="none"
        fontSize="7.5"
        fontWeight="700"
      >
        {SEEK_STEP_SECONDS}
      </text>
    </svg>
  );
}

const BACK_ARC = "M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8";
const FORWARD_ARC = "M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8";
