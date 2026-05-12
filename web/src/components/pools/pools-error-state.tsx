/**
 * Error state for the pools page — shown when the BFF returns a
 * non-OK status or when the upstream contract fails.
 *
 * The component is presentational; resolving the right title and
 * description per `kind` happens in the page (via next-intl) so the
 * component stays locale-agnostic.
 */

type PoolsErrorStateProps = {
  title: string;
  description: string;
  /** Optional href for a retry/back link, rendered as text-only CTA. */
  retryHref?: string;
  retryLabel?: string;
};

export function PoolsErrorState({
  title,
  description,
  retryHref,
  retryLabel,
}: PoolsErrorStateProps) {
  return (
    <div className="flex flex-col items-center justify-center rounded-lg border border-signal-bad/30 bg-cosmos-900/60 px-6 py-16 text-center shadow-[0_0_40px_-16px_rgba(248,113,113,0.2)]">
      <WarningGlyph />
      <h2 className="mt-6 font-display text-xl tracking-wider text-signal-bad">
        {title}
      </h2>
      <p className="mt-2 max-w-md text-sm text-slate-400">{description}</p>

      {retryHref && retryLabel && (
        <a
          href={retryHref}
          className="mt-6 inline-flex items-center rounded-full border border-sothoth-600/40 px-4 py-1.5 text-xs uppercase tracking-widest text-sothoth-400 transition-colors hover:bg-sothoth-600/10"
        >
          {retryLabel}
        </a>
      )}
    </div>
  );
}

function WarningGlyph() {
  // Open triangle — universally recognisable warning shape, kept thin
  // so it blends with the rest of the cosmic chrome.
  return (
    <svg
      width="48"
      height="48"
      viewBox="0 0 48 48"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className="text-signal-bad/80"
      aria-hidden="true"
    >
      <path
        d="M24 8 L42 38 L6 38 Z"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinejoin="round"
      />
      <line x1="24" y1="20" x2="24" y2="30" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
      <circle cx="24" cy="34" r="1.2" fill="currentColor" />
    </svg>
  );
}