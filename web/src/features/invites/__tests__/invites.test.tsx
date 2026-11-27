import { create } from "@bufbuild/protobuf";
import { Code } from "@connectrpc/connect";
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { type AcceptInviteRequest, Permission, SessionDelivery } from "@/gen/nama/v1/sign_in_pb";
import { type CreateInviteRequest, InviteSchema } from "@/gen/nama/v1/user_pb";
import { fill, openApp } from "@/test-app";
import { expectUnderField, refused } from "@/test-refusal";
import { admin, fakeServer, newUser } from "@/test-server";

afterEach(cleanup);

function invite(id: bigint, note: string) {
  return create(InviteSchema, {
    id,
    note,
    creatorName: "Sam",
    permissions: newUser,
    createdAt: 1_700_000_000n,
    expiresAt: 1_700_604_800n,
  });
}

test("without a Server URL, Settings' Users section offers no way to make an Invite", async () => {
  openApp("/code?settings=users", fakeServer({ signedIn: admin }).transport);

  await expect(screen.findByText(/needs this Server’s Server URL/u)).resolves.toBeDefined();
  expect(screen.queryByRole("button", { name: "Make an Invite" })).toBeNull();
});

test("making an Invite sends its note and the default Permissions, and shows its link", async () => {
  let sent: CreateInviteRequest | undefined;
  openApp(
    "/code?settings=users",
    fakeServer(
      { signedIn: admin },
      {
        UserService: {
          listInvites: () => ({ serverUrlSet: true }),
          createInvite: (request) => {
            sent = request;
            return { invite: invite(1n, "Mum"), url: "https://nama.example.com/invite/abc" };
          },
        },
      },
    ).transport,
  );

  fireEvent.click(await screen.findByRole("button", { name: "Make an Invite" }));
  fill("Who it’s for", "Mum");
  fireEvent.click(screen.getByRole("button", { name: "Make Invite" }));

  const link = await screen.findByLabelText("Invite link");
  expect(link).toHaveProperty("value", "https://nama.example.com/invite/abc");
  expect(screen.getByRole("button", { name: "Copy link" })).toBeDefined();
  expect(sent?.note).toBe("Mum");
  expect(sent?.permissions).toStrictEqual(newUser);
});

test("an Invite offers only the Permissions you hold, and never Admin", async () => {
  openApp(
    "/code?settings=users",
    fakeServer(
      { signedIn: [Permission.MANAGE_USERS, Permission.REQUEST] },
      { UserService: { listInvites: () => ({ serverUrlSet: true }) } },
    ).transport,
  );

  fireEvent.click(await screen.findByRole("button", { name: "Make an Invite" }));
  const form = screen.getByRole("group", { name: "Permissions" });
  expect(within(form).getByLabelText("Request")).toBeDefined();
  expect(within(form).queryByLabelText("Manage Libraries")).toBeNull();
  expect(within(form).queryByLabelText("Admin")).toBeNull();
});

test("revoking a pending Invite, once confirmed, removes it", async () => {
  let invites = [invite(1n, "Mum"), invite(2n, "")];
  openApp(
    "/code?settings=users",
    fakeServer(
      { signedIn: admin },
      {
        UserService: {
          listInvites: () => ({ invites, serverUrlSet: true }),
          revokeInvite: ({ inviteId }) => {
            invites = invites.filter((each) => each.id !== inviteId);
            return {};
          },
        },
      },
    ).transport,
  );

  fireEvent.click(await screen.findByRole("button", { name: "Revoke Invite for Mum" }));
  fireEvent.click(
    within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Revoke" }),
  );

  await waitFor(() => {
    expect(screen.queryByText("Mum")).toBeNull();
  });
  expect(screen.getByText("No note")).toBeDefined();
});

test("opening an Invite's link, picking a name and password lands signed in", async () => {
  let sent: AcceptInviteRequest | undefined;
  const { server, transport } = fakeServer(
    {},
    {
      SignInService: {
        acceptInvite: (request) => {
          sent = request;
          server.signedIn = newUser;
          return {};
        },
      },
    },
  );
  const router = openApp("/invite/abc", transport);

  await screen.findByRole("heading", { name: "Join Living Room" });
  fill("Username", "Mum");
  fill("Password", "correct horse");
  fireEvent.click(screen.getByRole("button", { name: "Join" }));

  await expect(screen.findByRole("link", { name: "Home" })).resolves.toBeDefined();
  expect(router.state.location.pathname).toBe("/");
  // Back doesn't reopen the used Invite.
  expect(router.history.canGoBack()).toBe(false);
  expect(sent?.token).toBe("abc");
  expect(sent?.delivery).toBe(SessionDelivery.COOKIE);
});

test("a taken name is shown next to its field, and an Invite that no longer works says so", async () => {
  let refusal = refused(Code.AlreadyExists, "taken", { reason: RefusalReason.USER_NAME_TAKEN });
  openApp(
    "/invite/abc",
    fakeServer(
      {},
      {
        SignInService: {
          acceptInvite: () => {
            throw refusal;
          },
        },
      },
    ).transport,
  );

  await screen.findByRole("heading", { name: "Join Living Room" });
  fill("Username", "Mum");
  fill("Password", "correct horse");
  fireEvent.click(screen.getByRole("button", { name: "Join" }));
  await expectUnderField("Username", "Someone already has that name. Pick another.");

  refusal = refused(Code.NotFound, "gone", { reason: RefusalReason.INVITE_NOT_FOUND });
  fireEvent.click(screen.getByRole("button", { name: "Join" }));

  await expect(screen.findByText(/This Invite no longer works/u)).resolves.toBeDefined();
});
