/**
 * Captures the real Klar frontend for the promo.
 *
 * The app's own pages — index.html (the settings window) and overlay.html (the
 * pill) — are served by the app's own Vite config and opened in Chromium with
 * `bridge.js` standing in for the Tauri shell. Nothing in ../src is changed or
 * copied. Output goes to public/captures/, which Remotion reads:
 *
 *   pill/NNNN.png       the overlay, frame by frame, Listening → Done (DPR 6)
 *   voice.png           Voice, dark: the Processing row (DPR 6)
 *   dictionary.png      Dictionary, dark: a sentence in the live preview (DPR 6)
 *   compare/*.png       DPR 2 shots for checking against the real app
 *   manifest.json       sizes, frame timing and the boxes the video crops to
 *
 * Usage: node capture/capture.mjs   (starts Vite on :1420 unless one is running)
 */

import { spawn } from "node:child_process";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import copy from "../copy.json" with { type: "json" };
import { applyDictionary, demo } from "./demo.mjs";
import { LEVEL_MS, levelsFor } from "./speech.mjs";

// The demo's clock times are written for a European machine, and the page is
// opened in that zone too, so 16:42 reads 16:42.
process.env.TZ = "Europe/Prague";

const APP = fileURLToPath(new URL("../../", import.meta.url));
const OUT = fileURLToPath(new URL("../public/captures/", import.meta.url));
const ORIGIN = "http://localhost:1420";
const FPS = 30;

/** The settings window's size, from src-tauri/tauri.conf.json. */
const MAIN = { width: 900, height: 620 };
/** The overlay window's size, from the same file. */
const OVERLAY = { width: 380, height: 96 };

const bridge = await readFile(new URL("./bridge.js", import.meta.url), "utf8");

async function up() {
  try {
    return (await fetch(ORIGIN)).ok;
  } catch {
    return false;
  }
}

/** The app's own dev server, as `npm run dev` starts it for the Tauri shell. */
async function vite() {
  if (await up()) return () => undefined;
  const child = spawn("npx", ["vite", "--port", "1420", "--strictPort"], {
    cwd: APP,
    stdio: "ignore",
  });
  for (let i = 0; i < 60 && !(await up()); i++) await new Promise((r) => setTimeout(r, 500));
  if (!(await up())) throw new Error("Vite did not start on :1420");
  return () => child.kill();
}

async function open(browser, { path, window, appearance, viewport, dpr, patch }) {
  const context = await browser.newContext({
    viewport,
    deviceScaleFactor: dpr,
    // A European Windows machine: 24-hour clock, like the screenshots.
    locale: "en-GB",
    timezoneId: "Europe/Prague",
    colorScheme: appearance === "light" ? "light" : "dark",
  });
  const page = await context.newPage();
  page.on("pageerror", (error) => {
    throw error;
  });
  const data = { ...demo(appearance), window };
  patch?.(data);
  await page.addInitScript({
    content: `window.__KLAR_DEMO__ = ${JSON.stringify(data)};
window.__KLAR_APPLY_DICTIONARY__ = ${applyDictionary.toString()};
${bridge}`,
  });
  await page.goto(ORIGIN + path, { waitUntil: "networkidle" });
  await page.evaluate(() => document.fonts.ready);
  return page;
}

async function section(page, name) {
  await page.getByRole("button", { name, exact: true }).click();
  await page.waitForTimeout(300);
  await page.evaluate(() => document.fonts.ready);
}

/**
 * The shots the video shows are taken at DPR 6: the largest the video draws
 * any of them is 5.4 frame pixels per CSS pixel, so the source is never
 * enlarged. Only the region around `boxes` is kept, with room to spare; its
 * origin is recorded so the boxes, in page coordinates, still line up.
 */
const SHOT_DPR = 6;

async function shoot(page, file, boxes) {
  const margin = 48;
  const left = Math.max(0, Math.min(...boxes.map((b) => b.x)) - margin);
  const top = Math.max(0, Math.min(...boxes.map((b) => b.y)) - margin);
  const viewport = page.viewportSize();
  const right = Math.min(viewport.width, Math.max(...boxes.map((b) => b.x + b.width)) + margin);
  const bottom = Math.min(viewport.height, Math.max(...boxes.map((b) => b.y + b.height)) + margin);
  const clip = { x: Math.floor(left), y: Math.floor(top), width: Math.ceil(right - left), height: Math.ceil(bottom - top) };
  await page.screenshot({ path: `${OUT}${file}`, clip });
  return clip;
}

