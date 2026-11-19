import { Code } from "@connectrpc/connect";
import { act, cleanup, fireEvent, screen } from "@testing-library/react";
import jsQR from "jsqr";
import { afterEach, beforeEach, expect, test, vi } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { SignInCodeStatus } from "@/gen/nama/v1/sign_in_pb";
import { fakeServer, fill, newUser, openApp, type ServerState } from "@/test-app";
import { confirmItsYou, recentAuthenticationRequired, refused } from "@/test-refusal";

beforeEach(() => {
  // Polling waits seconds between tries; the tests move the clock on instead.
  vi.useFakeTimers({ shouldAdvanceTime: true });
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

async function getCode() {
  fireEvent.click(await screen.findByRole("link", { name: "Sign in with a code" }));
  await screen.findByLabelText("Device name");
  fill("Device name", "Living room TV");
  fireEvent.click(screen.getByRole("button", { name: "Get a code" }));
}

async function nextPoll() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(5000);
  });
}

/** A signed-out Server showing "ABCD-EFGH" for "Living room TV", until `approve()` is called. */
function serverWaitingForApproval() {
  const state: ServerState = {};
  const transport = fakeServer(state, {
    startSignInCode: ({ deviceName }) => {
      expect(deviceName).toBe("Living room TV");
      return { code: "ABCD-EFGH", handle: "secret" };
    },
    pollSignInCode: ({ handle }) => {
      expect(handle).toBe("secret");
      if (state.signedIn === undefined) {
        return { status: SignInCodeStatus.WAITING };
      }
      return { status: SignInCodeStatus.SIGNED_IN };
    },
  });
  return {
    transport,
    approve: () => {
      state.signedIn = newUser;
    },
  };
}

test("once the code is approved, polling signs the browser in", async () => {
  const { transport, approve } = serverWaitingForApproval();
  const router = openApp("/sign-in?redirect=%2Fsettings", transport);

  await getCode();
  await expect(screen.findByText("ABCD-EFGH")).resolves.toBeDefined();

  await nextPoll();
  approve();
  await nextPoll();
  await screen.findByRole("heading", { name: "Settings" });
  expect(router.state.location.pathname).toBe("/settings");
});

test("back and forward step through signing in with a code, code and all", async () => {
  const router = openApp("/sign-in", serverWaitingForApproval().transport);

  await getCode();
  await screen.findByText("ABCD-EFGH");
  router.history.back();
  await screen.findByRole("button", { name: "Get a code" });
  router.history.back();
  await screen.findByRole("button", { name: "Sign in" });

  router.history.forward();
  await screen.findByRole("button", { name: "Get a code" });
  router.history.forward();
  await expect(screen.findByText("ABCD-EFGH")).resolves.toBeDefined();
});

/**
 * What the QR code `svg`, as qrcode.react draws it, says. Its dark modules are one path of runs,
 * each `M<x> <y>h<width>…` (or `M<x>,<y> h…`), drawn here dark on light for jsQR to read.
 */
function decodeQr(svg: Element) {
  const modules = Number(svg.getAttribute("viewBox")?.split(" ")[2]);
  const darkRuns = svg.querySelectorAll("path")[1]?.getAttribute("d") ?? "";
  const scale = 4;
  const width = modules * scale;
  const rgba = new Uint8ClampedArray(width * width * 4).fill(255);
  for (const { groups } of darkRuns.matchAll(/M(?<x>\d+)[ ,](?<y>\d+) ?h(?<length>\d+)/gu)) {
    const { x = "0", y = "0", length = "0" } = groups ?? {};
    for (let px = Number(x) * scale; px < (Number(x) + Number(length)) * scale; px += 1) {
      for (let py = Number(y) * scale; py < (Number(y) + 1) * scale; py += 1) {
        rgba.fill(0, (py * width + px) * 4, (py * width + px) * 4 + 3);
      }
    }
  }
  return jsQR(rgba, width, width)?.data;
}

test("the code shows a QR code that opens it at /code on a phone", async () => {
  openApp("/sign-in", serverWaitingForApproval().transport);

  await getCode();
  const qr = await screen.findByRole("img", { name: /Scan it with your phone/u });
  expect(decodeQr(qr)).toBe(`${globalThis.location.origin}/code?code=ABCD-EFGH`);
});

