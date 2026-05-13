/**
 * Generic section wrapper used by both feeds (swaps, liquidity).
 *
 * Provides:
 *   - A section header with title + optional subtitle/count.
 *   - A container with consistent chrome (border, background, shadow).
 *   - Slots for the table body and the pagination footer.
 *
 * Keeps the visual signature consistent across feeds without each
 * feed component having to reimplement the wrapping.
 */

type FeedSectionProps = {
  title: string;
  subtitle?: string;
  children: React.ReactNode;
};

export function FeedSection({ title, subtitle, children }: FeedSectionProps) {
  return (
    <section className="rounded-lg border border-cosmos-700/60 bg-cosmos-900/60 shadow-[0_0_40px_-12px_rgba(124,58,237,0.25)] backdrop-blur-sm">
      <header className="flex items-baseline justify-between gap-4 border-b border-cosmos-700/60 bg-cosmos-800/40 px-6 py-4">
        <h2 className="font-display text-lg tracking-wider text-sothoth-400">
          {title}
        </h2>
        {subtitle !== undefined && (
          <span className="text-[10px] uppercase tracking-[0.18em] text-slate-500">
            {subtitle}
          </span>
        )}
      </header>
      <div className="px-6 py-6">{children}</div>
    </section>
  );
}