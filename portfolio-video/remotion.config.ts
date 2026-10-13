/**
 * Bundler settings for the promo. The interface it shows comes from captures in
 * public/captures/ (see capture/capture.mjs), so nothing outside this folder is
 * bundled.
 */

import { Config } from "@remotion/cli/config";

Config.setVideoImageFormat("png");
