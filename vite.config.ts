import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  build: {
    // 隔离正式Web资源，避免将dist中的历史凭据预设随安装包重新发布。
    // 仅清理target下的可再生资源，不删除或修改dist中的任何文件。
    outDir: "src-tauri/target/web",
    emptyOutDir: true,
  },
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
});
