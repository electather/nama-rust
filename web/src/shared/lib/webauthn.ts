import type { MessageInitShape } from "@bufbuild/protobuf";

import type {
  AddPasskeyRequestSchema,
  SignInWithPasskeyRequestSchema,
  StartAddingPasskeyResponse,
  StartPasskeySignInResponse,
} from "@/gen/nama/hub/v1/account_pb";

/** ES256 and RS256, the only algorithms the Hub checks (`crates/nama-hub/src/accounts/webauthn.rs`). */
const ALGORITHMS = [-7, -257];

/** Whether this browser can use passkeys at all. */
export function browserHasPasskeys() {
  return "PublicKeyCredential" in globalThis;
}

/**
 * Has the browser create a passkey for the Account `started` is for, whose email is `email`, and
 * returns what the Hub needs to add it as `name`.
 */
export async function createPasskey(
  started: StartAddingPasskeyResponse,
  email: string,
  name: string,
): Promise<MessageInitShape<typeof AddPasskeyRequestSchema>> {
  const credential = await navigator.credentials.create({
    publicKey: {
      challenge: bytes(started.challenge),
      // The Hub's, never this page's hostname: the two may differ.
      rp: { id: started.rpId, name: "Nama Hub" },
      user: { id: bytes(started.userId), name: email, displayName: email },
      pubKeyCredParams: ALGORITHMS.map((alg) => ({ type: "public-key", alg })),
      excludeCredentials: started.excludeCredentialIds.map((id) => ({
        type: "public-key",
        id: bytes(id),
      })),
      // A passkey the sign-in page finds without an email first.
      authenticatorSelection: {
        residentKey: "required",
        requireResidentKey: true,
        userVerification: "preferred",
      },
    },
  });
  if (
    !(credential instanceof PublicKeyCredential) ||
    !(credential.response instanceof AuthenticatorAttestationResponse)
  ) {
    throw new TypeError("the browser created no passkey");
  }
  const publicKey = credential.response.getPublicKey();
  if (!publicKey) {
    throw new Error("the browser can't give the passkey's public key");
  }
  return {
    name,
    credentialId: new Uint8Array(credential.rawId),
    clientDataJson: new Uint8Array(credential.response.clientDataJSON),
    publicKey: new Uint8Array(publicKey),
    publicKeyAlgorithm: credential.response.getPublicKeyAlgorithm(),
  };
}

/** Has the browser sign `started`'s challenge with a passkey the person picks. */
export async function signWithPasskey(
  started: StartPasskeySignInResponse,
): Promise<MessageInitShape<typeof SignInWithPasskeyRequestSchema>> {
  const credential = await navigator.credentials.get({
    // No `allowCredentials`: the browser offers every passkey it has for the Hub.
    publicKey: {
      challenge: bytes(started.challenge),
      rpId: started.rpId,
      userVerification: "preferred",
    },
  });
  if (
    !(credential instanceof PublicKeyCredential) ||
    !(credential.response instanceof AuthenticatorAssertionResponse)
  ) {
    throw new TypeError("the browser gave no passkey");
  }
  const { response } = credential;
  return {
    credentialId: new Uint8Array(credential.rawId),
    clientDataJson: new Uint8Array(response.clientDataJSON),
    authenticatorData: new Uint8Array(response.authenticatorData),
    signature: new Uint8Array(response.signature),
    userHandle: response.userHandle ? new Uint8Array(response.userHandle) : new Uint8Array(),
  };
}

/** `array` on an `ArrayBuffer` of its own, which WebAuthn takes. */
function bytes(array: Uint8Array) {
  return new Uint8Array(array);
}
