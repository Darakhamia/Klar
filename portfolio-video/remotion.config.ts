/**
 * Bundler settings for the portfolio video.
 *
 * The video renders the app's own components from ../src rather than copies of
 * them, which takes two adjustments:
 *
 * - ../src sits outside this project, so a bare import from there (the
 *   @fontsource @imports in global.css) has to be able to find this project's
 *   node_modules when the app's own are not installed. Remotion already pins
 *   React to one copy for every file it bundles.
 * - The IPC layer is swapped for fake data. History.tsx and Voice.tsx import
 *   `../lib/settings`, which calls into Tauri; at bundle time that one module
 *   is replaced with src/mocks/settings.ts, and the components cannot tell.
 */

import path from "node:path";
import { webpack } from "@remotion/bundler";
import { Config } from "@remotion/cli/config";

const root = process.cwd();
const ipc = path.resolve(root, "..", "src", "lib", "settings");
const mock = path.resolve(root, "src", "mocks", "settings.ts");

Config.setVideoImageFormat("png");

Config.overrideWebpackConfig((config) => ({
  ...config,
  resolve: {
    ...config.resolve,
    modules: ["node_modules", path.resolve(root, "node_modules")],
  },
  plugins: [
    ...(config.plugins ?? []),
    new webpack.NormalModuleReplacementPlugin(/[\\/]lib[\\/]settings$/, (resource) => {
      if (path.resolve(resource.context, resource.request) === ipc) {
        resource.request = mock;
      }
    }),
  ],
}));
