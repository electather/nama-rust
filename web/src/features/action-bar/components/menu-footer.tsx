import { ChevronDown, ChevronUp, CornerDownLeft } from "lucide-react";

import { KeyHint } from "@/features/action-bar/components/key-hint";
import { Logo } from "@/shared/components/logo";

const navigate = [
  { name: "up", key: <ChevronUp aria-hidden strokeWidth={1.5} className="size-3" /> },
  { name: "down", key: <ChevronDown aria-hidden strokeWidth={1.5} className="size-3" /> },
];
const select = [
  { name: "enter", key: <CornerDownLeft aria-hidden strokeWidth={1.5} className="size-3" /> },
];
const close = [{ name: "esc", key: "Esc" }];

/** The action bar's keys, left out on a touch screen, and the mark. */
export function MenuFooter() {
  return (
    <footer className="flex items-center justify-between gap-3 border-t bg-card/40 px-3 py-2 text-xs text-muted-foreground">
      <div className="inline-flex gap-3.5 pointer-coarse:hidden">
        <KeyHint keys={navigate} label="Navigate" />
        <KeyHint keys={select} label="Select" />
        <KeyHint keys={close} label="Close" />
      </div>
      <Logo size={24} label="Nama" className="ms-auto" />
    </footer>
  );
}
