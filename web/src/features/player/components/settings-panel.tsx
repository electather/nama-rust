import { Settings } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { Popover, PopoverContent, PopoverTrigger } from "nama-ui/components/popover";
import { type ComponentProps, useRef, useState } from "react";

import { SettingsViews } from "@/features/player/components/settings-views";
import { PANEL_GAP, type PanelPlace } from "@/features/player/lib/panel-place";
import { type SettingsView, type SlideFrom, stepsBack } from "@/features/player/lib/settings-view";

/** What Playback settings shows, besides where it is. */
export type ViewProps = Omit<
  ComponentProps<typeof SettingsViews>,
  "view" | "from" | "go" | "close"
>;

/**
 * Playback settings: Speed and the Playback Limit, each opening its list in place, autoplay for an
 * Episode, and Playback info. Esc in a list goes back to the rows, and from there closes it.
 */
export function SettingsPanel({ place, ...props }: ViewProps & { place: PanelPlace }) {
  const [open, setOpen] = useState(false);
  const [view, setView] = useState<SettingsView>("settings");
  const [from, setFrom] = useState<SlideFrom>();
  const popupRef = useRef<HTMLDivElement>(null);
  const go = (next: SettingsView) => {
    setView(next);
    setFrom(next === "settings" ? "start" : "end");
    // The rows it replaced had focus, so the keyboard carries on in the new ones.
    requestAnimationFrame(() => {
      popupRef.current?.querySelector<HTMLElement>("button")?.focus();
    });
  };
  return (
    <Popover
      open={open}
      onOpenChange={(next, { reason }) => {
        if (stepsBack(next, reason, view)) {
          go("settings");
          return;
        }
        setOpen(next);
      }}
      // Back to the rows once closed, so it opens on them, without changing as it fades.
      onOpenChangeComplete={() => {
        setView("settings");
        setFrom(undefined);
      }}
    >
      <PopoverTrigger
        render={
          <Button
            variant="overlay"
            size="icon"
            aria-label="Playback settings"
            title="Playback settings"
          />
        }
      >
        <Settings aria-hidden />
      </PopoverTrigger>
      <PopoverContent
        ref={popupRef}
        width={view === "info" ? "text" : "menu"}
        {...place}
        sideOffset={PANEL_GAP}
        aria-label="Playback settings"
      >
        <SettingsViews
          key={view}
          view={view}
          from={from}
          go={go}
          close={() => {
            setOpen(false);
          }}
          {...props}
        />
      </PopoverContent>
    </Popover>
  );
}
