import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],
  
  // FIX 1: Wrap 'alias' inside a 'resolve' block
  resolve: {
    alias: {
      // FIX 2: Point '@' to the project root './' instead of './src'
      "@": path.resolve(__dirname, "./"),
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
    // FIX 3: Ensure Vite is allowed to fetch files outside of the src/ directory
    fs: {
      allow: ["."],
    },
  },
}));