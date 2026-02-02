import { readdir, copyFile, access } from "fs/promises";
import { join } from "path";

// Get build target from environment (set by CI) or default to macOS
const buildTarget = process.env.BUILD_TARGET || "aarch64-apple-darwin";
const isWindows = buildTarget.includes("windows");
const isMacOS = buildTarget.includes("apple-darwin");

// Platform-specific paths
const macDmgDir = `src-tauri/target/${buildTarget}/release/bundle/dmg`;
const macTgzDir = `src-tauri/target/${buildTarget}/release/bundle/macos`;
const winNsisDir = `src-tauri/target/${buildTarget}/release/bundle/nsis`;

// Fallback paths for local builds (no target subdirectory)
const localDmgDir = "src-tauri/target/release/bundle/dmg";
const localTgzDir = "src-tauri/target/release/bundle/macos";

async function exists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

async function rename() {
  // macOS DMG
  const dmgDir = (await exists(macDmgDir)) ? macDmgDir : localDmgDir;
  if (isMacOS && (await exists(dmgDir))) {
    const dmgFiles = await readdir(dmgDir);
    const dmg = dmgFiles.find(
      (f) => f.endsWith(".dmg") && f.toLowerCase().startsWith("caipi_")
    );
    if (dmg) {
      const arch = dmg.includes("aarch64") ? "aarch64" : "x64";
      const dest = join(dmgDir, `caipi_${arch}.dmg`);
      await copyFile(join(dmgDir, dmg), dest);
      console.log(`Created: ${dest}`);
    }
  }

  // macOS tar.gz (for updater)
  const tgzDir = (await exists(macTgzDir)) ? macTgzDir : localTgzDir;
  if (isMacOS && (await exists(tgzDir))) {
    const tgzFiles = await readdir(tgzDir);
    const tgz = tgzFiles.find(
      (f) => f.endsWith(".tar.gz") && f.toLowerCase().startsWith("caipi")
    );
    if (tgz) {
      const dest = join(tgzDir, "caipi.app.tar.gz");
      await copyFile(join(tgzDir, tgz), dest);
      console.log(`Created: ${dest}`);
    }

    // Copy signature file too
    const sig = tgzFiles.find((f) => f.endsWith(".tar.gz.sig"));
    if (sig) {
      const dest = join(tgzDir, "caipi.app.tar.gz.sig");
      await copyFile(join(tgzDir, sig), dest);
      console.log(`Created: ${dest}`);
    }
  }

  // Windows NSIS installer
  if (isWindows && (await exists(winNsisDir))) {
    const nsisFiles = await readdir(winNsisDir);
    const exe = nsisFiles.find(
      (f) =>
        f.endsWith(".exe") &&
        !f.includes("uninstall") &&
        f.toLowerCase().startsWith("caipi")
    );
    if (exe) {
      const dest = join(winNsisDir, "caipi_x64.exe");
      await copyFile(join(winNsisDir, exe), dest);
      console.log(`Created: ${dest}`);
    }
  }
}

rename().catch(console.error);
