# /// script
# dependencies = ["pillow"]
# requires-python = ">=3.12"
# ///
"""
Generate .icns file for macOS using iconutil.

Required sizes for .icns:
- 16x16, 16x16@2x (32)
- 32x32, 32x32@2x (64)
- 128x128, 128x128@2x (256)
- 256x256, 256x256@2x (512)
- 512x512, 512x512@2x (1024)
"""

import subprocess
import tempfile
from pathlib import Path
from PIL import Image

SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent
ICONS_DIR = PROJECT_ROOT / "src-tauri/icons"
MASTER_ICON = ICONS_DIR / "icon-1024.png"


def generate_icns() -> None:
    """Generate .icns file using iconutil."""
    if not MASTER_ICON.exists():
        print(f"Error: Master icon not found at {MASTER_ICON}")
        print("Run generate_icon.py first.")
        return

    master = Image.open(MASTER_ICON)

    # Create temporary iconset directory
    with tempfile.TemporaryDirectory() as tmpdir:
        iconset_path = Path(tmpdir) / "AppIcon.iconset"
        iconset_path.mkdir()

        # Required sizes for macOS iconset
        sizes = [
            ("icon_16x16.png", 16),
            ("icon_16x16@2x.png", 32),
            ("icon_32x32.png", 32),
            ("icon_32x32@2x.png", 64),
            ("icon_128x128.png", 128),
            ("icon_128x128@2x.png", 256),
            ("icon_256x256.png", 256),
            ("icon_256x256@2x.png", 512),
            ("icon_512x512.png", 512),
            ("icon_512x512@2x.png", 1024),
        ]

        for filename, size in sizes:
            icon = master.resize((size, size), Image.Resampling.LANCZOS)
            icon.save(iconset_path / filename, "PNG")
            print(f"Created {filename} ({size}x{size})")

        # Run iconutil to create .icns
        output_path = ICONS_DIR / "icon.icns"
        result = subprocess.run(
            ["iconutil", "-c", "icns", str(iconset_path), "-o", str(output_path)],
            capture_output=True,
            text=True,
        )

        if result.returncode == 0:
            print(f"\nSuccessfully created: {output_path}")
        else:
            print(f"\nError creating .icns: {result.stderr}")


def generate_ico() -> None:
    """Generate .ico file for Windows."""
    if not MASTER_ICON.exists():
        print(f"Error: Master icon not found at {MASTER_ICON}")
        return

    master = Image.open(MASTER_ICON)

    # Windows ICO sizes
    ico_sizes = [16, 24, 32, 48, 64, 128, 256]

    # Create images for each size
    images = []
    for size in ico_sizes:
        img = master.resize((size, size), Image.Resampling.LANCZOS)
        images.append(img)

    # Save as ICO (Pillow can create multi-size ICO files)
    output_path = ICONS_DIR / "icon.ico"
    images[0].save(
        output_path,
        format="ICO",
        sizes=[(s, s) for s in ico_sizes],
        append_images=images[1:],
    )
    print(f"Successfully created: {output_path}")


def main() -> None:
    print("Generating .icns and .ico files...")
    print()

    print("=== macOS .icns ===")
    generate_icns()

    print()
    print("=== Windows .ico ===")
    generate_ico()


if __name__ == "__main__":
    main()
