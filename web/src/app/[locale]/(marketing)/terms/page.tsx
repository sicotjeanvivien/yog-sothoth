import { setRequestLocale } from "next-intl/server";

type TermPageProps = {
  params: Promise<{ locale: string }>;
};

export default async function TermsPage({ params }: TermPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);
  return (
    <div>
      <h1>Terms Page</h1>
      <p>Welcome to the about page!</p>
    </div>
  );
}