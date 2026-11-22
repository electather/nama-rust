import { Button } from "nama-ui/components/button";

import { Decision } from "@/features/auth/components/decision";

/** Names the Device asking to sign in, so it's signed in only once the User has seen which. */
export function Approve({
  deviceName,
  pending,
  onApprove,
  onCancel,
}: {
  deviceName: string;
  pending: boolean;
  onApprove: () => void;
  onCancel: () => void;
}) {
  return (
    <Decision
      heading={`Sign in “${deviceName}”?`}
      text={
        <p className="text-sm text-pretty">
          It’s asking to sign in as you. Sign it in only if it’s yours.
        </p>
      }
    >
      <Button size="lg" disabled={pending} onClick={onApprove}>
        {pending ? "Signing it in…" : "Sign it in"}
      </Button>
      <Button variant="outline" size="lg" onClick={onCancel}>
        Cancel
      </Button>
    </Decision>
  );
}
