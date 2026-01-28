# GitHub Actions for Tauri macOS Builds - Research

## Overview

This document outlines the approach for moving the Caipi build process to GitHub Actions, enabling future cross-platform builds for Linux and Windows.

## Architecture

```
┌─────────────────────────────────┐      ┌─────────────────────────────┐
│  pietz/caipi (private)          │      │  pietz/caipi.ai (public)    │
│                                 │      │                             │
│  - Source code                  │──────│  - GitHub Release           │
│  - GitHub Actions workflow      │      │  - Casks/caipi.rb update    │
│  - Secrets for signing          │      │                             │
└─────────────────────────────────┘      └─────────────────────────────┘
```

## Required GitHub Secrets

Set these in `pietz/caipi` → Settings → Secrets → Actions:

| Secret | Description |
|--------|-------------|
| `APPLE_CERTIFICATE` | Base64-encoded `.p12` file |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the .p12 |
| `APPLE_ID` | Your Apple Developer email |
| `APPLE_PASSWORD` | App-specific password (not your account password) |
| `APPLE_TEAM_ID` | Found in Apple Developer membership page |
| `KEYCHAIN_PASSWORD` | Any password (for temp keychain in CI) |
| `TAURI_SIGNING_PRIVATE_KEY` | For updater signatures |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password for signing key |
| `CAIPI_AI_PAT` | Personal Access Token with write access to pietz/caipi.ai |

To get the certificate as base64:
```bash
openssl base64 -in MyCertificate.p12 -out MyCertificate-base64.txt
```

## Workflow Template

```yaml
# .github/workflows/release.yml
name: Release

on:
  workflow_dispatch:
    inputs:
      version:
        description: 'Version to release (e.g., 0.1.8)'
        required: true

jobs:
  build-macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: lts/*
          cache: npm

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-apple-darwin

      - uses: swatinem/rust-cache@v2
        with:
          workspaces: ./src-tauri -> target

      - run: npm ci

      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
          KEYCHAIN_PASSWORD: ${{ secrets.KEYCHAIN_PASSWORD }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
        with:
          args: --target aarch64-apple-darwin

      - name: Rename artifacts
        run: node scripts/release-rename.js

      - name: Create release on public repo
        uses: softprops/action-gh-release@v1
        with:
          repository: pietz/caipi.ai
          token: ${{ secrets.CAIPI_AI_PAT }}
          tag_name: v${{ inputs.version }}
          files: |
            src-tauri/target/release/bundle/dmg/caipi_aarch64.dmg
            src-tauri/target/release/bundle/macos/caipi.app.tar.gz
            src-tauri/target/release/bundle/macos/caipi.app.tar.gz.sig
```

## Cross-Repo Publishing Options

### Option 1: softprops/action-gh-release
- Specify `repository:` parameter to release to a different repo
- Requires a PAT with `repo` scope
- Best for creating releases with artifacts

### Option 2: cpina/github-action-push-to-another-repository
- For pushing files/commits to another repo
- Useful for updating the Homebrew cask formula

## Key Considerations

1. **Notarization takes time** - Apple notarization can take several minutes. The `tauri-action` handles this automatically when credentials are provided.

2. **Free Apple Developer account won't work** - Notarization requires a paid Apple Developer Program membership ($99/year).

3. **Cask formula update** - Need a second step to update `Casks/caipi.rb` with the new SHA256 and version, then push to the public repo.

4. **Universal binaries** - To support Intel Macs, add a second matrix entry with `--target x86_64-apple-darwin`.

## Future: Multi-Platform Builds

```yaml
strategy:
  matrix:
    include:
      - platform: macos-latest
        args: --target aarch64-apple-darwin
      - platform: macos-latest
        args: --target x86_64-apple-darwin
      - platform: ubuntu-22.04
        args: ''
      - platform: windows-latest
        args: ''
```

Linux requires additional dependencies:
```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

## References

- [Tauri GitHub Pipelines Documentation](https://v2.tauri.app/distribute/pipelines/github/)
- [tauri-apps/tauri-action](https://github.com/tauri-apps/tauri-action)
- [Tauri macOS Code Signing](https://v2.tauri.app/distribute/sign/macos/)
- [softprops/action-gh-release](https://github.com/softprops/action-gh-release)
- [Cross-repo artifact publishing guide](https://dev.to/oysterd3/how-to-release-built-artifacts-from-one-to-another-repo-on-github-3oo5)
