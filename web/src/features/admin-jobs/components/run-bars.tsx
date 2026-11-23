import { fillOf, tookMs } from "@/features/admin-jobs/lib/run-chart";
import { dayAndTime, duration, usualMs } from "@/features/admin-jobs/lib/run-times";
import type { JobRun } from "@/gen/nama/v1/job_pb";

const WIDTH = 400;
const HEIGHT = 56;
const GAP = 4;

/**
 * A bar for each of `ended`, oldest first, as tall as it took next to the longest, its time and how
 * long on hover, and a dashed line at how long it usually takes.
 */
export function RunBars({ ended }: { ended: JobRun[] }) {
  const longest = Math.max(...ended.map((run) => tookMs(run)));
  const width = (WIDTH - GAP * (ended.length - 1)) / ended.length;
  const bars = ended.map((run, index) => {
    const height = Math.max(3, (tookMs(run) / longest) * HEIGHT);
    return (
      <rect
        key={run.id.toString()}
        x={index * (width + GAP)}
        y={HEIGHT - height}
        width={width}
        height={height}
        rx={2}
        className={fillOf(run.outcome)}
      >
        <title>{`${dayAndTime(run.startedAtMs)}: ${duration(run.startedAtMs, run.endedAtMs ?? run.startedAtMs)}`}</title>
      </rect>
    );
  });
  const usualY = HEIGHT - ((usualMs(ended) ?? 0) / longest) * HEIGHT;
  return (
    // The chart's caption says what matters; each bar's time and length is there on hover too.
    <svg
      aria-hidden
      viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
      preserveAspectRatio="none"
      className="h-14 w-full overflow-visible"
    >
      {bars}
      <line
        x1={0}
        x2={WIDTH}
        y1={usualY}
        y2={usualY}
        strokeDasharray="3 3"
        vectorEffect="non-scaling-stroke"
        className="stroke-muted-foreground"
      />
    </svg>
  );
}
