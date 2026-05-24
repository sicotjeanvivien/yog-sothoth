import { setRequestLocale } from "next-intl/server";

type SupportUsPageProps = {
  params: Promise<{ locale: string }>;
};

export default async function SupportUsPage({ params }: SupportUsPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);
  return (
    <div>
      <h1>Support US Page</h1>
      <p>Welcome to the about page!</p>
    </div>
  );
}