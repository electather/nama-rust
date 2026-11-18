/** What to say about the User called `name` once the Server no longer has them. */
export function noLongerHere(name: string) {
  return { form: `${name} isn’t on this Server any more.` };
}
