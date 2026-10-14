export function CoordinatorError({ message }: { message: string }) {
  return (
    <div role="alert" className="rounded-[var(--radius-card)] border border-border bg-surface p-6">
      <p className="font-medium">Live auction data is unavailable</p>
      <p className="mt-1 text-sm text-muted">
        {message} Bids already sent are safe: payments go to the escrow on Zcash, not to this site.
      </p>
    </div>
  );
}
