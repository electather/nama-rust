import { Switch as SwitchPrimitive } from "@base-ui/react/switch";
import { motion, useAnimate, useReducedMotion } from "motion/react";
import { motionTokens } from "nama-ui/lib/motion-tokens";
import { useEffect, useRef, useState } from "react";

// uiarc's switch (uiarc.dev/components/switch) on Base UI: pressing stretches the thumb toward the
// other side, and releasing sends it across on a spring. The Rim off, Near-white on.

/** The thumb's size and its travel, in px: the track's `w-10.5` less its `p-0.75` either side and the thumb. */
const size = 18;
const travel = 18;
/** How far the thumb widens toward the other side while pressed. */
const stretch = 5;
/** Critically damped, so the thumb lands on its end without overshooting the state it reports. */
const glide = motionTokens.spring.smooth;
/** A toggle this soon after a release came from that press, which already stretched the thumb. */
const pressWindowMs = 250;

type RootProps = SwitchPrimitive.Root.Props;
type EventOf<K extends keyof RootProps> = Parameters<NonNullable<RootProps[K]>>[0];

function usePress(props: RootProps) {
  const [pressed, setPressed] = useState(false);
  const releasedAt = useRef(Number.NEGATIVE_INFINITY);
  const release = () => {
    if (pressed && props.disabled !== true) {
      releasedAt.current = performance.now();
    }
    setPressed(false);
  };
  const cancel = () => {
    setPressed(false);
  };
  const handlers = {
    onPointerDown: (event: EventOf<"onPointerDown">) => {
      props.onPointerDown?.(event);
      if (event.button === 0) {
        setPressed(true);
      }
    },
    onPointerUp: (event: EventOf<"onPointerUp">) => {
      props.onPointerUp?.(event);
      release();
    },
    onPointerLeave: (event: EventOf<"onPointerLeave">) => {
      props.onPointerLeave?.(event);
      cancel();
    },
    onPointerCancel: (event: EventOf<"onPointerCancel">) => {
      props.onPointerCancel?.(event);
      cancel();
    },
    onKeyDown: (event: EventOf<"onKeyDown">) => {
      props.onKeyDown?.(event);
      if (event.key === " ") {
        setPressed(true);
      }
    },
    onKeyUp: (event: EventOf<"onKeyUp">) => {
      props.onKeyUp?.(event);
      release();
    },
    onBlur: (event: EventOf<"onBlur">) => {
      props.onBlur?.(event);
      cancel();
    },
  };
  return { pressed, releasedAt, handlers };
}

/** A switch for a setting that takes effect as soon as it's flipped. */
function Switch({
  label,
  checked,
  defaultChecked,
  onCheckedChange,
  ...props
}: SwitchPrimitive.Root.Props & {
  /** Shown beside the track, and its aria-label when none is passed. */
  label?: string | undefined;
}) {
  const reduceMotion = useReducedMotion() === true;
  const [internal, setInternal] = useState(defaultChecked ?? false);
  const on = checked ?? internal;
  const { pressed, releasedAt, handlers } = usePress(props);
  const [thumb, animate] = useAnimate<HTMLSpanElement>();
  const shown = useRef(on);
  useEffect(() => {
    const toggled = shown.current !== on;
    shown.current = on;
    const fromPress = performance.now() - releasedAt.current < pressWindowMs;
    // A toggle with no press, such as from a label, gets the stretch over the glide's time instead.
    const controls =
      toggled && !reduceMotion && !fromPress
        ? animate(
            thumb.current,
            { scaleX: [1, 1.16, 1] },
            { duration: glide.visualDuration, times: [0, 0.4, 1], ease: ["easeOut", "easeInOut"] },
          )
        : undefined;
    return () => {
      controls?.stop();
    };
  }, [on, reduceMotion, releasedAt, animate, thumb]);
  const extra = pressed && !reduceMotion && props.disabled !== true ? stretch : 0;

  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className="group inline-flex shrink-0 cursor-pointer items-center gap-3 text-sm outline-none data-disabled:cursor-not-allowed data-disabled:opacity-50"
      aria-label={props["aria-label"] ?? label}
      {...props}
      {...handlers}
      checked={on}
      onCheckedChange={(next, details) => {
        if (checked === undefined) {
          setInternal(next);
        }
        onCheckedChange?.(next, details);
      }}
    >
      {/* The track mirrors in right-to-left, so the thumb's travel needs no sign of its own. */}
      <span className="relative isolate flex h-6 w-10.5 shrink-0 items-center rounded-full bg-input/80 p-0.75 transition-colors duration-150 ease-out-expo group-hover:bg-input group-focus-visible:ring-3 group-focus-visible:ring-ring/50 before:absolute before:inset-0 before:-z-1 before:rounded-full before:bg-primary before:opacity-0 before:transition-opacity before:duration-200 before:ease-out-expo group-data-checked:before:opacity-100 motion-reduce:transition-none motion-reduce:before:transition-none rtl:-scale-x-100">
        {/* Stretched, the thumb keeps its far edge where it was, like a held finger. */}
        <motion.span
          ref={thumb}
          data-slot="switch-thumb"
          className="block size-4.5 shrink-0 rounded-full bg-foreground shadow-sm transition-colors duration-200 ease-out-expo will-change-transform group-data-checked:bg-primary-foreground motion-reduce:transition-none"
          initial={false}
          animate={{ x: on ? travel - extra : 0, width: size + extra }}
          transition={
            reduceMotion ? { duration: 0 } : { x: glide, width: motionTokens.spring.snappy }
          }
        />
      </span>
      {label === undefined ? null : <span>{label}</span>}
    </SwitchPrimitive.Root>
  );
}

export { Switch };
