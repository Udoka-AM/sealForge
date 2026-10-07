/** Temporary screen body until the task in docs/06-IMPLEMENTATION-PLAN.md lands. */
export function Placeholder({ screen, task }: { screen: string; task: string }) {
  return (
    <div className="rounded-[var(--radius-card)] border border-border bg-surface p-6 text-muted">
      <p className="text-sm">
        {screen}: not built yet. Tracked in implementation plan task{" "}
        <span className="num">{task}</span>.
      </p>
    </div>
  );
}
