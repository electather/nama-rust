import { Code, type ConnectError } from "@connectrpc/connect";
import { useMutation } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";
import { useEffect } from "react";

import { useSensitiveMutation } from "@/components/confirm-its-you";
import { FormEnd, FormField } from "@/components/form-field";
import { Page } from "@/components/page";
import { Button } from "@/components/ui/button";
import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { type FormErrors, formText, onSubmitted, refusal } from "@/lib/forms";

export const Route = createFileRoute("/_member/code")({
  // `?code=` is what a phone opens by scanning the QR code a signing-in Device shows.
  validateSearch: (search): { code?: string | undefined } => ({
    code: typeof search["code"] === "string" ? search["code"] : undefined,
  }),
  component: EnterCode,
});

/** Signing in another Device: look up the Sign-in Code it shows, check its name, then approve. */
function EnterCode() {
  const { code: scanned } = Route.useSearch();
  const navigate = Route.useNavigate();
  const lookUp = useMutation(SignInService.method.lookUpSignInCode);
  // `mutate` keeps its identity across renders, unlike `lookUp`, so a re-render doesn't look the scanned code up again.
  const { mutate } = lookUp;
  useEffect(() => {
    if (scanned !== undefined) {
      mutate({ code: scanned });
    }
  }, [scanned, mutate]);
  const startOver = () => {
    lookUp.reset();
    // Otherwise the next code's field would start with this one.
    void navigate({ search: {} });
  };
  return (
    <Page heading="Sign in a Device">
      <section className="mt-2 max-w-sm">
        {lookUp.data === undefined ? (
          <LookUp
            scanned={scanned}
            onSubmit={(code) => {
              lookUp.mutate({ code });
            }}
            pending={lookUp.isPending}
            error={lookUp.error}
          />
        ) : (
          <Approve
            code={lookUp.variables.code ?? ""}
            deviceName={lookUp.data.deviceName}
            onDone={startOver}
          />
        )}
      </section>
    </Page>
  );
}

function LookUp({
  scanned,
  onSubmit,
  pending,
  error,
}: {
  scanned: string | undefined;
  onSubmit: (code: string) => void;
  pending: boolean;
  error: ConnectError | null;
}) {
  const errors = error === null ? {} : codeErrors(error);
  return (
    <>
      <p className="text-muted-foreground">
        Enter the Sign-in Code the other Device shows to sign it in as you.
      </p>
      <form
        className="mt-6 flex flex-col gap-5"
        onSubmit={onSubmitted((typed) => {
          onSubmit(formText(typed, "signInCode"));
        })}
      >
        <FormField
          label="Sign-in Code"
          name="signInCode"
          placeholder="XXXX-XXXX"
          defaultValue={scanned}
          autoComplete="off"
          autoCapitalize="characters"
          spellCheck={false}
          error={errors.signInCode}
        />
        <FormEnd
          action={pending ? "Looking it up…" : "Continue"}
          pending={pending}
          done={undefined}
          refusal={errors.form}
        />
      </form>
    </>
  );
}

/** Names the Device asking to sign in, so it's signed in only once the User has seen which. */
function Approve({
  code,
  deviceName,
  onDone,
}: {
  code: string;
  deviceName: string;
  /** Goes back to entering a code. */
  onDone: () => void;
}) {
  const approve = useSensitiveMutation(SignInService.method.approveSignInCode);
  const errors = approve.error ? codeErrors(approve.error) : {};
  if (approve.isSuccess) {
    return <Approved deviceName={deviceName} onDone={onDone} />;
  }

  return (
    <>
      <p className="text-muted-foreground">
        <strong className="font-medium text-foreground">{deviceName}</strong> is asking to sign in
        as you. Sign it in only if it’s yours.
      </p>
      {/* Only one of the two is ever set. */}
      <p role="alert" className="mt-4 text-sm text-destructive empty:hidden">
        {errors.signInCode}
        {errors.form}
      </p>
      <div className="mt-6 flex gap-2">
        <Button
          disabled={approve.isPending}
          onClick={() => {
            approve.mutate({ code });
          }}
        >
          {approve.isPending ? "Signing it in…" : "Sign it in"}
        </Button>
        <Button variant="ghost" onClick={onDone}>
          Cancel
        </Button>
      </div>
    </>
  );
}

function Approved({ deviceName, onDone }: { deviceName: string; onDone: () => void }) {
  return (
    <>
      <p className="text-muted-foreground">
        <strong className="font-medium text-foreground">{deviceName}</strong> is signed in as you.
        It may take a few seconds to notice.
      </p>
      <div className="-ms-4 mt-6">
        <Button variant="ghost" onClick={onDone}>
          Sign in another Device
        </Button>
      </div>
    </>
  );
}

function codeErrors(error: unknown): FormErrors {
  return refusal(error, "check that code", {
    [Code.NotFound]: {
      signInCode:
        "That code doesn’t match a Device waiting to sign in. Check it, or get a new code on the Device.",
    },
    [Code.FailedPrecondition]: { signInCode: "That code has already been used." },
  });
}
