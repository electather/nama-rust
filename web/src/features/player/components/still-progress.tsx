import { Progress } from "@/shared/ui/progress";

/** How far into an Episode you are, along the foot of its Still, once you've started it. */
export function StillProgress({ percent }: { percent: number }) {
  if (percent <= 0) {
    return null;
  }
  return (
    <span className="absolute inset-x-2 bottom-2">
      <Progress value={percent} aria-label="Watched" />
    </span>
  );
}