async function settingsWindow(browser) {
  // For the side-by-side check: the same sizes the real screenshots were taken at.
  {
    const page = await open(browser, {
      path: "/",
      window: "main",
      appearance: "dark",
      viewport: MAIN,
      dpr: 2,
    });
    await section(page, "History");
    await page.screenshot({ path: `${OUT}compare/history-dark.png` });
    await page.context().close();
  }
  {
    const page = await open(browser, {
      path: "/",
      window: "main",
      appearance: "light",
      viewport: MAIN,
      dpr: 2,
      // The card the real screenshot was taken on, so the hint wraps the same.
      patch: (data) => {
        data.acceleration.summary = "NVIDIA GeForce RTX 4070 (Vulkan)";
      },
    });
    await section(page, "Voice");
    await page.screenshot({ path: `${OUT}compare/voice-light.png` });
    await page.context().close();
  }

  // Voice, with the sentence the video highlights located on the page.
  const voice = await open(browser, {
    path: "/",
    window: "main",
    appearance: "dark",
    viewport: MAIN,
    dpr: SHOT_DPR,
  });
  await section(voice, "Voice");
  const voiceBoxes = await voice.evaluate((sentence) => {
    const row = document.querySelector(".row");
    const hint = row.querySelector(".row__hint");
    const at = hint.textContent.indexOf(sentence);
    const range = document.createRange();
    range.setStart(hint.firstChild, at);
    range.setEnd(hint.firstChild, at + sentence.length);
    const plain = (r) => ({ x: r.x, y: r.y, width: r.width, height: r.height });
    return {
      row: plain(row.getBoundingClientRect()),
      label: plain(row.querySelector(".row__label").getBoundingClientRect()),
      hint: plain(hint.getBoundingClientRect()),
      figure: plain(row.querySelector(".figure").getBoundingClientRect()),
      sentence: [...range.getClientRects()].map(plain),
    };
  }, copy.local);
  const voiceClip = await shoot(voice, "voice.png", [voiceBoxes.row]);
  await voice.context().close();

  // Dictionary, taller (the window is resizable) so the list sits whole under
  // the editor, with a sentence in the preview so the substitution shows.
  const dictionary = await open(browser, {
    path: "/",
    window: "main",
    appearance: "dark",
    viewport: { width: MAIN.width, height: 760 },
    dpr: SHOT_DPR,
  });
  await section(dictionary, "Dictionary");
  await dictionary.getByPlaceholder("I work at anthropik").fill(copy.tryHeard);
  await dictionary.waitForSelector(".dict__preview--changed");
  const dictionaryBoxes = await dictionary.evaluate(({ heard, term }) => {
    const plain = (r) => ({ x: r.x, y: r.y, width: r.width, height: r.height });
    // Where a phrase sits inside the input: an <input> has no text nodes to
    // range over, so measure its value in its own computed font.
    const input = document.querySelector(".dict__try .input");
    const style = getComputedStyle(input);
    const context = document.createElement("canvas").getContext("2d");
    context.font = style.font;
    const before = input.value.slice(0, input.value.indexOf(heard));
    const field = input.getBoundingClientRect();
    const left =
      field.x + parseFloat(style.borderLeftWidth) + parseFloat(style.paddingLeft) + context.measureText(before).width;
    const preview = document.querySelector(".dict__preview");
    const range = document.createRange();
    const at = preview.textContent.indexOf(term);
    range.setStart(preview.firstChild, at);
    range.setEnd(preview.firstChild, at + term.length);
    const replaced = range.getBoundingClientRect();
    return {
      heard: { x: left, y: replaced.y - (preview.getBoundingClientRect().y - field.y), width: context.measureText(heard).width, height: replaced.height },
      term: plain(replaced),
      list: plain(document.querySelector(".dict").getBoundingClientRect()),
      rows: [...document.querySelectorAll(".dict__row")].map((row) => ({
        row: plain(row.getBoundingClientRect()),
        term: plain(row.querySelector(".dict__term").getBoundingClientRect()),
      })),
      tryField: plain(document.querySelector(".dict__try").getBoundingClientRect()),
      preview: plain(document.querySelector(".dict__preview").getBoundingClientRect()),
    };
  }, { heard: "cooper netties", term: "Kubernetes" });
  const dictionaryClip = await shoot(dictionary, "dictionary.png", [
    dictionaryBoxes.tryField,
    dictionaryBoxes.preview,
  ]);
  await dictionary.context().close();

  return {
    voice: { file: "voice.png", dpr: SHOT_DPR, ...voiceClip, boxes: voiceBoxes },
    dictionary: { file: "dictionary.png", dpr: SHOT_DPR, ...dictionaryClip, boxes: dictionaryBoxes },
  };
}

/**
 * The overlay, one PNG per video frame.
 *
 * The page runs on a virtual clock. Each frame: pin performance.now() to the
 * frame's time, deliver the engine events due by then, let React commit, then
 * pause every CSS transition the page is running and seek it to how long ago
 * it started. Every bar height, the width change and the colour fades are the
 * stylesheet's own transitions, drawn at exactly the frame's moment.
 */
