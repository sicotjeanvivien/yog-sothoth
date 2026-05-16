import { setRequestLocale } from "next-intl/server";
export const dynamic = "force-dynamic";

export default async function OverviewPage() {
  return (
    <div className="mx-auto max-w-7xl px-6 py-10 lg:px-10 lg:py-12">
      Overview
    </div>
  );
}