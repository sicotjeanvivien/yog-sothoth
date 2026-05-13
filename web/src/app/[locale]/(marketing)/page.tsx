import { setRequestLocale, getTranslations } from "next-intl/server";
import { routing } from "../../../../i18n/routing";

type HomePageProps = {
  params: Promise<{ locale: string }>;
};

export default async function HomePage({ params }: HomePageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  // Translations are loaded server-side. No client-side hydration is
  // needed for static text on this page.
  const tBrand = await getTranslations("Brand");
  const tHome = await getTranslations("Home");

  return (
    <main className="flex min-h-screen items-center justify-center px-6 py-16">
      <section className="w-full max-w-xl text-center">
        <p className="mb-4 text-xs uppercase tracking-[0.4em] text-sothoth-400/80">
          {tBrand("tagline")}
        </p>
        <h1 className="font-display text-5xl tracking-wider text-sothoth-400">
          {tBrand("name")}
        </h1>
        <p className="mt-6 text-lg text-slate-300">
          {tHome("greeting", { name: tBrand("name") })}
        </p>
        <p className="mt-2 text-sm italic text-slate-400">
          {tBrand("motto")}
        </p>
        <p className="mt-12 inline-block rounded-full border border-sothoth-600/40 px-4 py-1 text-xs uppercase tracking-widest text-sothoth-400/70">
          {tHome("milestoneTag")}
        </p>
      </section>
    </main>
  );
}

// Required for static rendering of the locale-scoped page itself.
export function generateStaticParams() {
  return routing.locales.map((locale) => ({ locale }));
}