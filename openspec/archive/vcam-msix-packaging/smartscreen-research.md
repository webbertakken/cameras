# SmartScreen mitigation research

## Problem

Users downloading the Cameras NSIS installer face a high-friction SmartScreen
prompt: **"Windows protected your PC"** with the app publisher shown as
"Unknown". The only way past it is "More info" then "Run anyway" — a pattern
that discourages non-technical users and makes the app look suspicious.

This affects both the `.exe` NSIS installer and any unsigned/self-signed
binaries.

---

## How SmartScreen works

SmartScreen (and its successor Smart App Control on Windows 11) checks
downloaded executables and installers against three signals:

1. **Digital signature** — is it signed with a certificate that chains to a
   trusted root CA?
2. **Reputation** — has this specific file (by hash) or publisher (by cert)
   been seen frequently enough in telemetry?
3. **Mark of the Web (MotW)** — was the file downloaded from the internet?
   (Files without MotW, e.g. built locally, skip SmartScreen entirely.)

SmartScreen has two tiers of trust:

- **Known publisher**: signed with an OV/EV cert that has established
  reputation. Shows publisher name, no warning.
- **Unknown publisher**: unsigned, or signed with a cert that has no/low
  reputation. Shows the scary blue/yellow warning.

### Smart App Control (SAC) — Windows 11 22H2+

SAC is a stricter variant that replaces SmartScreen on fresh Windows 11
installs. In **evaluation mode**, it observes silently. In **enforcement
mode**, it blocks unsigned apps entirely (no "Run anyway" option). SAC is
automatically disabled on most existing machines, but new Windows 11 installs
ship with it in evaluation mode.

Key difference: SAC **blocks** unknown apps outright. SmartScreen merely
**warns**. An unsigned installer will be completely blocked on SAC-enforced
machines.

---

## Options ranked by effectiveness

### 1. OV code signing certificate (recommended)

**What**: A standard Organisation Validation (OV) code signing certificate
from a public CA (DigiCert, Sectigo, SSL.com, etc.).

**How it helps**:

- Eliminates the "Unknown publisher" warning immediately — your publisher name
  appears in the SmartScreen prompt
- After building reputation (typically a few hundred downloads over days to
  weeks), SmartScreen stops showing any warning at all
- Works with Smart App Control — signed apps are allowed through even in
  enforcement mode, provided the cert gains reputation
- Signs both the NSIS installer `.exe` and the installed binaries

**Cost**: USD 70–400/year depending on provider and term length.

| Provider | 1-year price | 3-year price | Notes                              |
| -------- | ------------ | ------------ | ---------------------------------- |
| SSL.com  | ~USD 70      | ~USD 165     | Cheapest, good reputation          |
| Sectigo  | ~USD 90      | ~USD 210     | Via resellers (e.g. Comodo)        |
| DigiCert | ~USD 400     | ~USD 1000    | Premium, fastest reputation        |
| SignPath | Free (OSS)   | ~EUR 420     | Free tier for open-source projects |

**Reputation build time**: Variable. Microsoft does not publish thresholds.
Empirically:

- First release with a new OV cert: SmartScreen warns ("publisher: Your Name")
  but shows publisher identity — much less scary than "Unknown publisher"
- After ~500–2,000 unique downloads over 1–4 weeks, warnings typically stop
- Each new binary hash resets reputation for that specific file, but publisher
  reputation persists across releases
- Signing with a timestamp ensures reputation survives cert renewal

**Process**:

1. Purchase OV cert (requires business registration or personal ID validation)
2. Receive a hardware token (USB) or use cloud-based signing (e.g. SSL.com
   eSigner, Azure Trusted Signing)
3. Sign the NSIS installer in CI via `signtool.exe` or equivalent
4. Also sign the MSIX sparse package with the same cert

**Effort**: Low. Add a signing step to `build.yml`. Tauri supports
`TAURI_SIGNING_PRIVATE_KEY` for update signing, but for Authenticode signing,
use `signtool.exe` directly or a Tauri signing plugin.

**Verdict**: Best bang for the buck. The "Unknown publisher" problem disappears
on day one (publisher name shown). Full SmartScreen bypass follows within
weeks. Works with SAC.

---

### 2. EV code signing certificate

**What**: Extended Validation certificate. Historically guaranteed immediate
SmartScreen bypass with zero reputation build.

**How it helps**:

- Immediate full SmartScreen bypass — no reputation warm-up period
- Higher trust signal in the SmartScreen prompt
- Required for kernel-mode driver signing (not relevant here)

**Cost**: USD 250–700/year. Must use a hardware token (HSM/USB) — no
exportable PFX files.

