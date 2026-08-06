# Brief: the installer, and the first ten seconds after it

Hand this to a design session. It is written to be pasted whole — the
constraints at the end are the part that makes the output usable rather than a
picture of an installer.

---

## What Klar is

A desktop dictation app for Windows. It lives in the system tray. You hold a
hotkey anywhere in the OS, speak, release, and clean written text appears at the
cursor in whatever application is focused. Speech is recognised on the machine
itself and never uploaded.

It is not a transcriber. Raw speech is transcribed and then tidied: filler words
removed, punctuation fixed, spoken self-corrections resolved — "Tuesday, no,
Friday" becomes "Friday". Messy speech in, finished text out.

The people installing it are not developers. They were sent a link by somebody
who uses it.

## What to design

**1. The Windows installer (NSIS).** Three images and an icon, no more — this is
a stock NSIS installer and its layout cannot be rearranged:

| Asset | Exact size | Where it appears |
|---|---|---|
| Header image | 150 × 57 px, BMP | Top-right strip of every page after the first |
| Sidebar image | 164 × 314 px, BMP | Full-height left panel of the welcome and finish pages |
| Installer icon | `.ico`, 256 × 256 down to 16 × 16 | The setup file in Explorer and in the title bar |
| Uninstaller icon | same | Add/Remove Programs |

The header is a strip 150 px wide and 57 tall. It is not a canvas — a wordmark
and air, nothing that needs to be read at speed. The sidebar is the only piece
with room to say anything.

**2. The first-run window.** This exists in code already (`Klar Onboarding.dc.html`)
and works: microphone test, model download with a progress bar, hotkey choice,
and a text box to dictate into. What it lacks is a reason to feel good about
what was just installed. Redesign it if it earns it; leave it if it does not.

**3. The moment after "Finish".** Klar starts in the tray and shows the
onboarding window. Nothing else happens. That is correct behaviour and a wasted
moment — this is the only time the user is looking for what they just installed.

## What already exists — use it, do not replace it

`design/Klar Tokens.dc.html` holds the design system, and it is not decorative
guidance. The interface is built from these tokens and new colours would not
match anything already on screen.

- **Ground** `#f3f2f2` light, **Ink** `#201e1d` dark. The dark theme inverts
  them; both must work.
- **Signal** `#ec3013` — one red. It marks what is live and what is the primary
  action, and it is used sparingly enough that it still means something. Do not
  introduce a second accent.
- **Archivo**, one family. 800 for display, 600 for titles and figures, 400 for
  body, 700 uppercase with wide tracking for labels.
- **Square corners.** Every radius token is `0px`. This is deliberate.
- **2 px rules** as the only divider. No shadows, no gradients, no glass.

The existing windows are still, dense and typographic — closer to a piece of
equipment than to a consumer app. The installer should look like it came from
the same shop.

## Constraints that make the output usable

- **BMP, not PNG.** NSIS takes 24-bit BMP for the header and sidebar. Deliver
  BMPs, and PNG sources beside them.
- **The sidebar is 164 px wide.** Anything smaller than about 11 px will not
  survive it. Assume the user's display scaling is 125% or 150% and the image is
  resampled badly, because on most Windows laptops it is.
- **No photography, no stock imagery, no illustration of a microphone.** Klar
  has no mascot and does not need one.
- **The installer runs for about four seconds.** Nobody reads it. It has one
  job: to look like something a person built on purpose, in the two seconds
  before the progress bar finishes.
- **It is unsigned.** SmartScreen shows "Windows protected your PC" first, and
  the Run button is hidden behind "More info". The first thing the user sees is
  Windows saying it does not know who wrote this. Design the welcome page in
  full knowledge that it follows a warning.

## What to deliver

1. The four assets at the exact sizes above, as flat files.
2. A one-screen rationale: what the sidebar says and why it is the right thing
   to say to somebody who has just clicked past a security warning.
3. Any onboarding changes as HTML matching the existing `.dc.html` files, using
   the tokens by name rather than by value.

## What not to do

Do not design an installer with steps Klar does not have. There is no
destination chooser, no component picker, no licence page, and no offer to
install anything else. It installs for the current user, needs no administrator,
and replaces any earlier version in place. Four screens: welcome, progress,
finish, and the error page nobody plans for.
