// Root layout — intentionally minimal. The real `<html>` and `<body>`
// tags live in `app/[locale]/layout.tsx` so that the `lang` attribute
// can be set from the active locale. Next.js still requires a root
// layout file at the top of the app directory.
export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return children;
}