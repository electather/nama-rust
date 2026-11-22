import { Link } from "@tanstack/react-router";
import { buttonVariants } from "nama-ui/components/button";

import { Decision } from "@/features/auth/components/decision";

/** Says the Device is signed in, and offers to sign in another. */
export function Approved({ deviceName }: { deviceName: string }) {
  return (
    <Decision
      heading={`“${deviceName}” is signed in`}
      text={
        <output className="text-sm text-pretty">
          It may take a few seconds to notice. You can sign it out any time in Settings.
        </output>
      }
    >
      <Link to="/" className={buttonVariants({ size: "lg" })}>
        Back to browsing
      </Link>
      <Link to="/code" className={buttonVariants({ variant: "outline", size: "lg" })}>
        Sign in another Device
      </Link>
    </Decision>
  );
}