async function overlay(browser) {
  const DPR = SHOT_DPR;
  const page = await open(browser, {
    path: "/overlay.html",
    window: "overlay",
    appearance: "dark",
    viewport: OVERLAY,
    dpr: DPR,
  });

  // The overlay page does not load Archivo itself (only the settings and
  // onboarding windows import @fontsource/archivo), so it falls back to
  // whatever the machine has. Here it gets the same files, as on a machine
  // with Archivo installed — which is how the real screenshots show it.
  await page.addStyleTag({
    content: [500, 700]
      .map(
        (weight) => `@font-face { font-family: "Archivo"; font-weight: ${weight};
  src: url("/node_modules/@fontsource/archivo/files/archivo-latin-${weight}-normal.woff2") format("woff2"); }`,
      )
      .join("\n"),
  });
  await page.evaluate(() => Promise.all([document.fonts.load('500 12px Archivo'), document.fonts.load('700 10px Archivo')]));

  await page.evaluate(() => {
    const started = new Map();
    window.__film = {
      /** Pause and seek every running transition to `now`. */
      seek(now) {
        for (const animation of document.getAnimations()) {
          if (!started.has(animation)) {
            started.set(animation, now);
            animation.pause();
          }
          animation.currentTime = Math.max(0, now - started.get(animation));
        }
      },
    };
  });

  const frameMs = 1000 / FPS;
  const listenFrom = 1000; // the counter reads 0:01 on the first captured frame
  const listenFrames = 72;
  const doneAt = listenFrom + listenFrames * frameMs;
  const doneFrames = 24;
  const levels = levelsFor(Math.ceil(doneAt / LEVEL_MS));
  let sent = 0;

  const step = async (ms, events, settle) => {
    await page.evaluate(
      async ({ ms, events, settle }) => {
        window.__klar.setTime(ms);
        for (const payload of events) window.__klar.emit("klar://event", payload);
        // Long enough for React to commit and the counter's 100 ms tick to run.
        await new Promise((r) => setTimeout(r, settle));
        window.__film.seek(ms);
      },
      { ms, events, settle },
    );
  };

  await rm(`${OUT}pill`, { recursive: true, force: true });
  await mkdir(`${OUT}pill`, { recursive: true });

  const pill = page.locator(".pill");
  const widths = [];
  const shoot = async () => {
    // Exactly the pill's border box, frame by frame, as its width animates.
    // Its own shadow falls outside the box and is left out: in the real
    // window it is cut off by the window's edge anyway, and the video gives
    // the pill a shadow of its own that follows whatever width it has.
    const clip = await pill.boundingBox();
    await page.screenshot({
      path: `${OUT}pill/${String(widths.length).padStart(4, "0")}.png`,
      clip,
      omitBackground: true,
    });
    widths.push(clip.width);
  };

  for (let f = 0; f * frameMs < doneAt; f++) {
    const ms = f * frameMs;
    const events = [];
    if (f === 0) events.push({ kind: "state", state: "recording" });
    // One level event per 50 ms of audio, as the engine sends them.
    while (sent < levels.length && sent * LEVEL_MS <= ms) {
      events.push({ kind: "level", peak: levels[sent] });
      sent++;
    }
    const capturing = ms >= listenFrom - 1;
    await step(ms, events, capturing ? 120 : 4);
    if (capturing) await shoot();
  }
  const firstDone = widths.length;
  for (let f = 0; f < doneFrames; f++) {
    const ms = doneAt + f * frameMs;
    const events =
      f === 0
        ? [
            { kind: "text", text: copy.clean, settled: true },
            { kind: "state", state: "injecting" },
          ]
        : [];
    await step(ms, events, 40);
    await shoot();
  }

  const height = (await pill.boundingBox()).height;
  await page.context().close();

  return {
    dir: "pill",
    dpr: DPR,
    fps: FPS,
    frames: widths.length,
    listening: { from: 0, to: firstDone - 1 },
    done: { from: firstDone, to: widths.length - 1 },
    height,
    /** CSS width of each frame's image; the height is always `height`. */
    widths,
  };
}

const stop = await vite();
const browser = await chromium.launch();
try {
  await rm(OUT, { recursive: true, force: true });
  await mkdir(`${OUT}compare`, { recursive: true });
  const shots = await settingsWindow(browser);
  const pill = await overlay(browser);
  await writeFile(`${OUT}manifest.json`, JSON.stringify({ ...shots, pill }, null, 2));
  console.log(`captured ${pill.frames} overlay frames, voice and dictionary`);
} finally {
  await browser.close();
  stop();
}
