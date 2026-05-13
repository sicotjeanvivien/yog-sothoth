/**
 * Empty and error states for the swap / liquidity feeds.
 *
 * Less prominent than the page-level Pools states — they sit inside
 * their feed section card rather than dominating the page chrome. The
 * page header and the latest-state card stay visible, so the user
 * still has context while one of the feeds is empty or failed.
 */

type FeedEmptyStateProps = {
  description: string;
};

export function FeedEmptyState({ description }: FeedEmptyStateProps) {
  return (
    <div className="py-8 text-center text-sm text-slate-500">
      {description}
    </div>
  );
}

type FeedErrorStateProps = {
  description: string;
};

export function FeedErrorState({ description }: FeedErrorStateProps) {
  return (
    <div className="py-8 text-center text-sm text-signal-bad/80">
      {description}
    </div>
  );
}