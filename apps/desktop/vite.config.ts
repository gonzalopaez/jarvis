import { defineConfig, loadEnv } from "vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const environment = loadEnv(mode, ".", "JARVIS_");
  const coreOrigin = environment.JARVIS_CORE_URL;

  return {

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
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
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
    // Development-only same-origin health bridge. Protected routes are not proxied.
    proxy: coreOrigin
      ? {
          "/api/v1/health": {
            target: coreOrigin,
            changeOrigin: true,
            secure: true,
          },
        }
      : undefined,
  },
  };
});
