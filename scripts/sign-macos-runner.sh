#!/usr/bin/env bash
# Sign the final runner before pack-runner.nu copies it into release/npm assets.
# Credentials live only in a temporary keychain, removed on success or failure.
set -euo pipefail
set +x
trap 'echo "Signing failed at line $LINENO (exit $?)." >&2' ERR

for name in MACOS_CERTIFICATE_P12 MACOS_CERTIFICATE_PASSWORD \
    MACOS_SIGN_IDENTITY APPLE_ID APPLE_TEAM_ID APPLE_APP_PASSWORD; do
    if [[ -z "${!name:-}" ]]; then
        echo "Missing required secret: $name" >&2
        exit 1
    fi
done
if [[ "$MACOS_SIGN_IDENTITY" != "Developer ID Application: "* ]]; then
    echo 'MACOS_SIGN_IDENTITY must name a Developer ID Application certificate.' >&2
    exit 1
fi
binary="${1:?usage: sign-macos-runner.sh path/to/kui}"
[[ -f "$binary" ]] || { echo "Runner not found: $binary" >&2; exit 1; }
: "${RUNNER_TEMP:?RUNNER_TEMP must name the runner temporary directory}"
umask 077
work_dir=$(mktemp -d "$RUNNER_TEMP/kui-signing.XXXXXX")
keychain="$work_dir/signing.keychain-db"
original_keychains=()
while IFS= read -r path; do
    original_keychains+=("$path")
done < <(security list-keychains -d user | sed -E 's/^[[:space:]]*"//; s/"[[:space:]]*$//')
cleanup() {
    security list-keychains -d user -s "${original_keychains[@]}" >/dev/null 2>&1 || true
    security delete-keychain "$keychain" >/dev/null 2>&1 || true
    rm -rf "$work_dir"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
keychain_password=$(openssl rand -hex 32)
printf '%s' "$MACOS_CERTIFICATE_P12" | base64 --decode > "$work_dir/certificate.p12"
security create-keychain -p "$keychain_password" "$keychain"
security set-keychain-settings -lut 7200 "$keychain"
security unlock-keychain -p "$keychain_password" "$keychain"
security import "$work_dir/certificate.p12" -k "$keychain" \
    -P "$MACOS_CERTIFICATE_PASSWORD" -T /usr/bin/codesign >/dev/null
security set-key-partition-list -S apple-tool:,apple:,codesign: -s \
    -k "$keychain_password" "$keychain" >/dev/null
rm "$work_dir/certificate.p12"

# codesign's identity/key lookup still needs the keychain in the user search
# list, even with --keychain. Keep the defaults for certificate-chain lookup.
security list-keychains -d user -s "$keychain" "${original_keychains[@]}"
codesign --force --sign "$MACOS_SIGN_IDENTITY" --keychain "$keychain" \
    --identifier dev.qxuken.kui --options runtime --timestamp "$binary"
codesign --verify --strict --verbose=2 "$binary"

# Store Apple's credentials in the disposable keychain, not the login keychain.
xcrun notarytool store-credentials kui-release --keychain "$keychain" \
    --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" \
    --password "$APPLE_APP_PASSWORD"
ditto -c -k "$binary" "$work_dir/kui.zip"
diagnostics="$RUNNER_TEMP/kui-notarization"
mkdir -p "$diagnostics"
# Preserve the response even on rejection/timeout; a zero exit alone does not
# establish acceptance. The submission id also lets a maintainer resume a wait.
submit_exit=0
xcrun notarytool submit "$work_dir/kui.zip" --keychain "$keychain" \
    --keychain-profile kui-release --wait --timeout 60m --output-format json \
    > "$diagnostics/submission.json" || submit_exit=$?
cat "$diagnostics/submission.json"
submission_id=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' \
    "$diagnostics/submission.json")
xcrun notarytool log "$submission_id" --keychain "$keychain" \
    --keychain-profile kui-release "$diagnostics/log.json" || true
if [[ "$submit_exit" != 0 ]]; then
    exit "$submit_exit"
fi
python3 - "$diagnostics/submission.json" <<'PY'
import json
import sys

with open(sys.argv[1]) as response:
    status = json.load(response).get("status")
if status != "Accepted":
    sys.exit(f"Notarization was not accepted: {status}; see the diagnostic artifact.")
PY
# Standalone executables cannot be stapled. The unchanged signed binary is
# notarized; Gatekeeper looks up its ticket online.
codesign --verify --strict --verbose=2 "$binary"
