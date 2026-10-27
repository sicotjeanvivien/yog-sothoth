/**
 * The changelog, one entry per release, edited at each release; its test guards unique
 * `vX.Y.Z` versions, newest first. English only. ⚠️ `version` is the anchor id an announcement's
 * `link_url` points at (`/changelog#<version>`).
 */

export type ReleaseSectionKind = "features" | "fixes";

export type ReleaseItem = {
  /** Free English copy, one bullet. */
  text: string;
  /** Only the code is concerned: folded under the technical details, out of the filters. */
  technical: boolean;
};

export type ReleaseSection = {
  /** Which section label the chrome shows (i18n key). */
  kind: ReleaseSectionKind;
  items: readonly ReleaseItem[];
};

export type Release = {
  /** `vX.Y.Z` — display name AND anchor id of the block. */
  version: string;
  /** ISO date (YYYY-MM-DD), rendered localized. */
  date: string;
  /** One-sentence headline under the version. */
  summary: string;
  sections: readonly ReleaseSection[];
};

/** Newest first — the order the page renders. */
export const RELEASES: readonly Release[] = [
  {
    version: "v0.3.1",
    date: "2026-10-27",
    summary: "The hourly sweep for the tokens without metadata no longer compares each one to the whole table.",
    sections: [
      {
        kind: "fixes",
        items: [
          {
            text: "The hourly sweep for the mints without metadata reads them in a tenth of a second on a copy of production, instead of more than two minutes; while it ran, the database slowed down and some API requests failed.",
            technical: false,
          },
        ],
      },
    ],
  },
  {
    version: "v0.3.0",
    date: "2026-10-27",
    summary:
      "The API serves the fee parameters of DAMM v2 pools, refuses a query key a route does not take, and no longer fails on very large amounts.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "The fee parameters of a DAMM v2 pool's creation and of an operator's fee update, on pool-creations and pool-changes: the base fee by its schedule (time, market cap or rate limiter, linear or exponential), the compounding share and the dynamic fee. Each state is named, an unchanged field included; fees is null only where the stored bytes do not decode, and the indexer counts those it receives (yog_indexer_fee_parameters_undecodable_total).",
            technical: true,
          },
          {
            text: "A page of signals filtered by protocol or by severity reads only what can fill it, compressed chunks included, through two new indexes (migration 040).",
            technical: true,
          },
          {
            text: "TimescaleDB 2.31.0.",
            technical: true,
          },
        ],
      },
      {
        kind: "fixes",
        items: [
          {
            text: "The Meteora and Raydium logos of the menu fit their box again.",
            technical: false,
          },
          {
            text: "Changed: a query key a route does not take is refused (400, naming the key) on every API route, instead of being ignored — /api/swaps/totals?protocol=… used to answer with every protocol.",
            technical: true,
          },
          {
            text: "A page of pool creations, liquidity changes or position changes no longer fails (500) when one of its amounts ends in sixteen zeros or more.",
            technical: true,
          },
          {
            text: "yog-signals stops within the grace every daemon has, and names a detector that outlives it.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.2.0",
    date: "2026-10-26",
    summary:
      "DAMM v2 has its own page and two tabs: its liquidity, and where its volume comes from — rotation runs, or the rest.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "A page for Meteora DAMM v2, in the menu: its TVL, volume, fees and active pools over the period you choose, the same figures as its row on the Overview, with the fees' effective rate and, in the advanced mode, the protocol's share.",
            technical: false,
          },
          {
            text: "Its first tab, Liquidity: the TVL hour by hour on a tight axis, the liquidity deposited when pools are created and added above it, removed below, per hour or per day, and in the advanced mode the largest moves of the period with their owner.",
            technical: false,
          },
          {
            text: "Each liquidity move is valued once, at its pool's price, never above what the pool traded: a deposit of a token the market overprices is worth what the pool would pay for it, and a token without a market price counts 0, as in the TVL. Liquidity is measured from this release on, so the 24-hour figures fill in over a day and the 7-day ones over a week.",
            technical: false,
          },
          {
            text: "Its second tab, Volume: each hour or day of volume split between rotation runs and the rest. A rotation run is a stretch of 5-minute slices in which a pool's SOL, USDC or USDT side (both its sides if it has none or two) changes hands at least once, no more than 30 minutes apart — most often a pool born within the minute that trades thousands of balanced swaps before its liquidity leaves. Under the chart, the period's split, or the hour's or the day's under the pointer; beside it, the runs of the period, how long they last and how much of their volume their first hour holds; below, the largest runs, named by their tokens. The published volume does not change.",
            technical: false,
          },
          {
            text: "Rotation runs are measured from this release on: a bar they do not cover whole keeps its volume, unsplit, in a neutral tint; the 24-hour figures fill in over a day and the 7-day ones over a week.",
            technical: false,
          },
          {
            text: "A green dot beside DAMM v2 in the menu while the ingestion is live, and its page in the sitemap.",
            technical: false,
          },
          {
            text: "yog-analytic writes each liquidity move into analytic_liquidity_moves and their sums per protocol and hour (migration 035), and each tranche's rotation into analytic_pool_5min, and the rotation runs drawn from it into analytic_rotation_runs and analytic_rotation_run_hours (migrations 036 and 039).",
            technical: true,
          },
          {
            text: "The API follows its route rules: the on-chain collections at /api and under /api/protocols/{protocol}, and all but pool-creations under /api/pools/{pool} (swaps, liquidity-changes, pool-creations, position-changes, position-fee-claims, protocol-fee-claims, reward-claims, reward-changes, pool-changes); the measures yog-analytic computes — swaps at /api, under a protocol and under a pool, pool-creations at /api and under a protocol, liquidity-moves and rotation-runs under a protocol (totals, hourly, daily), tvl and its hourly readings, pool-discoveries — and the pages of liquidity-moves and rotation-runs under a protocol and a pool; pools, tokens and their prices; the signals of one protocol or one pool; /api/announcements with ?status=. Migrations 037 and 038 open the pool facts to the API and index the business reads.",
            technical: true,
          },
          {
            text: "Changed: /api/protocols is now a page with a cursor, its entries under items. Removed, their readers moved: /api/protocols/activity and its series, /api/protocols/tvl and its hourly series, /api/tokens/activity (now /api/swaps/by-token), /api/announcements/active (now /api/announcements?status=open), and the pool filter of /api/signals (now /api/pools/{pool}/signals); /api/signals refuses a query key it does not take.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.16",
    date: "2026-10-23",
    summary:
      "The TVL now says what it counts: no pool drops out because one of its tokens has no price, and its change no longer takes a pool seen for the first time for a deposit.",
    sections: [
      {
        kind: "fixes",
        items: [
          {
            text: "A pool with one token without a market price dropped out of the TVL whole, with the value of its other side. That side now counts, the token without a price counting 0, as Meteora does.",
            technical: false,
          },
          {
            text: "A dormant pool touched for the first time brought its whole stock into the TVL, and its change showed it as a deposit. The change now leaves out the pools discovered during the period, and says how much they hold; a pool born during the period still counts as a deposit.",
            technical: false,
          },
          {
            text: "The TVL's tooltip says what it counts: the pools observed since 1 October 2026, at Jupiter prices under 15 minutes old, a token without a price at 0 — and what it does not measure, the liquidity that can be traded.",
            technical: false,
          },
          {
            text: "The TVL series starts again: the hours measured under the former price rule are deleted. The TVL itself shows \"—\" until the first hour measured after this release has closed, about an hour, and its change stays hidden until the new series covers the period.",
            technical: false,
          },
          {
            text: "The tvl_drain detector keeps reading a pool's TVL only when both of its sides are priced (pool_current_tvl.tvl_complete_usd): a drain over half a pool would be overstated.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.15",
    date: "2026-10-22",
    summary:
      "Search engines can now find the Overview, and the indexer no longer skips transactions when its stream provider turns a reconnection down for a reason the request did not cause.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "The Overview is listed in the sitemap, in English and in French, so search engines can index it.",
            technical: false,
          },
          {
            text: "Production can be read by a key that can do nothing else: the state of each service, the daemons' counters and a few counts from the database, as a user with no rights of its own and a sudo rule limited to three exact commands.",
            technical: true,
          },
        ],
      },
      {
        kind: "fixes",
        items: [
          {
            text: "When the stream provider refused a reconnection over an expired key, a quota or an incident on its side, the indexer resumed from the latest block, and the transactions in between were never indexed. It now asks again for the block where it stopped, and gives that point up only when the provider says the request itself is at fault.",
            technical: false,
          },
          {
            text: "Every resume point given up is logged and counted (yog_indexer_grpc_resume_marks_abandoned_total), labelled by the gRPC code that refused it.",
            technical: true,
          },
          {
            text: "tokio 1.54.1: 1.54.0 was withdrawn from crates.io.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.14",
    date: "2026-10-21",
    summary:
      "The volume counts again only what changed hands: a nearly empty pool could publish hundreds of millions of dollars for a few cents.",
    sections: [
      {
        kind: "fixes",
        items: [
          {
            text: "A token without a market price was valued at its pool's price after the last swap, which a single swap can push to any value when the pool is nearly empty: one pool published $801 million of volume for three swaps worth two cents, and the 24-hour volume read over a billion dollars. Each direction of a trade is now worth what changed hands on the side of the token that has a market price, usually SOL, fee included as Meteora counts it: a purchase, the SOL the buyer paid; a sale, the SOL the seller received plus the fee the pool took.",
            technical: false,
          },
          {
            text: "A fee taken in a token without a market price is now worth its share of the trade that paid it, no longer that token's average price over five minutes, which could overstate it after a crash.",
            technical: false,
          },
          {
            text: "The volume, fees and most traded tokens of the past days are computed again under these rules. For about three hours after this update, the Overview shows figures that stop at an earlier hour, announced by its band, and the swap flow signal waits while they catch up.",
            technical: false,
          },
          {
            text: "The analytic writer prices a token in its pool at the rate its five-minute tranche traded at, no longer at the pool's mid price; its five-minute aggregate, recreated, carries the fee of each direction; and it writes the active pools of every hour it freezes.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.13",
    date: "2026-10-21",
    summary: "The database is backed up again.",
    sections: [
      {
        kind: "fixes",
        items: [
          {
            text: "Backups had stopped: a dump over 8.39 GB was refused by the bucket at its 1,001st part. Parts are now 32 MiB, so a dump of up to 33.5 GB is stored.",
            technical: true,
          },
          {
            text: "TimescaleDB 2.30.2, Rust 1.99.1, Node 26, Yellowstone gRPC schema 14 and the dependencies' updates.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.12",
    date: "2026-10-20",
    summary:
      "The Overview now says which protocol its figures come from, over 24 hours or 7 days, with the most traded tokens and an advanced mode; a token sold off within the hour no longer inflates the volume; and the price service gets the memory it needs.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "The Overview is rebuilt: TVL, volume, fees and active pools, activity protocol by protocol (DAMM v2 observed, DLMM, Raydium and Orca to come), volume by hour or by day, the most traded tokens and the latest signals. A period selector switches the whole page between 24 hours and 7 days; 30 days opens once enough data is measured.",
            technical: false,
          },
          {
            text: "An advanced mode, off by default, puts more data on the same screen without making the page longer: swaps and their rate per second, turnover (the period's volume divided by the TVL), fees and active pools by protocol, and fees by hour or by day under the volume. The choice is remembered by a cookie, declared in the privacy policy.",
            technical: false,
          },
          {
            text: "A volume or fee figure that leaves out hours it could not value shows the share it does cover, such as \"97%\", rounded down so that a partial figure never reads 100%.",
            technical: false,
          },
          {
            text: "Every figure of the Overview stops at the last complete hour, the same for the whole page, up to about 70 minutes behind. A period or a change that would compare with hours not yet measured shows \"—\" and says why, rather than a smaller number; figures stopped over two hours back are announced by a band at the top of the page.",
            technical: false,
          },
          {
            text: "The API serves the protocols' activity, TVL and their hourly and daily series over a window (/api/protocols/…), and the tokens by volume (/api/tokens/activity). /api/stats, which only the old Overview read, is removed.",
            technical: true,
          },
        ],
      },
      {
        kind: "fixes",
        items: [
          {
            text: "A token sold off within an hour used to be valued at its market price from before the fall: one pool could publish $14.5 million of volume for less than a dollar actually received. Each token is now priced every five minutes, and never above what its own pool pays for it: in each pool, the token traded in the most pools keeps its market price, and the other is bounded by the pool's rate.",
            technical: false,
          },
          {
            text: "The hourly volume and fees now start about a week back: older hours could not be valued again under the new rule, and are dropped rather than mixed with it. For about three hours after this update, the 24-hour volume shows as unavailable while it is rebuilt.",
            technical: false,
          },
          {
            text: "The swap flow signal reads the same valuation, about fifteen minutes later than before.",
            technical: false,
          },
          {
            text: "The analytic writer prices each pool's two tokens once per five-minute tranche, keeps the market price beside the price it publishes, and values the swaps from it; no reader values a swap anymore.",
            technical: true,
          },
          {
            text: "The service that fetches token prices and names gets 64 MB of memory instead of 32: it needed about 35, so it lived on swap and was slowed down during its calls to the price providers.",
            technical: false,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.11",
    date: "2026-10-19",
    summary:
      "Shared links now show a preview, every API error answers in one format, deploying a new version no longer slows the site down, and the price gap signal no longer floods the feed with shallow pools.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "Shared links and search engines now get a title, a description and a preview image, in each page's language, and a sitemap lists the site's pages in both languages.",
            technical: false,
          },
        ],
      },
      {
        kind: "fixes",
        items: [
          {
            text: "A new version is built on GitHub when it is tagged, and the server only downloads it: it used to compile it itself, for 13 to 41 minutes, while still serving the site and the API, which slowed both down.",
            technical: false,
          },
          {
            text: "The signal comparing a pool's price with the market price only looks at pools holding at least $10,000: below that, a pool's price strays from the market by nature, and those pools made 98% of all signals, about 12,700 a day.",
            technical: false,
          },
          {
            text: "The signal engine reads fewer pools at each round: the price gap signal only those above its value floor, instead of every pool ever priced, and the swap flow signal only the active pools above its volume floor, plus those it cannot value, to count them. Its memory no longer follows every pool discovered.",
            technical: true,
          },
          {
            text: "The API now answers all its errors in one format, RFC 9457 Problem Details: a query or path it cannot read (an unknown severity, a non-numeric limit), an unknown route (404) and a wrong method (405) used to get plain text or an empty body.",
            technical: false,
          },
          {
            text: "An empty pool or cursor parameter on /api/signals (?pool=) is now refused with a 400 naming the parameter: it used to be read as absent and return the unfiltered feed.",
            technical: false,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.10",
    date: "2026-10-18",
    summary:
      "The pool registry writes lighter, the hourly volume and fees no longer re-read every swap of the hour at each refresh, new token names no longer re-read every pool, the signal engine gets the memory it needs, two discoveries of the same pool no longer collide, and an ordinary instruction no longer counts as a reading failure.",
    sections: [
      {
        kind: "fixes",
        items: [
          {
            text: "The pool registry drops its index on the last time a pool was seen, 345 MB that no query read since the pool list went away: refreshing that time on every event no longer has to write into every index of the table.",
            technical: true,
          },
          {
            text: "The hourly volume and fees are summed from five-minute slices refreshed every five minutes, instead of re-reading every swap of the open hour every five minutes: that read grew through the hour, up to a minute and a half, close to the two minutes after which the database cancels a query.",
            technical: true,
          },
          {
            text: "New tokens get their names looking only at the pools discovered in the last hour, instead of re-reading every pool known every ten seconds; every pool is still swept once an hour, for the tokens learned late.",
            technical: true,
          },
          {
            text: "The signal engine gets 64 MB of memory instead of 32: it needed about 40, so it lived on swap, reading back what it had just been made to set aside.",
            technical: true,
          },
          {
            text: "Two transactions discovering the same new pool at the same moment no longer make one of them fail, or deadlock in the database: the pool is recorded on its first event instead of its next one, and a failure that remains is now counted instead of only logged.",
            technical: true,
          },
          {
            text: "An ordinary instruction sent to the DAMM v2 program with few or no arguments is no longer counted as an event the indexer failed to read: nothing was lost, but the failure counter, the one that would show a misread of the stream, was never at zero. A truncated event is still counted, and so is data too short to be any instruction.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.9",
    date: "2026-10-18",
    summary:
      "The dashboard drops its pool pages, which timed out in production, ahead of the pages built around protocols and tokens, and the DAMM v2 TVL no longer drops to nothing one hour in two.",
    sections: [
      {
        kind: "fixes",
        items: [
          {
            text: "The pool list, the pool pages and the Watchlist are gone: their data timed out in production. Their old addresses answer 404.",
            technical: false,
          },
          {
            text: "The Overview no longer shows the Top pools block, which failed to load.",
            technical: false,
          },
          {
            text: "A signal card shows its pool's pair without linking to a pool page; hovering it shows the pool address.",
            technical: false,
          },
          {
            text: "The About page no longer promises to watch pools or query their history.",
            technical: false,
          },
          {
            text: "The API drops its eight /api/pools routes and every read only they used; the contract and the dashboard's schemas are regenerated.",
            technical: true,
          },
          {
            text: "The DAMM v2 TVL no longer drops to nothing, or to a third, about one hour in two.",
            technical: false,
          },
          {
            text: "The price worker asks Jupiter only for the tokens one of our pools moved, or that Jupiter priced, in the last 24 hours — about 20,000 instead of 95,000 — so a price round no longer outlasts the 15-minute freshness bound.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.8",
    date: "2026-10-17",
    summary:
      "The site's About, Support us, changelog and legal pages take the dashboard's look, and the site no longer scrolls sideways on small phones.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "The changelog shows one block per release, with an All / Features / Fixes filter. Changes that only concern the code are folded under the technical details.",
            technical: false,
          },
          {
            text: "The About page is rebuilt in five numbered sections. Its list of covered protocols follows the dashboard menu: DAMM v2 observed; DLMM, Raydium and Orca soon.",
            technical: false,
          },
          {
            text: "Support us lists three free ways to help (a star on GitHub, a share on X or LinkedIn, feedback) and a Solana donation address with a Copy button.",
            technical: false,
          },
          {
            text: "The privacy policy, legal notice and terms share one layout: tabs between the three, the update date, a table of contents and numbered sections. Their texts are unchanged, except as noted below.",
            technical: false,
          },
        ],
      },
      {
        kind: "fixes",
        items: [
          {
            text: "Support us no longer offers GitHub Sponsors: its button led to a GitHub profile, and no sponsorship exists. The privacy policy no longer mentions it either.",
            technical: false,
          },
          {
            text: "The About page now says how Yog-Scope works: a continuous stream from a Solana node rather than a WebSocket connection, one decoder per protocol, and tens of thousands of pools rather than hundreds.",
            technical: false,
          },
          {
            text: "On screens narrower than 375 px, the site's pages no longer scroll sideways.",
            technical: false,
          },
          {
            text: "Release dates in the changelog no longer show the previous day west of UTC.",
            technical: false,
          },
          {
            text: "The database gets more memory: its daily compression no longer slows the whole database down, indexing included.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.7",
    date: "2026-10-17",
    summary:
      "The dashboard frame no longer waits for its data, and the site's home page, header and footer take the dashboard's look.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "The dashboard menu marks the pages still to come as \"Soon\" instead of a lock, which read as a paid feature. Its new Preferences hold the language switch, and up to 720 px the menu becomes a drawer.",
            technical: false,
          },
          {
            text: "A live indicator in the Overview header tells how fresh our indexing is (Live, Delayed, Stale or Offline). A click opens the last event, the Solana slot and the RPC latency, which leave the menu.",
            technical: false,
          },
          {
            text: "The site's home page shows an illustrated preview of the dashboard and how Yog-Scope works, in five steps.",
            technical: false,
          },
          {
            text: "The site's header and footer are rebuilt, with an \"Open the dashboard\" button, the version and a FR | EN switch that stays on the same page. The site takes the dashboard's colours and the Geist typeface, and its logos are vector images.",
            technical: false,
          },
        ],
      },
      {
        kind: "fixes",
        items: [
          {
            text: "The dashboard no longer waits for any data before it shows. The announcements and each Overview block arrive on their own, and a block that fails shows its own error instead of an error page.",
            technical: false,
          },
          {
            text: "The site and the API are served by a front web server shared with other sites on the same machine. Nothing changes for visitors.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.6",
    date: "2026-10-16",
    summary:
      "Pages no longer fail on a token logo that is not a link or on a mistyped pool address, and the network status is split between the Solana network and our own indexing.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "The network status is split in two: /api/network gives the Solana network as our probe sees it (slot and latency), /api/ingestion gives how fresh our own indexing is. /api/network/status is gone. In the dashboard, the network panel loads each half on its own, so one that is slow or down no longer blanks the other.",
            technical: false,
          },
          {
            text: "The current state of a pool gives the dollar value of each of its two sides, under the same price-age limit as its TVL. Tokens embedded in a pool or a signal no longer carry a price, which had no age limit: the pool page's composition chart now uses the server's values, and is hidden when a side's price is missing or more than 15 minutes old; the signal card no longer shows a line of token prices.",
            technical: false,
          },
        ],
      },
      {
        kind: "fixes",
        items: [
          {
            text: "A token logo that is not an http or https link is now dropped, and the page shows the token's initial instead. A logo that was not a link at all used to make the whole page fail to load (pool list, pool page, signals) or the Overview's top-pools block show an error.",
            technical: false,
          },
          {
            text: "A pool page opened with a malformed or truncated address now shows \"Pool not found\". It used to fail with a server error, or show an error with a Retry button that could never work.",
            technical: false,
          },
          {
            text: "A security update of the TLS library used by the backend services (rustls 0.23.46).",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.5",
    date: "2026-10-14",
    summary:
      "The Overview's figures load again: they now read the values computed once per hour instead of being recomputed at each request.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "A new Yog-Scope logo and new browser icons, on the site and in the dashboard.",
            technical: false,
          },
        ],
      },
      {
        kind: "fixes",
        items: [
          {
            text: "The Overview's 24h volume, fees and total TVL now read the hourly values introduced in v0.1.4, instead of revaluing a full day of swaps at each request, which had made them fail to load. A figure the hourly service has not caught up with shows as — rather than as a partial sum.",
            technical: false,
          },
          {
            text: "Valuing the TVL of every pool now reads each token's latest price from a table that keeps one row per token, instead of searching the whole price history twice per pool. It was the slowest step of the hourly service.",
            technical: true,
          },
          {
            text: "The price-deviation signal reads its prices from the same table, instead of searching the whole price history twice per pool. It was the slowest step of the signal service.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.4",
    date: "2026-10-13",
    summary:
      "Groundwork for the Overview figures: swaps are now valued once per hour and kept, and the backup service's database access is narrowed.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "A new background service values each pool's swaps in dollars once per hour, keeps the result, and freezes each hour ten minutes after it closes. Nothing reads these figures yet: the Overview's 24h volume, fees and TVL, which are still recomputed from every swap at each request and currently fail to load, will move to them once they cover a full day.",
            technical: true,
          },
        ],
      },
      {
        kind: "fixes",
        items: [
          {
            text: "The backup service now reads the application's data schema by schema, instead of through PostgreSQL's built-in read-all role, which reached beyond what a backup needs.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.3",
    date: "2026-10-09",
    summary:
      "The dashboard is now organised by protocol, and raw swaps are kept for a week so the database stays within its disk.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "The dashboard menu is organised by protocol: Overview, then Meteora DAMM v2, Meteora DLMM, Raydium and Orca, then Token. Only Overview is open for now; the other entries carry a lock until their page exists. The pool list, signals and watchlist pages are no longer in the menu.",
            technical: false,
          },
          {
            text: "A new look for the dashboard frame: one colour per role, and the Geist typeface.",
            technical: false,
          },
        ],
      },
      {
        kind: "fixes",
        items: [
          {
            text: "Raw swap events are kept for 7 days instead of 30, and compressed one day after their day ends instead of seven: since every DAMM v2 pool is observed, they were outgrowing the disk. The hourly swap history that charts and analytics read is unaffected and still kept in full; a pool's list of individual swaps now goes back 7 days.",
            technical: false,
          },
          {
            text: "Next.js is updated to 16.3.8, following five advisories published on 7 October. None of them applied to this site's configuration.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.2",
    date: "2026-10-07",
    summary:
      "More regular price sampling: Jupiter requests are spaced instead of sent in bursts, and a restart keeps the prices a round had already received.",
    sections: [
      {
        kind: "fixes",
        items: [
          {
            text: "Requests to Jupiter are spaced under the configured rate limit (60 per minute by default) instead of sent in bursts, which Jupiter refused (HTTP 429) and which left tokens that have a price without one, round after round.",
            technical: false,
          },
          {
            text: "A token Jupiter answers without a price is asked again less and less often — up to every 15 minutes at the default 30-second cadence — instead of at every round; the next price it gets puts it back on every round. A request in which none of its 50 tokens gets a price is read as a failed answer, and changes no token's turn.",
            technical: true,
          },
          {
            text: "Stopping the enrichment service in the middle of a pricing round keeps the prices already received, instead of losing the whole round.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.1",
    date: "2026-10-06",
    summary: "A security update of the dashboard's image library.",
    sections: [
      {
        kind: "fixes",
        items: [
          {
            text: "sharp, the image library behind the dashboard, is updated to 0.35.5 for a vulnerability in the librsvg it bundles (CVE-2026-96889). Nothing else changes.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.0",
    date: "2026-10-06",
    summary:
      "The first production release: a real-time observer of Meteora DAMM v2 on Solana, with a signal engine that surfaces risk across every observed pool.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "Real-time indexing of Meteora DAMM v2 events — swaps, liquidity, positions, fee updates and pool lifecycle — decoded from on-chain Anchor emissions, for an operator-selected set of pools; the other pools their transactions touch are discovered along the way.",
            technical: false,
          },
          {
            text: "Pool pages: composition, spot price decoded from the on-chain sqrt-price, realized-fees analytics with 30-day fee and effective-rate charts, and Swaps, Liquidity and Alerts tabs.",
            technical: false,
          },
          {
            text: "Pools list with a fee column and a fee-tier filter, and a watchlist of starred pools, kept in the browser.",
            technical: false,
          },
          {
            text: "Overview with global KPIs — total TVL, 24h volume and fees, pools discovered — a top-pools ranking and the latest signals.",
            technical: false,
          },
          {
            text: "Signal engine with three risk detectors: flow imbalance, price–oracle deviation, and TVL drain (rug-like liquidity exodus).",
            technical: false,
          },
          {
            text: "Live signal feed on /signals — streamed over SSE, with severity × detector filters and per-detector explanations — and a worst-severity indicator on the pools list, with a hover detail.",
            technical: false,
          },
          {
            text: "Token enrichment: metadata, USD prices and pool-account backfill, independent from the ingestion path. Jupiter rate-limits (429) are retried with backoff, and every provider call carries a timeout.",
            technical: false,
          },
          {
            text: "Operator announcements: a dismissible banner for maintenance, incidents, releases and beta notes, published without a deploy.",
            technical: false,
          },
          {
            text: "Dashboard: collapsible sidebar with a network-status panel (Solana slot, RPC latency), metric definitions behind info popovers, an English and French interface, and a privacy notice that lists every processing and transfer.",
            technical: false,
          },
          {
            text: "Hardened serving: the API bounds its memory, connections and heavy requests — past its limits it turns heavy requests and streams away instead of running out of memory — and the dashboard sends HSTS and a content security policy that blocks framing and limits where it connects.",
            technical: true,
          },
        ],
      },
    ],
  },
] as const;
