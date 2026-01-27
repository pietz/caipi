import { readdir, copyFile } from "fs/promises";
import { join } from "path";

const dmgDir = "src-tauri/target/release/bundle/dmg";
const tgzDir = "src-tauri/target/release/bundle/macos";

async function rename() {
  // Rename DMG (handle both "caipi_" and "Caipi_" prefixes)
  const dmgFiles = await readdir(dmgDir);
  const dmg = dmgFiles.find((f) => f.endsWith(".dmg") && f.toLowerCase().startsWith("caipi_"));
  if (dmg) {
    const arch = dmg.includes("aarch64") ? "aarch64" : "x64";
    const dest = join(dmgDir, `caipi_${arch}.dmg`);
    await copyFile(join(dmgDir, dmg), dest);
    console.log(`Created: ${dest}`);
  }

  // Rename tar.gz (for updater) - handle both "caipi" and "Caipi" prefixes
  const tgzFiles = await readdir(tgzDir);
  const tgz = tgzFiles.find((f) => f.endsWith(".tar.gz") && f.toLowerCase().startsWith("caipi"));
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

rename().catch(console.error);
