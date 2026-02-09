/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        liberte: {
          bg: "var(--liberte-bg)",
          surface: "var(--liberte-surface)",
          panel: "var(--liberte-panel)",
          accent: "var(--liberte-accent)",
          text: "var(--liberte-text)",
          muted: "var(--liberte-muted)",
          success: "var(--liberte-success)",
          warning: "var(--liberte-warning)",
          border: "var(--liberte-border)",
        },
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "sans-serif"],
        mono: ["JetBrains Mono", "monospace"],
      },
    },
  },
  plugins: [],
};
