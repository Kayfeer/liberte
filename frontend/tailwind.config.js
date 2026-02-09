/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        liberte: {
          bg: "#1a1a2e",
          surface: "#16213e",
          panel: "#0f3460",
          accent: "#e94560",
          text: "#eaeaea",
          muted: "#8a8a9a",
          success: "#4ade80",
          warning: "#fbbf24",
          border: "#2a2a4a",
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