/** A Server whose first code, "ABCD-EFGH", has expired, and whose next is "JKMN-PQRS". */
function serverWithExpiredCode() {
  const codes = ["ABCD-EFGH", "JKMN-PQRS"];
  return fakeServer(
    {},
    {
      startSignInCode: () => {
        const code = codes.shift() ?? "";
        return { code, handle: code };
      },
      pollSignInCode: ({ handle }) => ({
        status: handle === "ABCD-EFGH" ? SignInCodeStatus.EXPIRED : SignInCodeStatus.WAITING,
      }),
    },
  );
}

test("an expired code offers a new one", async () => {
  openApp("/sign-in", serverWithExpiredCode());

  await getCode();
  await nextPoll();
  fireEvent.click(await screen.findByRole("button", { name: "Get a new code" }));
  await expect(screen.findByText("JKMN-PQRS")).resolves.toBeDefined();
  expect(screen.queryByText("ABCD-EFGH")).toBeNull();
});

test("a code the Server forgot, after a restart, counts as expired", async () => {
  const transport = fakeServer(
    {},
    {
      startSignInCode: () => ({ code: "ABCD-EFGH", handle: "secret" }),
      pollSignInCode: () => {
        throw refused(Code.NotFound, "no sign-in is waiting on that handle", {
          reason: RefusalReason.HANDLE_NOT_FOUND,
        });
      },
    },
  );
  openApp("/sign-in", transport);

  await getCode();
  await nextPoll();
  await expect(screen.findByRole("button", { name: "Get a new code" })).resolves.toBeDefined();
});

test("too many waiting sign-ins says so, not that there were too many tries", async () => {
  const transport = fakeServer(
    {},
    {
      startSignInCode: () => {
        throw refused(Code.ResourceExhausted, "too many sign-ins are waiting", {
          reason: RefusalReason.TOO_MANY_PENDING_SIGN_INS,
        });
      },
    },
  );
  openApp("/sign-in", transport);

  await getCode();
  await expect(
    screen.findByText(/Too many Devices are waiting to sign in/u),
  ).resolves.toBeDefined();
});

/** Refuses any code but the one "Living room TV" shows, as the Server would match it. */
function refuseUnlessShown(code: string) {
  // As the Server does, ignoring case and dashes.
  if (code.replaceAll("-", "").toLowerCase() !== "abcdefgh") {
    throw refused(Code.NotFound, "no such code", { reason: RefusalReason.SIGN_IN_CODE_NOT_FOUND });
  }
}

/**
 * A Server where "Living room TV" shows `abcdefgh`, approving into `approved`, and anyone signing
 * in becomes a new User. Signed in, unless `given` another state.
 */
function serverWithCode(approved: string[] = [], given?: ServerState) {
  const state = given ?? { signedIn: newUser };
  return fakeServer(state, {
    signIn: () => {
      state.signedIn = newUser;
      return {};
    },
    lookUpSignInCode: ({ code }) => {
      refuseUnlessShown(code);
      return { deviceName: "Living room TV" };
    },
    approveSignInCode: ({ code }) => {
      refuseUnlessShown(code);
      approved.push(code);
      return {};
    },
  });
}

async function lookUp(code: string) {
  fill("Sign-in Code", code);
  fireEvent.click(await screen.findByRole("button", { name: "Continue" }));
}

test("looking up a code names the Device before it's approved", async () => {
  const approved: string[] = [];
  openApp("/code", serverWithCode(approved));
  await screen.findByRole("heading", { name: "Sign in a Device" });

  // Typed in lower case, without its dash: the field writes it as the Device shows it.
  await lookUp("abcdefgh");
  await screen.findByRole("heading", { name: "Sign in “Living room TV”?" });
  expect(approved).toStrictEqual([]);

  fireEvent.click(screen.getByRole("button", { name: "Sign it in" }));
  await screen.findByRole("heading", { name: "“Living room TV” is signed in" });
  expect(approved).toStrictEqual(["ABCD-EFGH"]);
});

