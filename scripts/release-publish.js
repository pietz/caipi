import { readFile, writeFile, copyFile } from "fs/promises";
import { execSync } from "child_process";
import { join } from "path";

const CAIPI_AI_PATH = "/Users/pietz/Private/caipi.ai";
const BUNDLE_PATH = "src-tauri/target/release/bundle";

async function publish() {
  // 1. Get version from tauri.conf.json
  const tauriConf = JSON.parse(await readFile("src-tauri/tauri.conf.json", "utf-8"));
  const version = tauriConf.version;
  console.log(`\n📦 Publishing v${version}\n`);

  // 2. Read signature
  const sigPath = join(BUNDLE_PATH, "macos/caipi.app.tar.gz.sig");
  const signature = (await readFile(sigPath, "utf-8")).trim();

  // 3. Generate latest.json
  const latestJson = {
    version,
    notes: "",
    pub_date: new Date().toISOString(),
    platforms: {
      "darwin-aarch64": {
        signature,
        url: `https://github.com/pietz/caipi.ai/releases/download/v${version}/caipi.app.tar.gz`,
      },
    },
  };

  const latestPath = join(BUNDLE_PATH, "latest.json");
  await writeFile(latestPath, JSON.stringify(latestJson, null, 2));
  console.log(`✓ Generated latest.json`);

  // 4. Get SHA256 of DMG
  const dmgPath = join(BUNDLE_PATH, "dmg/caipi_aarch64.dmg");
  const sha256Output = execSync(`shasum -a 256 "${dmgPath}"`, { encoding: "utf-8" });
  const sha256 = sha256Output.split(" ")[0];
  console.log(`✓ SHA256: ${sha256.slice(0, 16)}...`);

  // 5. Update cask formula (if it exists)
  const caskPath = join(CAIPI_AI_PATH, "Casks/caipi.rb");
  try {
    let caskContent = await readFile(caskPath, "utf-8");
    caskContent = caskContent.replace(/version "[^"]+"/, `version "${version}"`);
    caskContent = caskContent.replace(/sha256 "[^"]+"/, `sha256 "${sha256}"`);
    await writeFile(caskPath, caskContent);
    console.log(`✓ Updated cask formula`);
  } catch (err) {
    console.log(`⏭ Skipped cask formula (not found)`);
  }

  // 6. Create GitHub release with files
  const dmgFile = join(BUNDLE_PATH, "dmg/caipi_aarch64.dmg");
  const tgzFile = join(BUNDLE_PATH, "macos/caipi.app.tar.gz");
  const sigFile = join(BUNDLE_PATH, "macos/caipi.app.tar.gz.sig");

  console.log(`\n🚀 Creating GitHub release v${version}...\n`);

  try {
    // Delete existing release if it exists (for re-runs)
    execSync(`gh release delete v${version} --repo pietz/caipi.ai --yes 2>/dev/null || true`, {
      encoding: "utf-8",
    });

    // Create release and upload files
    execSync(
      `gh release create v${version} \
        "${dmgFile}" \
        "${tgzFile}" \
        "${sigFile}" \
        "${latestPath}" \
        --repo pietz/caipi.ai \
        --title "v${version}" \
        --notes "Release v${version}"`,
      { stdio: "inherit" }
    );
    console.log(`\n✓ GitHub release created`);
  } catch (err) {
    console.error("Failed to create release:", err.message);
    process.exit(1);
  }

  // 7. Commit cask formula update (if it exists)
  try {
    await readFile(caskPath, "utf-8"); // Check if file exists
    console.log(`\n📝 Committing cask formula update...\n`);
    execSync(`git -C "${CAIPI_AI_PATH}" add Casks/caipi.rb`);
    execSync(`git -C "${CAIPI_AI_PATH}" commit -m "Update cask to v${version}"`);
    execSync(`git -C "${CAIPI_AI_PATH}" push`);
    console.log(`✓ Cask formula committed and pushed`);
  } catch (err) {
    // Cask doesn't exist or no changes - skip silently
  }

  console.log(`
✅ Release v${version} published!

Download: https://github.com/pietz/caipi.ai/releases/latest/download/caipi_aarch64.dmg
`);
}

publish().catch((err) => {
  console.error("Release failed:", err);
  process.exit(1);
});
