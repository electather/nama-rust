import type { MessageInitShape } from "@bufbuild/protobuf";
import { Code, type Transport } from "@connectrpc/connect";
import { cleanup, fireEvent, screen } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  type LibraryAccessSchema,
  type MaximumAgeSchema,
  Permission,
} from "@/gen/nama/v1/sign_in_pb";
import { openApp } from "@/test-app";
import { refused } from "@/test-refusal";
import { admin, fakeServer, newUser } from "@/test-server";
import { dropdown, pick } from "@/test-settings";

afterEach(cleanup);

/** Settings' Users section, the address a test opens it at. */
const usersSection = "/code?settings=users";

/** Opens `name`'s details from the list of Users, once they're in its place. */
async function openUser(name: string) {
  fireEvent.click(await screen.findByRole("link", { name }));
  await screen.findByRole("region", { name });
}

/** A fake Server keeping Alex's Maximum Age and Libraries as they're set, with two Libraries. */
function keepingAlex() {
  const alex: {
    permissions?: Permission[];
    maximumAge?: MessageInitShape<typeof MaximumAgeSchema> | undefined;
    libraryAccess?: MessageInitShape<typeof LibraryAccessSchema> | undefined;
  } = {};
  const { transport } = fakeServer(
    {
      signedIn: admin,
      libraries: [
        { id: 1n, name: "Movies" },
        { id: 2n, name: "Cartoons" },
      ],
    },
    {
      UserService: {
        listUsers: () => ({ users: [{ id: 2n, name: "Alex", permissions: newUser, ...alex }] }),
        setMaximumAge: ({ maximumAge }) => {
          alex.maximumAge = maximumAge;
          return {};
        },
        setLibraryAccess: ({ libraryAccess }) => {
          alex.libraryAccess = libraryAccess;
          return {};
        },
      },
    },
  );
  return { alex, transport };
}

/** Opens the app afresh on Alex's details. */
async function reopenAlex(transport: Transport) {
  cleanup();
  openApp(usersSection, transport);
  await openUser("Alex");
}

describe("an Admin setting a User's Maximum Age", () => {
  test("is saved as it's picked, and is there after reloading", async () => {
    const { alex, transport } = keepingAlex();
    await reopenAlex(transport);

    expect(dropdown("Maximum Age").textContent).toBe("None");
    expect(
      screen.getByRole("switch", { name: "Allow unrated titles" }).getAttribute("aria-disabled"),
    ).toBe("true");
    pick("Maximum Age", "13");
    await expect.poll(() => alex.maximumAge?.age).toBe(13);
    fireEvent.click(screen.getByRole("switch", { name: "Allow unrated titles" }));
    await expect.poll(() => alex.maximumAge?.allowUnrated).toBe(true);
    await reopenAlex(transport);

    expect(dropdown("Maximum Age").textContent).toBe("13");
    expect(
      screen.getByRole("switch", { name: "Allow unrated titles" }).getAttribute("aria-checked"),
    ).toBe("true");
  });

  test("is changed, then cleared by picking None, and is gone after reloading", async () => {
    const { alex, transport } = keepingAlex();
    alex.maximumAge = { age: 16, allowUnrated: true };
    await reopenAlex(transport);

    pick("Maximum Age", "13");
    await expect.poll(() => alex.maximumAge).toMatchObject({ age: 13, allowUnrated: true });
    fireEvent.click(screen.getByRole("switch", { name: "Allow unrated titles" }));
    await expect.poll(() => alex.maximumAge?.allowUnrated).toBe(false);
    pick("Maximum Age", "None");
    await expect.poll(() => alex.maximumAge).toBeUndefined();
    await reopenAlex(transport);

    expect(dropdown("Maximum Age").textContent).toBe("None");
  });
});

