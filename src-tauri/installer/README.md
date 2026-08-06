# Installer artwork

Four files NSIS reads and one thing to know about each. They come from the
design session briefed in `design/installer-brief.md`; the master artwork and
the reasoning are in `design/Klar Installer.dc.html` and
`design/Klar Installer Assets.dc.html`.

| File | Size | Where it appears |
|---|---|---|
| `header.bmp` | 150 × 57, 24-bit BMP | The strip at the top of every page after the first, in the installer and the uninstaller alike |
| `sidebar.bmp` | 164 × 314, 24-bit BMP | The full-height panel on the welcome and finish pages |
| `installer.ico` | 256/48/32/16 | The setup file in Explorer and in the title bar |
| `uninstaller.ico` | 256/48/32/16 | Add/Remove Programs |

`header.png` and `sidebar.png` are the same images in a format you can open
without a Windows machine. Nothing reads them; they are there so a change can
be reviewed in a diff.

**The BMPs must stay 24-bit and uncompressed.** NSIS reads bottom-up BI_RGB and
nothing else — no PNG, no RLE, no alpha channel. A bitmap saved with
transparency loads as garbage or not at all, and the installer is the last
place anybody looks.

**Do not resample the small icon entries.** 16 and 32 px are drawn, not scaled;
running the 256 through an image editor's downsize produces a grey smear at the
size Windows actually shows in the taskbar. If the artwork changes, the small
sizes come from the design session too.

The 256 px entry of each `.ico` is PNG-compressed. As delivered they were
uncompressed DIBs at 279 KB each — half a megabyte of icon inside an installer
whose entire size is the argument for the Vulkan build over the CUDA one. The
smaller entries were left byte for byte alone and the 256 px pixels are
identical to what was delivered.

The sidebar says where speech goes rather than what the app does, because it is
shown to somebody who has just clicked through "Windows protected your PC" —
see `docs/signing.md` for why that warning appears and what removes it.
