/** Which Item is being fixed, and what opens or closes one. */
export interface Fixing {
  fixing: bigint | undefined;
  toggle: (itemId: bigint) => void;
}