**Important caveat (2024+)**: Microsoft has been tightening SmartScreen. As of
late 2024, even EV-signed binaries can trigger SmartScreen warnings for new
publishers. The "instant bypass" guarantee has weakened. Multiple developers
report that EV certs from new publishers still show warnings for the first few
releases.

**CI complexity**: EV certs require a physical USB token (e.g. SafeNet
eToken). This means:

- Self-hosted runner with the token plugged in, or
- Cloud HSM service (DigiCert KeyLocker, SSL.com eSigner, Azure Trusted
  Signing) — adds USD 20–50/month
- GitHub-hosted runners cannot use physical tokens

**Verdict**: Not worth the premium for an indie team. The instant bypass
guarantee is weakening, the cost is higher, and CI integration is more
complex. An OV cert achieves nearly the same result at lower cost and
complexity.

---

### 3. Azure Trusted Signing (formerly Azure Code Signing)

**What**: Microsoft's own cloud-based signing service. Launched GA in 2024.

**How it helps**:

- Microsoft-issued certificates — highest possible SmartScreen trust
- Immediate or near-immediate SmartScreen bypass (Microsoft's own certs have
  inherent reputation)
- No hardware tokens — signs via Azure API
- CI-friendly: works with GitHub Actions via Azure CLI
- Signs both Authenticode (exe/dll) and MSIX packages

**Cost**: USD 10/month (Basic tier). Requires an Azure account and identity
verification.

**Process**:

1. Create an Azure account and Trusted Signing resource
2. Complete identity verification (personal or organisation)
3. Create a certificate profile
4. Integrate with CI using `azure/trusted-signing-action@v0.5.0` or
   `signtool.exe` with the Trusted Signing dlib

**Caveats**:

- Requires Azure identity verification (similar rigour to OV cert validation)
- Relatively new service — some rough edges in tooling
- Tied to Azure ecosystem

