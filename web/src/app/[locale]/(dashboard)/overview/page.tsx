import { setRequestLocale } from "next-intl/server";
export const dynamic = "force-dynamic";

export default async function OverviewPage() {
  return (
    <div>
      <h1>Overview Page</h1>
      <p>Welcome to the about page!</p>
    </div>
  );
}