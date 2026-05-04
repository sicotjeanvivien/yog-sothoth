import type { Config } from "tailwindcss";

// Color palette extracted from the Yog-Sothoth mockups:
// deep cosmic background, violet/purple accents for the brand,
// cyan for secondary highlights, and a few semantic colors used
// across alerts and status indicators in the dashboard.
const config: Config = {
  content: [
    "./src/**/*.{ts,tsx}",
    "./i18n/**/*.{ts,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        // Surfaces — from the deepest cosmic backdrop to elevated cards.
        cosmos: {
          950: "#05030d",
          900: "#0a0815",
          800: "#0f0c1d",
          700: "#15122a",
          600: "#1c1838",
        },
        // Brand violet — used for the logo glow, primary accents, charts.
        sothoth: {
          400: "#a78bfa",
          500: "#8b5cf6",
          600: "#7c3aed",
          700: "#6d28d9",
        },
        // Cyan accent — secondary highlights, hover states, active items.
        eldritch: {
          400: "#22d3ee",
          500: "#06b6d4",
          600: "#0891b2",
        },
        // Semantic — kept conservative and readable on dark surfaces.
        signal: {
          good: "#34d399",
          warn: "#fbbf24",
          bad: "#f87171",
        },
      },
      fontFamily: {
        // Brand display font reserved for the logotype and major headings.
        // Falls back to system serif if Cinzel is not loaded.
        display: ["Cinzel", "Georgia", "serif"],
        // Default UI font: same stack Tailwind ships, kept here for clarity.
        sans: [
          "ui-sans-serif",
          "system-ui",
          "-apple-system",
          "Segoe UI",
          "Roboto",
          "sans-serif",
        ],
      },
    },
  },
  plugins: [],
};

export default config;