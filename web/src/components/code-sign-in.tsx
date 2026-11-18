import { Code } from "@connectrpc/connect";
import { useMutation } from "@connectrpc/connect-query";
import { QRCodeSVG } from "qrcode.react";
import { type ReactNode, useEffect, useState } from "react";

import { SignInLayout, SignInPage } from "@/components/sign-in-layout";
import {
  type PollSignInCodeResponse,
  SessionDelivery,
  SignInCodeStatus,
  SignInService,
  type StartSignInCodeResponse,
} from "@/gen/nama/v1/sign_in_pb";
import { FormField } from "@/shared/components/form-field/form-field";
import { isCode } from "@/shared/lib/connect-errors";
import { type FormErrors, formText, refusal, serverMessage } from "@/shared/lib/forms";
import { Button } from "@/shared/ui/button";

const heading = "Sign in with a code";

/**
 * Signing this browser in without a password: it shows a Sign-in Code, and a User signed in on
 * another Device approves it at `/code`.
 */
export function CodeSignIn({
  onSignedIn,
  footer,
}: {
  /** Called once, when the code is approved and this browser holds its session. */
  onSignedIn: () => Promise<void>;
  /** Another way to sign in instead. */
  footer: ReactNode;
}) {
  // Kept while getting a new code, so the expired one stays up until the new one replaces it.
  const [shown, setShown] = useState<StartSignInCodeResponse>();
  const [deviceName, setDeviceName] = useState("");
  const start = useMutation(SignInService.method.startSignInCode, { onSuccess: setShown });
  const errors = start.error ? startErrors(start.error) : {};
  const getCode = (name: string) => {
    setDeviceName(name);
    start.mutate({ deviceName: name, delivery: SessionDelivery.COOKIE });
  };

  if (!shown) {
    return (
      <NameThisDevice
        onSubmit={getCode}
        pending={start.isPending}
        errors={errors}
        footer={footer}
      />
    );
  }
  return (
    <SignInPage heading={heading} intro={<WhereToEnterIt />} footer={footer}>
      <WaitForApproval key={shown.handle} shown={shown} onSignedIn={onSignedIn}>
        {/* The Device name field is gone by now, so its refusal shows here. */}
        <p role="alert" className="text-sm text-destructive empty:hidden">
          {errors.form ?? errors.deviceName}
        </p>
        <Button
          disabled={start.isPending}
          onClick={() => {
            getCode(deviceName);
          }}
        >
          Get a new code
        </Button>
      </WaitForApproval>
    </SignInPage>
  );
}

function NameThisDevice({
  onSubmit,
  pending,
  errors,
  footer,
}: {
  onSubmit: (deviceName: string) => void;
  pending: boolean;
  errors: FormErrors;
  footer: ReactNode;
}) {
  return (
    <SignInLayout
      heading={heading}
      intro="Name this Device and get a Sign-in Code. Then enter it on a Device where you’re already signed in."
      footer={footer}
      action={pending ? "Getting a code…" : "Get a code"}
      pending={pending}
      error={errors.form}
      onSubmit={(form) => {
        onSubmit(formText(form, "deviceName"));
      }}
    >
      <DeviceNameField error={errors.deviceName} />
    </SignInLayout>
  );
}

// Unlike signing in with a password, getting a code still needs the Device's name: the Server
// doesn't name it after the browser for a Sign-in Code.
function DeviceNameField({ error }: { error: string | undefined }) {
  return (
    <FormField
      label="Device name"
      name="deviceName"
      hint="So you can tell your Devices apart in Settings."
      placeholder="Living room laptop"
      maxLength={64}
      error={error}
    />
  );
}

function WhereToEnterIt() {
  return (
    <>
      Scan the QR code with your phone. Or, on a Device where you’re signed in, go to{" "}
      <strong className="font-medium text-foreground">{window.location.origin}/code</strong> and
      enter this code.
    </>
  );
}