test("back from a signed-in Device goes to the code, not to approving it again", async () => {
  const router = openApp("/code", serverWithCode());
  await screen.findByRole("heading", { name: "Sign in a Device" });
  await lookUp("abcdefgh");
  fireEvent.click(await screen.findByRole("button", { name: "Sign it in" }));
  await screen.findByRole("heading", { name: "“Living room TV” is signed in" });

  router.history.back();
  await expect(screen.findByRole("heading", { name: "Sign in a Device" })).resolves.toBeDefined();
  expect(screen.queryByRole("button", { name: "Sign it in" })).toBeNull();
});

test("a wrong code shows a plain error", async () => {
  openApp("/code", serverWithCode());
  await screen.findByRole("heading", { name: "Sign in a Device" });

  await lookUp("WRONG-123");
  await expect(screen.findByText(/That code doesn’t match/u)).resolves.toBeDefined();
  expect(screen.queryByRole("button", { name: "Sign it in" })).toBeNull();
});

/** A signed-in Server where "Living room TV" waits, refusing approval until `Reauthenticate`. */
function serverNeedingRecentAuthentication() {
  let reauthenticated = false;
  return fakeServer(
    { signedIn: newUser },
    {
      lookUpSignInCode: () => ({ deviceName: "Living room TV" }),
      approveSignInCode: () => {
        if (!reauthenticated) {
          throw recentAuthenticationRequired();
        }
        return {};
      },
      reauthenticate: () => {
        reauthenticated = true;
        return {};
      },
    },
  );
}

test("without Recent Authentication, a code is approved once you confirm it’s you", async () => {
  openApp("/code", serverNeedingRecentAuthentication());
  await screen.findByRole("heading", { name: "Sign in a Device" });

  await lookUp("abcdefgh");
  await screen.findByRole("heading", { name: "Sign in “Living room TV”?" });
  fireEvent.click(screen.getByRole("button", { name: "Sign it in" }));
  await confirmItsYou();

  await expect(
    screen.findByRole("heading", { name: "“Living room TV” is signed in" }),
  ).resolves.toBeDefined();
});

test("an approval the Server refuses goes back to the code, kept, saying why", async () => {
  openApp(
    "/code",
    fakeServer(
      { signedIn: newUser },
      {
        lookUpSignInCode: () => ({ deviceName: "Living room TV" }),
        approveSignInCode: () => {
          throw refused(Code.FailedPrecondition, "already used", {
            reason: RefusalReason.SIGN_IN_CODE_ALREADY_APPROVED,
          });
        },
      },
    ),
  );
  await screen.findByRole("heading", { name: "Sign in a Device" });

  await lookUp("abcdefgh");
  fireEvent.click(await screen.findByRole("button", { name: "Sign it in" }));

  await expect(
    screen.findByText("That code has already been used. Get a new code on the Device."),
  ).resolves.toBeDefined();
  expect(screen.getByLabelText<HTMLInputElement>("Sign-in Code").value).toBe("ABCD-EFGH");
});

test("opening a scanned code looks it up, ready to approve", async () => {
  const approved: string[] = [];
  openApp("/code?code=abcdefgh", serverWithCode(approved));

  await screen.findByRole("heading", { name: "Sign in “Living room TV”?" });
  fireEvent.click(screen.getByRole("button", { name: "Sign it in" }));
  await screen.findByRole("heading", { name: "“Living room TV” is signed in" });
  expect(approved).toStrictEqual(["abcdefgh"]);

  fireEvent.click(screen.getByRole("link", { name: "Sign in another Device" }));
  const field = await screen.findByLabelText<HTMLInputElement>("Sign-in Code");
  expect(field.value).toBe("");
});

test("a scanned code that doesn't match shows it with a plain error", async () => {
  openApp("/code?code=WRONG-123", serverWithCode());

  await expect(screen.findByText(/That code doesn’t match/u)).resolves.toBeDefined();
  // The field reads the look-alike O as the zero a Sign-in Code would have.
  expect(screen.getByLabelText<HTMLInputElement>("Sign-in Code").value).toBe("WR0N-G123");
});

test("opening a scanned code signed out comes back to it after signing in", async () => {
  openApp("/code?code=abcdefgh", serverWithCode([], {}));

  fireEvent.change(await screen.findByLabelText("Username"), { target: { value: "Sam" } });
  fill("Password", "correct horse");
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
  await screen.findByRole("heading", { name: "Sign in “Living room TV”?" });
  expect(screen.getByRole("button", { name: "Sign it in" })).toBeDefined();
});
