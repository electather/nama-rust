/**
 * What was typed into a Setup or Sign-in Code field, as the code it's meant to be: upper case,
 * cut to `length` characters, in dashed groups of 4. The codes' alphabet (Crockford's base32) has
 * no I, L, O or U, so the look-alikes become the digits they stand for.
 */
export function formatCode(typed: string, length: number) {
  const plain = typed
    .toUpperCase()
    .replaceAll("O", "0")
    .replaceAll(/[IL]/gu, "1")
    .replaceAll(/[^0-9A-Z]/gu, "")
    .slice(0, length);
  return plain.match(/.{1,4}/gu)?.join("-") ?? "";
}
