// ⚠️ `<html>` and `<body>` live in `app/[locale]/layout.tsx`, which sets `lang`; Next.js
// still requires this root layout.
export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return children;
}