/** Shows the code until it's approved, or, once it expires, `newCode` to get another. */
function WaitForApproval({
  shown,
  onSignedIn,
  children: newCode,
}: {
  shown: StartSignInCodeResponse;
  onSignedIn: () => Promise<void>;
  children: ReactNode;
}) {
  const expired = usePollUntilApproved(shown.handle, onSignedIn);
  return (
    <div className="flex flex-col gap-5">
      <ShownCode code={shown.code} expired={expired} />
      {expired && newCode}
    </div>
  );
}

/** The code, with its QR code while it can still be approved. */
function ShownCode({ code, expired }: { code: string; expired: boolean }) {
  return (
    <div className="rounded-xl border bg-surface-raised px-4 py-8 text-center">
      <p
        className={
          expired
            ? "text-4xl font-semibold tracking-wide whitespace-nowrap text-muted-foreground tabular-nums line-through"
            : "text-4xl font-semibold tracking-wide whitespace-nowrap tabular-nums"
        }
      >
        {code}
      </p>
      {!expired && <ScanToApprove code={code} />}
      <output className="mt-3 block text-sm text-muted-foreground">
        {expired ? "This code has expired." : "Waiting for it to be approved…"}
      </output>
    </div>
  );
}

/** `code` as a link to `/code` on this Server, for a phone to open by scanning. */
function ScanToApprove({ code }: { code: string }) {
  return (
    // Dark modules on a light ground, since not every scanner reads them inverted. That holds only
    // while the app is dark only: a light theme makes `foreground` dark.
    <QRCodeSVG
      value={`${window.location.origin}/code?${new URLSearchParams({ code })}`}
      title="Scan it with your phone to approve this Device there"
      size={176}
      marginSize={4}
      bgColor="transparent"
      fgColor="currentColor"
      className="mx-auto mt-6 rounded-lg bg-foreground text-background"
    />
  );
}

// ponytail: fixed-interval polling, which is what the API offers.
const pollEvery = 3000;

/**
 * Polls the sign-in `handle` stands for until it's approved, then calls `onSignedIn`. Returns
 * whether it expired instead.
 *
 * A poll is a mutation, not a query: a signed-in answer hands out the session only once.
 */
function usePollUntilApproved(handle: string, onSignedIn: () => Promise<void>) {
  const { mutate, isPending, data, error } = useMutation(SignInService.method.pollSignInCode, {
    onSuccess: async ({ status }) => {
      if (status === SignInCodeStatus.SIGNED_IN) {
        await onSignedIn();
      }
    },
  });

  // Runs again after every poll, since each brings a new `data` or `error`. `isPending` alone
  // wouldn't do: a quick answer can arrive before React renders the pending state.
  useEffect(() => {
    const poll = () => {
      mutate({ handle });
    };
    const pollAgain = !isPending && isUndecided(statusOf(data, error));
    const timer = pollAgain ? setTimeout(poll, pollEvery) : undefined;
    return () => {
      clearTimeout(timer);
    };
  }, [isPending, data, error, handle, mutate]);
  return statusOf(data, error) === SignInCodeStatus.EXPIRED;
}

/** Where a sign-in has got to, going by its last poll: none before the first, or after a failed one. */
function statusOf(data: PollSignInCodeResponse | undefined, error: unknown) {
  // A Server restart forgets the handle, so `not_found` means the code is gone too.
  return isCode(error, Code.NotFound) ? SignInCodeStatus.EXPIRED : data?.status;
}

// Without a status, such as after a poll failed while the Server restarted, it polls again.
function isUndecided(status: SignInCodeStatus | undefined) {
  return status !== SignInCodeStatus.EXPIRED && status !== SignInCodeStatus.SIGNED_IN;
}

function startErrors(error: unknown): FormErrors {
  return refusal(error, "get a code", {
    [Code.InvalidArgument]: { deviceName: serverMessage(error) },
    // The Server caps how many sign-ins wait at once; each code expires within 10 minutes.
    [Code.ResourceExhausted]: {
      form: "Too many Devices are waiting to sign in. Try again in a few minutes.",
    },
  });
}
