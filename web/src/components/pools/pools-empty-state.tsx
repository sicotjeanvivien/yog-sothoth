/**
 * Empty state for the pools page — shown when yog-api returns zero
 * pools. Reachable in two situations:
 *
 *   - Fresh database before the indexer has observed anything.
 *   - The user navigated past the last page (next_cursor null and
 *     no items). Rare but possible.
 *
 * Visual: centered card with a subtle eldritch glyph, mirroring the
 * page chrome's cosmic palette without distracting from the message.
 */

type PoolsEmptyStateProps = {
  title: string;
  description: string;
};

export function PoolsEmptyState({ title, description }: PoolsEmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center rounded-lg border border-cosmos-700/60 bg-cosmos-900/40 px-6 py-16 text-center shadow-[0_0_40px_-16px_rgba(124,58,237,0.2)]">
      <EmptyGlyph />
      <h2 className="mt-6 font-display text-xl tracking-wider text-sothoth-400">
        {title}
      </h2>
      <p className="mt-2 max-w-md text-sm text-slate-400">{description}</p>
    </div>
  );
}

function EmptyGlyph() {
  // Concentric rings — a minimalist nod to the "all-seeing" Yog-Sothoth
  // metaphor without dragging in any third-party iconography.
  return (
    <svg
      width="48"
      height="48"
      viewBox="0 0 48 48"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className="text-sothoth-500/70"
      aria-hidden="true"
    >
      <circle cx="24" cy="24" r="22" stroke="currentColor" strokeWidth="0.8" opacity="0.3" />
      <circle cx="24" cy="24" r="14" stroke="currentColor" strokeWidth="0.8" opacity="0.55" />
      <circle cx="24" cy="24" r="6" stroke="currentColor" strokeWidth="1" />
      <circle cx="24" cy="24" r="1.5" fill="currentColor" />
    </svg>
  );
}