import { app, BrowserWindow } from "electron";
import { registerIpcHandlers } from "./ipc";
import { RuntimeService } from "./runtimeService";
import { createMainWindow } from "./window";

const gotLock = app.requestSingleInstanceLock();

if (!gotLock) {
  app.quit();
} else {
  const service = new RuntimeService();
  registerIpcHandlers(service);

  app.on("second-instance", () => {
    const window = createMainWindow();
    if (window.isMinimized()) {
      window.restore();
    }
    window.focus();
  });

  app.whenReady().then(() => {
    createMainWindow();

    app.on("activate", () => {
      if (BrowserWindowCount() === 0) {
        createMainWindow();
      }
    });
  }).catch((error) => {
    console.error("[APKRunner] Failed to start Electron app", error);
    app.quit();
  });

  app.on("window-all-closed", () => {
    if (process.platform !== "darwin") {
      app.quit();
    }
  });
}

function BrowserWindowCount(): number {
  return BrowserWindow.getAllWindows().length;
}
