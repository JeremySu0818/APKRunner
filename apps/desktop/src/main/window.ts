import { BrowserWindow, nativeTheme } from "electron";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

function currentModuleDir(): string {
  return dirname(fileURLToPath(import.meta.url));
}

export function createMainWindow(): BrowserWindow {
  nativeTheme.themeSource = "dark";
  const moduleDir = currentModuleDir();
  const window = new BrowserWindow({
    width: 1480,
    height: 940,
    minWidth: 1180,
    minHeight: 760,
    backgroundColor: "#05070d",
    title: "APKRunner",
    titleBarStyle: process.platform === "darwin" ? "hiddenInset" : "default",
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
      preload: join(moduleDir, "../preload/index.mjs")
    }
  });

  const rendererUrl = process.env.ELECTRON_RENDERER_URL;
  if (rendererUrl) {
    void window.loadURL(rendererUrl);
  } else {
    void window.loadFile(join(moduleDir, "../renderer/index.html"));
  }

  return window;
}
