# Code signing

What it costs, what it actually buys, and what to do. Read the first section
before spending anything — the thing most people expect to buy is not the thing
that is for sale.

## Two signatures, unrelated

| | Updater signature | Code signature |
|---|---|---|
| Made with | minisign, via `tauri signer` | Authenticode, via `signtool` |
| Key | `$HOME\.klar\updater.key` | a certificate from a CA |
| Costs | nothing | $10–700 a year |
| Protects against | somebody serving a hostile update to every installed copy | nothing, directly |
| Status | working since 0.3.0 | not done |

The updater signature is the one that matters for safety, and it is already in
place. Code signing is about what Windows says to a person double-clicking the
installer. Do not let one stand in for the other in your head: an installer can
be code-signed by a well-known publisher and still ship a malicious update, and
Klar's updater refuses an unsigned update whether or not anything is
code-signed.

## What SmartScreen actually does

Today, unsigned, the sequence is:

1. Edge or Chrome warns that the file "isn't commonly downloaded".
2. Windows shows a blue box: **"Windows protected your PC"**, publisher
   "Unknown". The Run button is hidden behind a **More info** link.

Signing changes this in stages, and the stage nobody expects is the middle one.

**With an OV certificate or Azure Trusted Signing.** The blue box still appears
on a new certificate. What changes is that "Unknown publisher" is replaced by
your name, and — this is the part being bought — the reputation now accumulates
against the certificate rather than against each individual file. Every build
today starts from zero because every build is a new unknown file. Signed, the
tenth build inherits what the first nine earned. The warning stops appearing
once enough people have installed it; there is no published threshold and no
way to check progress.

**With an EV certificate.** Historically, immediate: no blue box from the first
download, because EV certificates were granted SmartScreen reputation on issue.
Microsoft has been quietly reducing that advantage and does not document the
current behaviour. Treat "instant trust" as likely but not promised.

**Neither removes the browser's "not commonly downloaded" line** on day one.
That one is per-file and per-URL and only downloads clear it.

So: signing does not make the warning go away tomorrow. It makes it go away
eventually, and it puts a name on it in the meantime. For an app sent to three
people that is worth little; for one on a public site it is the difference
between "somebody built this" and "nobody knows what this is".

## What to buy

Since June 2023 the CA/Browser Forum requires code-signing private keys to sit
on FIPS 140-2 Level 2 hardware. There is no longer such a thing as a cheap
certificate file you download and keep. Every option below is a consequence of
that rule.

### Azure Trusted Signing — the recommendation

About **$10 a month**. No hardware, no token to lose, no key to back up:
Microsoft holds the key in their HSM and issues a fresh short-lived certificate
for each signature. Timestamping means the signature outlives the certificate.

The catch is validation. Individuals — not just companies — can enrol, but
Microsoft requires a verifiable history of legal presence going back three
years, checked against public records and government ID. Enrolment takes days,
sometimes longer, and can be refused with little explanation. Start the
application before planning a release around it.

Once approved, the whole integration is one line, which
`scripts\sign.ps1 -Azure` writes for you.

### An OV certificate — the fallback

$200–450 a year from Sectigo, DigiCert, SSL.com and others. Issued to
individuals as well as companies, usually with a notarised identity check. It
arrives either on a posted USB token or as access to the CA's cloud HSM; the
cloud option is worth the small extra cost, because a posted token that is lost
or wiped means buying the certificate again.

Same SmartScreen behaviour as Trusted Signing, roughly four times the price.
Choose it if Trusted Signing enrolment is refused.

### An EV certificate — probably not

$400–700 a year, hardware token, and in practice a registered legal entity.
The reason to consider it is the day-one SmartScreen bypass, and that is the
part Microsoft has been walking back. Not worth it for Klar as it stands.

## Rehearse first

The build path can be proven before any of the above. `scripts\sign.ps1` makes
a self-signed certificate, points the build at it, and the installer comes out
signed:

```powershell
. .\scripts\env.ps1
.\scripts\sign.ps1 -Rehearse
npm run tauri build -- --features vulkan -c src-tauri/tauri.release.conf.json -c src-tauri/tauri.signing.conf.json
.\scripts\sign.ps1 -Verify C:\kv\release\bundle\nsis\Klar_0.3.1_x64-setup.exe
```

`-Verify` will report that the chain is untrusted. That is the correct result:
the certificate vouches for itself and nothing else vouches for it. What the
rehearsal establishes is that `signtool.exe` is found, that the config overlay
merges, and that both the executable and the NSIS installer come out signed —
everything a real certificate then inherits by changing one thumbprint.

**Do not publish a rehearsal build.** A signature Windows cannot chain is worse
than no signature: unsigned, SmartScreen says it does not know the publisher;
self-signed, it says the signature is invalid, which reads to anybody as
tampering.

## When a real certificate arrives

```powershell
.\scripts\sign.ps1 -Thumbprint <hex from the certificate>
```

or, for Trusted Signing:

```powershell
$env:AZURE_TENANT_ID = "..."
$env:AZURE_CLIENT_ID = "..."
$env:AZURE_CLIENT_SECRET = "..."
.\scripts\sign.ps1 -Azure "https://weu.codesigning.azure.net,<account>,<profile>"
```

Both write `src-tauri/tauri.signing.conf.json`, which is gitignored — a
thumbprint belongs to one machine and an Azure account to one subscription.
`scripts\sign.ps1 -Off` deletes it.

## Two things that will bite

**Signing is not optional once started.** A signed 0.4.0 followed by an
unsigned 0.4.1 does not return the user to the current state; it makes the
newer file look worse than the older one, and any reputation the certificate
earned is not transferred back to unsigned files. Decide to sign when the
process can be repeated for every release.

**The certificate is not the updater key.** Losing the code-signing certificate
costs money and a re-issue. Losing `$HOME\.klar\updater.key` permanently breaks
updates for every copy of Klar already installed, and no certificate anywhere
fixes it. Back up the second one; the first one can be replaced.
