/** Names the store the images are kept on now. */
export function StoreInUse({ name }: { name: string }) {
  return (
    <p className="mt-3 text-sm">
      In use: <span className="font-medium">{name}</span>
    </p>
  );
}
