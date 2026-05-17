import { setRequestLocale } from "next-intl/server";

type AboutPageProps = {
  params: Promise<{ locale: string }>;
};

export default async function AboutPage({ params }: AboutPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);
  return (
    <div>
      <h1>About Page</h1>
      <p>Welcome to the about page!</p>
    </div>
  );
}