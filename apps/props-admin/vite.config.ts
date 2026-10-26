import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  base: "/props-admin/",
  plugins: [vue()],
  server: {
    port: 5174,
    strictPort: true,
    proxy: {
      "/internal": {
        target: process.env.SERVER_URL ?? "http://127.0.0.1:3000",
        changeOrigin: true,
      },
    },
  },
  preview: {
    port: 4174,
    strictPort: true,
    proxy: {
      "/internal": {
        target: process.env.SERVER_URL ?? "http://127.0.0.1:3000",
        changeOrigin: true,
      },
    },
  },
});
