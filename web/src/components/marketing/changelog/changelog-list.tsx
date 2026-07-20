/**
 * Changelog — the release blocks.
 *
 * One card per release (the marketing-card shell of `TermsProse`),
 * newest first, straight from the `RELEASES` data file. Each block's
 * anchor id is the version string — the target of release
 * announcements' `link_url` (`/changelog#v0.1.1`) — with `scroll-mt`
 * so the fragment lands below the fixed navbar.
 *
 * Server Component through and through: static data, no interactivity.
 * Section labels come from i18n; the release copy itself is English
 * data by decision (see `releases.ts`).
 */

import { getLocale, getTranslations } from "next-intl/server";

import { RELEASES } from "./releases";

export async function ChangelogList() {
  const t = await getTranslations("Changelog.sections");
  const locale = await getLocale();
  const formatDate = new Intl.DateTimeFormat(locale, { dateStyle: "long" });

  return (
    <section className="mx-auto max-w-[1800px] px-6 pb-20 lg:px-12 lg:pb-28">
      <div className="mx-auto flex max-w-[128ch] flex-col gap-8">
        {RELEASES.map((release) => (
          <article
            key={release.version}
            id={release.version}
            className="scroll-mt-24 rounded-[6px] border border-sothoth-500/15 bg-cosmos-900/60 p-6 transition-colors hover:border-sothoth-500/35 lg:p-8"
          >
            <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1">
              <h2 className="font-display text-[24px] font-semibold tracking-[0.02em] text-[#f1ecff]">
                {release.version}
              </h2>
              <time
                dateTime={release.date}
                className="text-[13px] tracking-[0.04em] text-slate-500"
              >
                {formatDate.format(new Date(release.date))}
              </time>
            </div>
            <p className="mt-3 text-[17px] leading-[1.7] text-slate-300">
              {release.summary}
            </p>
            {release.sections.map((section) => (
              <div key={section.kind} className="mt-6">
                <h3 className="text-[13px] font-semibold tracking-[0.22em] text-sothoth-400 uppercase">
                  {t(section.kind)}
                </h3>
                <ul className="mt-3 flex flex-col gap-2">
                  {section.items.map((item) => (
                    <li
                      key={item}
                      className="flex gap-3 text-[15px] leading-[1.7] text-slate-300"
                    >
                      <span aria-hidden className="mt-[2px] text-sothoth-400">
                        •
                      </span>
                      <span>{item}</span>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </article>
        ))}
      </div>
    </section>
  );
}
