# Package a macOS app from Linux

A Linux Docker job can turn a KUI project into a macOS `.app` using a compatible
macOS runner. After it assembles the bundle, the job can sign and notarize it
for direct distribution. Recipients open the finished app with Gatekeeper
enabled. [Apple's Developer ID distribution guide](https://developer.apple.com/developer-id/)

## Assemble the app

Run from the repository checkout in a Linux build container with the Linux KUI
CLI on `PATH`. To build the CLI in that container, install the
[Linux build prerequisites](setup.md#linux) and run
`cargo build --locked -p kui-cli`. Mount a macOS ARM64 `kui` executable read-only at
`/work/kui-darwin-arm64`. Use the runner version pinned in the project's
`kui.config.lua`, or the CLI's version when the project leaves it unpinned.
The runner can come from a KUI release or a macOS build of that version.

```sh
kui pack examples/lua/tutorial/tasks \
  --format app --target darwin-arm64 \
  --runner /work/kui-darwin-arm64 --out /work/dist
```

The output is `/work/dist/Tasks-1.0.0-darwin-arm64.app.zip`. Inside the bundle,
`Contents/MacOS/Tasks` is the native runner and `Contents/Resources/app.zip`
holds the Lua project. Sign the final bundle after KUI puts the project and
icon in it.

## Verify a local build

With `rcodesign` and `unzip` installed in the container, unpack into a fresh
directory and run `rcodesign sign Tasks.app` there. It gives the app an ad hoc
signature, writing it in place and replacing an existing signature. On a
development Mac, `codesign --verify --strict Tasks.app` checks that signature.
This is the local launch check. Use the next section for an app downloaded
by another Mac. [Signing with rcodesign](https://gregoryszorc.com/docs/apple-codesign/stable/apple_codesign_rcodesign_signing.html)

## Sign and notarize for distribution

Install `rcodesign`, `unzip` and `zip` in the Linux job. Provide three
read-only secret files at run time.

| Variable | File |
| --- | --- |
| `MACOS_P12_FILE` | The **Developer ID Application** certificate and private key exported as a `.p12` |
| `MACOS_P12_PASSWORD_FILE` | The password for that export |
| `APPLE_NOTARY_API_KEY_FILE` | App Store Connect API credentials encoded as JSON by `rcodesign encode-app-store-connect-api-key` |

The job needs network access to Apple's timestamp and notary services.
`--for-notarization` prepares the signature for notarization, including the
hardened runtime. `--staple` waits for Apple to accept the submission and
attaches the resulting ticket to the app. Create the distribution ZIP from
that bundle. [Apple's notarization requirements](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution) · [rcodesign notarization commands](https://gregoryszorc.com/docs/apple-codesign/stable/apple_codesign_rcodesign_notarizing.html)

```sh
set -eu
dist=/work/dist
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
unzip -q "$dist/Tasks-1.0.0-darwin-arm64.app.zip" -d "$stage"

rcodesign sign --for-notarization \
  --p12-file "$MACOS_P12_FILE" \
  --p12-password-file "$MACOS_P12_PASSWORD_FILE" \
  "$stage/Tasks.app"
rcodesign notary-submit --api-key-file "$APPLE_NOTARY_API_KEY_FILE" \
  --staple "$stage/Tasks.app"

(cd "$stage" && zip -qry Tasks-macos-arm64.zip Tasks.app)
mv "$stage/Tasks-macos-arm64.zip" "$dist/"
(cd "$dist" && sha256sum Tasks-macos-arm64.zip > Tasks-macos-arm64.zip.sha256)
```

Keep the submission ID in the build log. If notarization fails, obtain Apple's
diagnostics with `rcodesign notary-log --api-key-file "$APPLE_NOTARY_API_KEY_FILE" SUBMISSION_ID`.
Repeat the signing and notarization steps after changing the app bundle.

## Check the distribution ZIP on macOS

Download `Tasks-macos-arm64.zip` through the channel your users will use on a
separate Apple Silicon Mac. Unzip it and run the checks from the directory
containing `Tasks.app`.

```sh
codesign --verify --strict --verbose=2 Tasks.app
xcrun stapler validate Tasks.app
syspolicy_check distribution Tasks.app
```

Open the app in Finder and add a task. Quit and reopen it to check that the task
was saved. `syspolicy_check` is available on recent macOS versions, and the
Finder launch exercises the receiving Mac's actual policy.
[Apple's distribution testing guide](https://developer.apple.com/documentation/xcode/packaging-mac-software-for-distribution) · [Gatekeeper diagnostics](https://developer.apple.com/forums/thread/813858)
