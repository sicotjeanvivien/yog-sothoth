/**
 * Support us — "Make it known" card.
 *
 * Full-width card encouraging the two lowest-friction support
 * actions: starring the GitHub repo and sharing the project on
 * social networks. Sits between the hero and the Feedback /
 * Sponsor grid because these are the cheapest ways to help and
 * also, at v0.1, probably the most useful for the project.
 *
 * Inside, a 2-column split:
 *   - Left  → Star on GitHub
 *   - Right → Share on X / LinkedIn (pre-filled text)
 *
 * The pre-filled share text lives in the i18n bundle so it can
 * change per locale.
 */

import { getTranslations } from "next-intl/server";

import { GithubIcon, LinkedinIcon, XIcon } from "@/components/shared/icon";

const GITHUB_REPO_URL = "https://github.com/sicotjeanvivien/yog-sothoth";
const SITE_URL = "https://yog-scope.xyz";

const CTA_PRIMARY_CLASS =
  "inline-flex items-center justify-center gap-2 rounded-[4px] border border-sothoth-500/45 bg-sothoth-600/15 px-5 py-[10px] text-[14px] font-semibold text-[#f1ecff] transition-colors hover:border-sothoth-500/70 hover:bg-sothoth-600/30";

const CTA_SECONDARY_CLASS =
  "inline-flex items-center justify-center gap-2 rounded-[4px] border border-slate-700 bg-transparent px-5 py-[10px] text-[14px] font-semibold text-slate-200 transition-colors hover:border-slate-500 hover:bg-slate-800/40";

export async function SupportUsMakeKnown() {
  const t = await getTranslations("SupportUs.makeKnown");

  // Pre-built share URLs. Pulled here rather than inline so we can
  // see the encoding shape at a glance.
  const shareText = t("shareText");
  const twitterShareUrl = `https://twitter.com/intent/tweet?text=${encodeURIComponent(
    shareText,
  )}&url=${encodeURIComponent(SITE_URL)}`;
  const linkedinShareUrl = `https://www.linkedin.com/sharing/share-offsite/?url=${encodeURIComponent(
    SITE_URL,
  )}`;

  return (
    <section className="mx-auto max-w-[1800px] px-6 pb-6 lg:px-12">
      <div className="mx-auto max-w-[128ch]">
        <article className="rounded-[6px] border border-sothoth-500/15 bg-cosmos-900/60 p-6 transition-colors hover:border-sothoth-500/35 lg:p-8">
          <div className="grid grid-cols-1 gap-8 lg:grid-cols-2 lg:gap-10">
            {/* Left — Star on GitHub */}
            <div>
              <h2 className="font-display text-[18px] font-semibold tracking-[0.02em] text-[#f1ecff] lg:text-[20px]">
                {t("star.title")}
              </h2>
              <p className="mt-3 text-[15px] leading-[1.65] text-slate-300">
                {t("star.body")}
              </p>
              <div className="mt-5">
                <a
                  href={GITHUB_REPO_URL}
                  target="_blank"
                  rel="noopener noreferrer"
                  className={CTA_PRIMARY_CLASS}
                >
                  <GithubIcon size={16} />
                  {t("star.cta")}
                </a>
              </div>
            </div>

            {/* Right — Share */}
            <div className="lg:border-l lg:border-sothoth-500/15 lg:pl-10">
              <h2 className="font-display text-[18px] font-semibold tracking-[0.02em] text-[#f1ecff] lg:text-[20px]">
                {t("share.title")}
              </h2>
              <p className="mt-3 text-[15px] leading-[1.65] text-slate-300">
                {t("share.body")}
              </p>
              <div className="mt-5 flex flex-wrap gap-3">
                <a
                  href={twitterShareUrl}
                  target="_blank"
                  rel="noopener noreferrer"
                  className={CTA_SECONDARY_CLASS}
                >
                  <XIcon size={14} />
                  {t("share.ctaX")}
                </a>
                <a
                  href={linkedinShareUrl}
                  target="_blank"
                  rel="noopener noreferrer"
                  className={CTA_SECONDARY_CLASS}
                >
                  <LinkedinIcon size={14} />
                  {t("share.ctaLinkedin")}
                </a>
              </div>
            </div>
          </div>
        </article>
      </div>
    </section>
  );
}