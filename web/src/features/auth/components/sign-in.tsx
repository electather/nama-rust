import { useState } from "react";

import { CodeSignIn } from "@/features/auth/components/code-sign-in";
import { PasswordSignIn } from "@/features/auth/components/password-sign-in";
import { SwitchSignInWay } from "@/features/auth/components/switch-sign-in-way";
import { useFinishSignIn } from "@/features/auth/data/use-finish-sign-in";

/** The sign-in page: a password, or a code approved on another Device. */
export function SignIn({ redirect }: { redirect: string | undefined }) {
  const [withCode, setWithCode] = useState(false);
  const signedIn = useFinishSignIn(redirect);
  const otherWay = (label: string) => (
    <SwitchSignInWay
      label={label}
      onSwitch={() => {
        setWithCode(!withCode);
      }}
    />
  );
  return withCode ? (
    <CodeSignIn onSignedIn={signedIn} footer={otherWay("Sign in with a password")} />
  ) : (
    <PasswordSignIn onSignedIn={signedIn} footer={otherWay("Sign in with a code")} />
  );
}
