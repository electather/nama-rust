import { Button } from "@/shared/ui/button";

/** The button under a sign-in form that swaps it for the other way to sign in. */
export function SwitchSignInWay({ label, onSwitch }: { label: string; onSwitch: () => void }) {
  return (
    <Button variant="ghost" onClick={onSwitch}>
      {label}
    </Button>
  );
}
