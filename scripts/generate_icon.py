# /// script
# dependencies = ["pillow", "svglib", "reportlab"]
# requires-python = ">=3.12"
# ///
"""
Generate macOS app icon for Caipi following Apple's HIG specifications.

macOS Icon Specs:
- Canvas: 1024 × 1024 px
- Icon shape: 824 × 824 px centered
- Corner radius: 185.4 px (for 824px shape)
- Gutter: 100 px on all sides
- Drop shadow: 28px blur radius, 12px Y-offset, 50% black opacity
"""

import io
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter
from reportlab.graphics import renderPM
from svglib.svglib import svg2rlg

# Paths
SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent
SVG_PATH = Path.home() / "Desktop/caipi.svg"
ICONS_DIR = PROJECT_ROOT / "src-tauri/icons"

# macOS icon specifications
CANVAS_SIZE = 1024
ICON_SIZE = 824
CORNER_RADIUS = 185
GUTTER = (CANVAS_SIZE - ICON_SIZE) // 2  # 100px

# Shadow specs
SHADOW_BLUR = 28
SHADOW_Y_OFFSET = 12
SHADOW_OPACITY = 0.5

# Background color (white to match the SVG interior)
BG_COLOR = (255, 255, 255)  # Pure white


def create_rounded_rect_mask(size: int, radius: int) -> Image.Image:
    """Create a mask for a rounded rectangle."""
    mask = Image.new("L", (size, size), 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle(
        [(0, 0), (size - 1, size - 1)],
        radius=radius,
        fill=255,
    )
    return mask


def create_shadow(mask: Image.Image, blur: int, opacity: float) -> Image.Image:
    """Create a drop shadow from a mask."""
    shadow = Image.new("RGBA", mask.size, (0, 0, 0, 0))
    shadow_alpha = mask.point(lambda x: int(x * opacity))
    shadow.putalpha(shadow_alpha)
    shadow = shadow.filter(ImageFilter.GaussianBlur(blur))
    return shadow


def load_svg_as_png(svg_path: Path, size: int) -> Image.Image:
    """Load an SVG file and convert it to PNG at the specified size with transparency."""
    drawing = svg2rlg(str(svg_path))
    if drawing is None:
        raise ValueError(f"Could not load SVG from {svg_path}")

    # Calculate scale factor
    scale = size / max(drawing.width, drawing.height)
    drawing.width = drawing.width * scale
    drawing.height = drawing.height * scale
    drawing.scale(scale, scale)

    # Render to PNG bytes (default white background)
    png_bytes = io.BytesIO()
    renderPM.drawToFile(drawing, png_bytes, fmt="PNG")
    png_bytes.seek(0)

    return Image.open(png_bytes).convert("RGBA")


def generate_macos_icon() -> Image.Image:
    """Generate the macOS app icon with proper specs."""
    # Create canvas with transparency
    canvas = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))

    # Create the rounded rectangle mask
    icon_mask = create_rounded_rect_mask(ICON_SIZE, CORNER_RADIUS)

    # Create the shadow
    shadow = create_shadow(icon_mask, SHADOW_BLUR, SHADOW_OPACITY)

    # Position shadow (offset down by SHADOW_Y_OFFSET)
    shadow_pos = (GUTTER, GUTTER + SHADOW_Y_OFFSET)
    canvas.paste(shadow, shadow_pos, shadow)

    # Create the icon background with rounded corners
    icon_bg = Image.new("RGBA", (ICON_SIZE, ICON_SIZE), (*BG_COLOR, 255))
    icon_bg.putalpha(icon_mask)

    # Paste background onto canvas
    canvas.paste(icon_bg, (GUTTER, GUTTER), icon_bg)

    # Load the SVG logo (without background circle)
    logo_padding = 60  # Padding inside the rounded rect
    logo_target_size = ICON_SIZE - (logo_padding * 2)

    print(f"Loading SVG from: {SVG_PATH}")
    logo = load_svg_as_png(SVG_PATH, logo_target_size)

    # Center the logo on the icon
    logo_x = GUTTER + (ICON_SIZE - logo.width) // 2
    logo_y = GUTTER + (ICON_SIZE - logo.height) // 2

    # Composite the logo onto the canvas (use alpha compositing)
    canvas = Image.alpha_composite(canvas, Image.new("RGBA", canvas.size, (0, 0, 0, 0)))

    # Create a temporary canvas for the logo at the right position
    logo_canvas = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    logo_canvas.paste(logo, (logo_x, logo_y), logo)

    # Composite
    canvas = Image.alpha_composite(canvas, logo_canvas)

    return canvas


def generate_all_sizes(master: Image.Image) -> None:
    """Generate all required icon sizes for Tauri."""
    sizes = {
        "icon.png": 512,  # Base icon
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
    }

    ICONS_DIR.mkdir(parents=True, exist_ok=True)

    for filename, size in sizes.items():
        icon = master.resize((size, size), Image.Resampling.LANCZOS)
        icon.save(ICONS_DIR / filename, "PNG")
        print(f"Created {filename} ({size}x{size})")

    # Save the full 1024x1024 master for creating icns/ico
    master.save(ICONS_DIR / "icon-1024.png", "PNG")
    print("Created icon-1024.png (1024x1024)")


def main() -> None:
    print("Generating Caipi macOS app icon...")
    print(f"SVG source: {SVG_PATH}")
    print(f"Output directory: {ICONS_DIR}")
    print()

    # Generate the master icon
    master = generate_macos_icon()

    # Generate all sizes
    generate_all_sizes(master)

    print()
    print("Done!")


if __name__ == "__main__":
    main()