**Verdict**: Excellent option if you're comfortable with Azure. Cheapest
recurring cost, best SmartScreen outcome (Microsoft's own certs), and
CI-native. Strong contender alongside traditional OV certs.

---

### 4. Microsoft Store distribution

**What**: Publish the app to the Microsoft Store (either as full MSIX or via
the Store's external installer support).

**How it helps**:

- Store-distributed apps bypass SmartScreen entirely — they are trusted by
  definition
- Automatic updates via the Store
- Discoverability via Store search
- No signing cost — Microsoft signs the package

**Cost**: One-time USD 19 developer registration fee (personal) or USD 99
(organisation).

**Caveats**:

- Store review process (1–3 days per submission, can be unpredictable)
- App must comply with Store policies (no kernel drivers, restricted
  capabilities need approval)
- Virtual camera COM registration via sparse package may conflict with Store
  sandboxing — needs investigation
- Store MSIX packaging is a full MSIX (not sparse), which conflicts with
  Tauri's NSIS installer approach
- Users who download from the Store expect Store-style updates, not custom
  update mechanisms
- Canon EDSDK redistribution may violate Store policies (proprietary SDK)

**Verdict**: Viable as a parallel distribution channel alongside the direct
download. Not a replacement — the direct download installer still needs
signing. Worth pursuing later once the app is more mature, but does not solve
the SmartScreen problem for direct downloads.

---

### 5. winget (Windows Package Manager)

**What**: Publish the app to the winget community repository or use a self-
hosted manifest.

**How it helps**:

- `winget install cameras` installs from a known repository — some users
  prefer this
- winget downloads are still subject to SmartScreen (winget just downloads the
  installer and runs it)
- **Does not bypass SmartScreen** — the installer still needs to be signed

**Cost**: Free. Submit a PR to `microsoft/winget-pkgs` with a manifest.

**Caveats**:

- The installer URL must be a stable download link (GitHub Releases works)
- winget does hash verification but does not exempt from SmartScreen
- Requires maintaining version manifests for each release

**Verdict**: Nice distribution channel but does not help with SmartScreen at
all. Worth doing eventually for discoverability, but orthogonal to this
problem.

---

### 6. Submit to Microsoft for SmartScreen reputation

**What**: Proactively submit the installer to Microsoft for reputation
analysis via the [SmartScreen file submission portal](https://www.microsoft.com/en-us/wdsi/filesubmission).

**How it helps**:

- Can accelerate reputation building for a specific binary
- Free

**Caveats**:

- Only works for signed binaries — unsigned submissions are ignored
- No guarantee of timeline or outcome
- Must re-submit for each new release
- Anecdotally unreliable — many developers report no effect

**Verdict**: Free complement to OV signing. Submit each release but do not
rely on it as a primary strategy.

---

### 7. Self-signed certificate (current approach)

**What**: The `create-dev-cert.ps1` script creates a self-signed cert and
installs it in the local machine's trusted root store.

**How it helps**:

- Works for local development and testing
- **Does not help with SmartScreen for distributed builds** — self-signed
  certs have no chain of trust that SmartScreen recognises
- SmartScreen treats self-signed exactly like unsigned

**Cost**: Free.

**Verdict**: Fine for dev builds. Useless for distribution. Must be replaced
with a real CA-issued cert for releases.

---

## Recommendation for Cameras app

### Immediate (before next release)

**Azure Trusted Signing** or **OV code signing certificate** — pick one:

| Factor                | Azure Trusted Signing    | Traditional OV cert      |
| --------------------- | ------------------------ | ------------------------ |
| Cost                  | USD 10/month             | USD 70–400/year          |
| SmartScreen bypass    | Near-immediate           | Days to weeks            |
| CI integration        | Native (Azure action)    | signtool.exe + PFX/token |
| Vendor lock-in        | Azure ecosystem          | Any CA, portable PFX     |
| Identity verification | Required (similar to OV) | Required                 |
| Hardware token needed | No                       | No (OV) / Yes (EV)       |

**My recommendation: Azure Trusted Signing.** It is the cheapest, has the best
SmartScreen outcome (Microsoft's own certificates), integrates cleanly with
GitHub Actions, and avoids hardware token hassles. The USD 10/month cost is
negligible.

If Azure is undesirable, an OV cert from SSL.com (~USD 70/year) is the next
best option. The initial "publisher name but still warned" phase is
acceptable — it builds trust over a few weeks of downloads.

### Short-term (next 1–2 months)

1. Sign the NSIS installer with the chosen certificate
2. Sign the MSIX sparse package with the same certificate
3. Sign all shipped DLLs (`vcam_source.dll`, `cameras.exe`)
4. Submit each release to the SmartScreen file submission portal
5. Add signing step to `build.yml` CI workflow

### Medium-term (3–6 months)

1. Publish to the Microsoft Store as a parallel channel
2. Publish to winget for developer convenience
3. Both are additive — the direct download with Authenticode signing remains
   the primary channel

### What NOT to do

- **Do not use an EV cert** — the premium is not justified for an indie app,
  the instant bypass guarantee is weakening, and CI integration with hardware
  tokens is painful
- **Do not rely on self-signed certs** — they provide zero SmartScreen benefit
- **Do not skip signing** — Smart App Control will outright block unsigned apps
  on an increasing number of Windows 11 machines
- **Do not expect winget alone to solve this** — winget does not bypass
  SmartScreen

---

## Implementation notes for CI signing

### Azure Trusted Signing in GitHub Actions

```yaml
- name: Sign installer (Windows)
  if: matrix.target == 'windows-x64' && startsWith(github.ref, 'refs/tags/v')
  uses: azure/trusted-signing-action@v0.5.0
  with:
    azure-tenant-id: ${{ secrets.AZURE_TENANT_ID }}
    azure-client-id: ${{ secrets.AZURE_CLIENT_ID }}
    azure-client-secret: ${{ secrets.AZURE_CLIENT_SECRET }}
    endpoint: https://eus.codesigning.azure.net/
    trusted-signing-account-name: ${{ secrets.SIGNING_ACCOUNT }}
    certificate-profile-name: ${{ secrets.SIGNING_PROFILE }}
    files-folder: src-tauri/target/release/bundle/nsis/
    files-folder-filter: exe
    file-digest: SHA256
    timestamp-rfc3161: http://timestamp.acs.microsoft.com
    timestamp-digest: SHA256
```

### Traditional OV cert signing

```yaml
- name: Sign installer (Windows)
  if: matrix.target == 'windows-x64' && startsWith(github.ref, 'refs/tags/v')
  shell: pwsh
  run: |
    $signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" |
      Sort-Object { $_.Directory.Parent.Name } -Descending |
      Select-Object -First 1
    $files = Get-ChildItem "src-tauri\target\release\bundle\nsis\*.exe"
    foreach ($file in $files) {
      & $signtool.FullName sign /fd SHA256 /f cert.pfx /p "${{ secrets.CERT_PASSWORD }}" `
        /tr http://timestamp.digicert.com /td SHA256 $file.FullName
    }
```

---

## References

- [Microsoft SmartScreen documentation](https://learn.microsoft.com/en-us/windows/security/operating-system-security/virus-and-threat-protection/microsoft-defender-smartscreen/)
- [Smart App Control](https://support.microsoft.com/en-us/topic/what-is-smart-app-control-285ea03d-fa88-4983-a7cb-a777c5c0d5b4)
- [Azure Trusted Signing](https://learn.microsoft.com/en-us/azure/trusted-signing/overview)
- [SmartScreen file submission](https://www.microsoft.com/en-us/wdsi/filesubmission)
- [Sparse MSIX packages](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/grant-identity-to-nonpackaged-apps)
