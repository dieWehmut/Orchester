import type { CapacitorConfig } from "@capacitor/cli";
const config: CapacitorConfig = {
  appId: "dev.orchester.mobile",
  appName: "Orchester",
  backgroundColor: "#1b110e",
  loggingBehavior: "none",
  webDir: "dist",
  server: { cleartext: true },
  android: { allowMixedContent: true },
  plugins: {
    SystemBars: { style: "DARK", initialViewportFitValueHint: "cover" },
  },
};
export default config;