describe("an Admin setting a User's Libraries", () => {
  test("chooses which they see, starting from every Library, and it's there after reloading", async () => {
    const { alex, transport } = keepingAlex();
    await reopenAlex(transport);

    expect(dropdown("Libraries").textContent).toBe("All Libraries");
    expect(screen.queryByRole("switch", { name: "Movies" })).toBeNull();
    pick("Libraries", "Chosen Libraries");
    await expect.poll(() => alex.libraryAccess?.libraryIds).toStrictEqual([1n, 2n]);
    fireEvent.click(screen.getByRole("switch", { name: "Movies" }));
    await expect.poll(() => alex.libraryAccess?.libraryIds).toStrictEqual([2n]);
    await reopenAlex(transport);

    expect(dropdown("Libraries").textContent).toBe("Chosen Libraries");
    expect(screen.getByRole("switch", { name: "Movies" }).getAttribute("aria-checked")).toBe(
      "false",
    );
    expect(screen.getByRole("switch", { name: "Cartoons" }).getAttribute("aria-checked")).toBe(
      "true",
    );
  });

  test("is changed, then goes back to every Library by picking All Libraries, after reloading too", async () => {
    const { alex, transport } = keepingAlex();
    alex.libraryAccess = { libraryIds: [2n] };
    await reopenAlex(transport);

    fireEvent.click(screen.getByRole("switch", { name: "Movies" }));
    await expect.poll(() => alex.libraryAccess?.libraryIds).toStrictEqual([2n, 1n]);
    pick("Libraries", "All Libraries");
    await expect.poll(() => alex.libraryAccess).toBeUndefined();
    await reopenAlex(transport);

    expect(dropdown("Libraries").textContent).toBe("All Libraries");
    expect(screen.queryByRole("switch", { name: "Movies" })).toBeNull();
  });
});

describe("manage Users and Chosen Libraries, which the Server never lets a User hold together", () => {
  const why =
    "Alex can’t Manage Users while on Chosen Libraries, unless they Manage Libraries too.";

  test("manage Users isn't offered on Chosen Libraries, nor taking Manage Libraries from one who holds both", async () => {
    const { alex, transport } = keepingAlex();
    alex.libraryAccess = { libraryIds: [2n] };
    await reopenAlex(transport);

    expect(screen.getByRole("checkbox", { name: "Manage Users" })).toMatchObject({
      checked: false,
      disabled: true,
    });
    expect(screen.getByText(why)).toBeDefined();
    alex.permissions = [Permission.MANAGE_USERS, Permission.MANAGE_LIBRARIES];
    await reopenAlex(transport);

    expect(screen.getByRole("checkbox", { name: "Manage Libraries" })).toMatchObject({
      checked: true,
      disabled: true,
    });
    expect(screen.getByRole("checkbox", { name: "Manage Users" })).toHaveProperty(
      "disabled",
      false,
    );
  });

  test("chosen Libraries isn't offered to a User with Manage Users, unless they Manage Libraries too", async () => {
    const { alex, transport } = keepingAlex();
    alex.permissions = [Permission.MANAGE_USERS];
    await reopenAlex(transport);

    fireEvent.click(dropdown("Libraries"));
    const chosen = screen.getByRole("option", { name: "Chosen Libraries" });
    expect(chosen.getAttribute("aria-disabled")).toBe("true");
    expect(screen.getByText("A User with Manage Users sees every Library.")).toBeDefined();
    alex.permissions = [Permission.MANAGE_USERS, Permission.MANAGE_LIBRARIES];
    await reopenAlex(transport);

    pick("Libraries", "Chosen Libraries");
    await expect.poll(() => alex.libraryAccess?.libraryIds).toStrictEqual([1n, 2n]);
  });

  test("a change the Server refuses for it says why", async () => {
    const { transport } = fakeServer(
      { signedIn: admin, libraries: [{ id: 1n, name: "Movies" }] },
      {
        UserService: {
          listUsers: () => ({ users: [{ id: 2n, name: "Alex", permissions: newUser }] }),
          setLibraryAccess: () => {
            throw refused(Code.FailedPrecondition, "refused", {
              reason: RefusalReason.MANAGE_USERS_NEEDS_EVERY_LIBRARY,
            });
          },
        },
      },
    );
    await reopenAlex(transport);

    pick("Libraries", "Chosen Libraries");

    await expect(screen.findByText(why)).resolves.toBeDefined();
    expect(dropdown("Libraries").textContent).toBe("All Libraries");
  });
});
