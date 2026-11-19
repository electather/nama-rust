import { cn } from "cn";
import { Clock, Info, TriangleAlert } from "lucide-react";
import type { ReactNode } from "react";

const looks = {
  // Nothing went wrong: it's waiting on time, such as too many tries.
  wait: { Icon: Clock, box: "bg-secondary text-foreground", icon: "" },
  // The Server couldn't be reached, or refused for a reason no field explains.
  error: { Icon: TriangleAlert, box: "bg-destructive/10 text-destructive", icon: "" },
  info: { Icon: Info, box: "border text-foreground", icon: "text-muted-foreground" },
};

/** A message for a whole sign-in form, with the icon its kind takes. */
export function Notice({ kind, children }: { kind: keyof typeof looks; children: ReactNode }) {
  const { Icon, box, icon } = looks[kind];
  return (
    <p
      role="alert"
      className={cn("flex items-start gap-2.5 rounded-xl px-3.5 py-2.5 text-sm", box)}
    >
      <Icon aria-hidden className={cn("mt-0.5 size-4 shrink-0", icon)} />
      <span>{children}</span>
    </p>
  );
}
