# GCS Canon EDSDK for CI

## Context

The Canon EDSDK is proprietary (`.proprietary/` is gitignored). CI needs it to
build Windows binaries with Canon support (`--features canon`).

### Current state

- `checks.yml` runs on **Linux** — Canon checks compile mock code only
  (FFI guarded by `#[cfg(target_os = "windows")]`). Works without SDK.
- `build.yml` Windows build runs on `windows-latest` but does **not** pass
  `--features canon` — current releases ship without Canon support.

### Files needed (3 total)

| File           | Path under `.proprietary/Canon/EDSDKv132010W/EDSDKv132010W/Windows/EDSDK_64/` |
| -------------- | ----------------------------------------------------------------------------- |
| `EDSDK.lib`    | `Library/EDSDK.lib` — link-time library                                       |
| `EDSDK.dll`    | `Dll/EDSDK.dll` — runtime DLL                                                 |
| `EdsImage.dll` | `Dll/EdsImage.dll` — runtime DLL                                              |

`build.rs` expects these at the relative path above. It emits
`rustc-link-search` for `Library/` and copies DLLs from `Dll/` to the target
directory.

## Plan

### 1. GCS bucket setup (manual, one-time)

```bash
# Create bucket (single-region, private)
gcloud storage buckets create gs://cameras-ci-sdk \
  --location=europe-west4 \
  --uniform-bucket-level-access

# Upload SDK files preserving directory structure
cd .proprietary/Canon/EDSDKv132010W/EDSDKv132010W/Windows/EDSDK_64
gcloud storage cp Library/EDSDK.lib gs://cameras-ci-sdk/canon-edsdk/Library/
gcloud storage cp Dll/EDSDK.dll gs://cameras-ci-sdk/canon-edsdk/Dll/
gcloud storage cp Dll/EdsImage.dll gs://cameras-ci-sdk/canon-edsdk/Dll/
```

### 2. Service account (manual, one-time)

```bash
# Create service account with read-only access
gcloud iam service-accounts create cameras-ci-sdk-reader \
  --display-name="CI SDK Reader"

# Grant read-only access to bucket
gcloud storage buckets add-iam-policy-binding gs://cameras-ci-sdk \
  --member="serviceAccount:cameras-ci-sdk-reader@PROJECT.iam.gserviceaccount.com" \
  --role="roles/storage.objectViewer"

# Create key (save as JSON)
gcloud iam service-accounts keys create sa-key.json \
  --iam-account=cameras-ci-sdk-reader@PROJECT.iam.gserviceaccount.com
```

### 3. GitHub secret

Add `GCP_SA_KEY` as a repository secret containing the JSON key file contents.

### 4. CI workflow changes

#### `build.yml` — Windows build

Add SDK download step before the build, enable canon feature for Windows:

```yaml
- target: windows-x64
  os: windows-latest
  args: --features canon # <-- enable canon
```

New step (after checkout, before build):

```yaml
- name: Download Canon EDSDK
  if: matrix.target == 'windows-x64'
  uses: google-github-actions/auth@v2
  with:
    credentials_json: ${{ secrets.GCP_SA_KEY }}

- name: Download Canon EDSDK files
  if: matrix.target == 'windows-x64' && env.GCP_SA_KEY != ''
  env:
    GCP_SA_KEY: ${{ secrets.GCP_SA_KEY }}
  run: |
    gcloud storage cp -r gs://cameras-ci-sdk/canon-edsdk/* .proprietary/Canon/EDSDKv132010W/EDSDKv132010W/Windows/EDSDK_64/
```

#### `checks.yml` — no changes needed

Linux CI already handles Canon feature via mocks. No SDK required.

### 5. Graceful fallback

When `GCP_SA_KEY` is not available (forks, new contributors):

- `build.rs` prints a warning but doesn't fail when SDK files are missing
- Windows build will compile without Canon FFI (feature enabled but no lib to link)
- Actually this WILL fail linking — need conditional: only add `--features canon`
  when SDK was successfully downloaded

Revised approach:

```yaml
- name: Download Canon EDSDK
  id: canon-sdk
  if: matrix.target == 'windows-x64'
  continue-on-error: true
  run: |
    if [ -z "${{ secrets.GCP_SA_KEY }}" ]; then
      echo "skip=true" >> $GITHUB_OUTPUT
      exit 0
    fi
    # ... download steps ...
    echo "skip=false" >> $GITHUB_OUTPUT

# Build step uses canon feature only when SDK available
- name: Build (CI)
  uses: tauri-apps/tauri-action@v0
  with:
    args: ${{ matrix.target == 'windows-x64' && steps.canon-sdk.outputs.skip != 'true' && '--features canon' || matrix.args }}
```

### 6. Security

- Service account has **read-only** access to one bucket
- Key stored as GitHub encrypted secret (only available to repo collaborators)
- Forks cannot access secrets — build degrades gracefully (no Canon)
- No credentials in logs (gcloud auth handles this)
