# Changelog

Klar is unsigned and has no auto-update, so an installer is handed to somebody
and then lives on their machine until they replace it. That makes the version in
the filename the only way anyone — including whoever built it — can tell which
Klar they are running. Bump it in the same commit as the change.

The number lives in three files and they must agree: `Cargo.toml` (workspace),
`package.json`, and `src-tauri/tauri.conf.json`. The last is the one that names
the installer.

Newest first.

---

## Unreleased

**Updates.** Klar checks for a newer version at startup and installs one from
Settings → General → Updates. The check asks the update server for one file and
carries nothing about what was dictated; it is switchable and the row says so.
Every update is signature-checked before it runs.

An update replaces the previous Klar rather than sitting beside it — the
installer keys off the app identifier and a directory named after the product,
neither of which carries a version.

Needs a signing keypair and a manifest on the site before it does anything; see
the README. A build without them says "no update channel configured" rather than
failing to start.

---

## 0.2.0

**The dictionary, history and statistics.** The three panes that had said
"waiting on storage" now hold what they promised.

- Teach Klar a name it mangles and it spells it correctly from the next sentence
  on. Terms are biased into whisper's prompt before decoding and substituted
  afterwards for whatever the bias missed. The editor previews the substitution
  live, so an entry can be checked without holding the hotkey and hoping.
- Every dictation is recorded on this machine, labelled with the application it
  went into. Delete a row or all of them.
- Statistics per day, and time saved against a typing speed you set — the claim
  is only as honest as that number, so it is yours to change. The totals survive
  clearing the history: the text is the private part, how much you used the app
  is not.
- `klar-cli dict`, `history` and `stats` do all of the above from a terminal.
  `dict try` checks an entry against a sentence with no model and no microphone.

**Not only NVIDIA.** A Vulkan build covers AMD, Intel and NVIDIA alike and
carries no redistributable runtime, which makes its installer around a quarter
of a gigabyte smaller than the CUDA one. Verified on a desktop RTX 5070 Ti, a
laptop RTX 3060 and an AMD Radeon RX 7900 XT.

**Klar now says which device is doing the work.** A build made for one vendor's
card runs perfectly on another's, finds no device, quietly falls back to the CPU
and is roughly ten times slower with nothing to say so. Settings → Voice →
Processing names the device and explains the mismatch when there is one.

Model load runs a throwaway inference so the first dictation is not the one that
pays for the first inference.

---

## 0.1.0

The first version anybody else ran. Hold a hotkey anywhere, speak, release, and
clean text appears at the cursor: capture, streaming recognition, a local polish
stage that is off by default, clipboard injection that gives the clipboard back,
rebindable push-to-talk, start with Windows, light and dark, an overlay, and
onboarding that takes a fresh machine to a first dictation without a terminal.
