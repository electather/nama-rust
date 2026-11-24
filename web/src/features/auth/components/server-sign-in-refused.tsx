import { FormNotices } from "@/shared/components/sign-in-layout/form-notices";
import { SignInCard } from "@/shared/components/sign-in-layout/sign-in-card";
import type { FormErrors } from "@/shared/lib/forms";

/** A sign-in to a Server that can't go ahead, saying why. */
export function ServerSignInRefused({ heading, errors }: { heading: string; errors: FormErrors }) {
  return (
    <SignInCard heading={heading}>
      <FormNotices errors={errors} />
    </SignInCard>
  );
}
