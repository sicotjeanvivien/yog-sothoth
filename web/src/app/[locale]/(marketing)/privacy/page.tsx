import { setRequestLocale } from "next-intl/server";

type PrivacyPageProps = {
  params: Promise<{ locale: string }>;
};

export default async function PrivacyPage({ params }: PrivacyPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);
  return (
    <div>
      <h1>Privacy Page</h1>
      <p>Welcome to the about page!</p>
    </div>
  );
}