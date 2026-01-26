# /// script
# dependencies = ["pillow", "numpy"]
# requires-python = ">=3.12"
# ///
"""
Create macOS Big Sur style app icon from a square source image.

Specifications:
- Canvas: 1024x1024
- Icon shape: 824x824 centered (100px padding)
- Corner radius: ~185px with continuous corners (squircle approximation)
- Shadow: 28px blur, 12px Y-offset, black 50%
"""

import numpy as np
from PIL import Image, ImageDraw, ImageFilter
from pathlib import Path


def create_squircle_mask(size: int, radius: float, smoothness: float = 0.6) -> Image.Image:
    """
    Create a squircle (superellipse) mask for continuous corners.

    Apple uses n≈4-5 for their superellipse. We approximate with n=4.
    Formula: |x|^n + |y|^n = 1
    """
    mask = Image.new('L', (size, size), 0)

    # Use superellipse formula for smoother corners
    # Higher n = more square, lower n = more circular
    n = 4.0  # Apple-like continuous corners

    center = size / 2
    # The "radius" of the superellipse (distance from center to edge)
    a = size / 2

    # Calculate corner radius ratio
    corner_ratio = radius / size

    pixels = np.zeros((size, size), dtype=np.uint8)

    for y in range(size):
        for x in range(size):
            # Normalize coordinates to -1 to 1
            nx = (x - center) / a
            ny = (y - center) / a

            # Superellipse equation: |x|^n + |y|^n <= 1
            # Adjust n based on position for continuous corner effect
            value = abs(nx) ** n + abs(ny) ** n

            if value <= 1.0:
                pixels[y, x] = 255
            else:
                # Anti-aliasing at edges
                dist = value - 1.0
                if dist < 0.02:
                    pixels[y, x] = int(255 * (1 - dist / 0.02))

    mask = Image.fromarray(pixels, mode='L')
    return mask


def create_rounded_rect_mask(size: int, radius: int) -> Image.Image:
    """Create a simple rounded rectangle mask with anti-aliasing."""
    # Create at 2x for better anti-aliasing, then downscale
    scale = 2
    large_size = size * scale
    large_radius = radius * scale

    mask = Image.new('L', (large_size, large_size), 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle(
        [(0, 0), (large_size - 1, large_size - 1)],
        radius=large_radius,
        fill=255
    )

    # Downscale with antialiasing
    mask = mask.resize((size, size), Image.LANCZOS)
    return mask


def create_continuous_corner_mask(size: int, radius: int) -> Image.Image:
    """
    Create a mask with Apple-style continuous corners.

    This blends between a rounded rect and a squircle for smooth corners.
    """
    # Create both masks
    rounded = create_rounded_rect_mask(size, radius)

    # For continuous corners, we blend the rounded rect with a slightly
    # larger radius version, creating a smoother transition
    larger_radius = int(radius * 1.15)
    smoother = create_rounded_rect_mask(size, larger_radius)

    # Blend them - the result has smoother corner transitions
    rounded_arr = np.array(rounded, dtype=np.float32)
    smoother_arr = np.array(smoother, dtype=np.float32)

    # Use the minimum (intersection) with slight blend
    blended = np.minimum(rounded_arr, smoother_arr * 0.95 + rounded_arr * 0.05)

    return Image.fromarray(blended.astype(np.uint8), mode='L')


def add_shadow(image: Image.Image, blur: int = 28, offset_y: int = 12, opacity: float = 0.5) -> Image.Image:
    """Add a drop shadow to an RGBA image."""
    # Create shadow from alpha channel
    shadow = Image.new('RGBA', image.size, (0, 0, 0, 0))

    # Extract alpha and use as shadow base
    alpha = image.split()[3]

    # Create shadow layer
    shadow_layer = Image.new('RGBA', image.size, (0, 0, 0, int(255 * opacity)))
    shadow_layer.putalpha(alpha)

    # Offset the shadow
    shadow_offset = Image.new('RGBA', image.size, (0, 0, 0, 0))
    shadow_offset.paste(shadow_layer, (0, offset_y))

    # Blur the shadow
    shadow_alpha = shadow_offset.split()[3]
    shadow_alpha = shadow_alpha.filter(ImageFilter.GaussianBlur(blur))
    shadow_offset.putalpha(shadow_alpha)

    # Composite: shadow behind image
    result = Image.new('RGBA', image.size, (0, 0, 0, 0))
    result = Image.alpha_composite(result, shadow_offset)
    result = Image.alpha_composite(result, image)

    return result


def create_app_icon(
    source_path: str | Path,
    output_path: str | Path,
    canvas_size: int = 1024,
    icon_size: int = 824,
    corner_radius: int = 185,
    add_drop_shadow: bool = True,
    background_color: tuple = (255, 255, 255, 255),
) -> None:
    """
    Create a macOS Big Sur style app icon.

    Args:
        source_path: Path to source square image
        output_path: Path for output icon
        canvas_size: Final canvas size (default 1024)
        icon_size: Size of the icon shape (default 824)
        corner_radius: Corner radius for the shape (default 185)
        add_drop_shadow: Whether to add shadow (default True)
        background_color: RGBA background for the icon shape
    """
    source_path = Path(source_path)
    output_path = Path(output_path)

    # Load source image
    source = Image.open(source_path).convert('RGBA')
    print(f"Source image: {source.size}")

    # Resize source to fit icon_size (no extra padding - source fills the shape)
    source_resized = source.resize((icon_size, icon_size), Image.LANCZOS)

    # Create the icon shape with background
    icon_shape = Image.new('RGBA', (icon_size, icon_size), background_color)

    # Paste source directly (no offset)
    icon_shape.paste(source_resized, (0, 0), source_resized)

    # Create and apply the continuous corner mask
    mask = create_rounded_rect_mask(icon_size, corner_radius)

    # Apply mask to icon
    icon_with_alpha = Image.new('RGBA', (icon_size, icon_size), (0, 0, 0, 0))
    icon_with_alpha.paste(icon_shape, (0, 0), mask)

    # Create final canvas
    canvas = Image.new('RGBA', (canvas_size, canvas_size), (0, 0, 0, 0))

    # Center icon on canvas
    offset = (canvas_size - icon_size) // 2
    canvas.paste(icon_with_alpha, (offset, offset), icon_with_alpha)

    # Add shadow if requested (subtle shadow - half strength)
    if add_drop_shadow:
        canvas = add_shadow(canvas, blur=14, offset_y=6, opacity=0.15)

    # Save
    canvas.save(output_path, 'PNG')
    print(f"Saved icon to: {output_path}")
    print(f"Final size: {canvas.size}")


def create_inapp_logo(
    source_path: str | Path,
    output_path: str | Path,
    size: int = 512,
    corner_radius_ratio: float = 0.225,
    background_color: tuple = (255, 255, 255, 255),
) -> None:
    """
    Create an in-app logo (no shadow, no extra padding).
    """
    source_path = Path(source_path)
    output_path = Path(output_path)

    source = Image.open(source_path).convert('RGBA')
    source_resized = source.resize((size, size), Image.LANCZOS)

    # Create shape with background
    icon_shape = Image.new('RGBA', (size, size), background_color)
    icon_shape.paste(source_resized, (0, 0), source_resized)

    # Apply rounded corners
    radius = int(size * corner_radius_ratio)
    mask = create_rounded_rect_mask(size, radius)

    result = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    result.paste(icon_shape, (0, 0), mask)

    result.save(output_path, 'PNG')
    print(f"Saved in-app logo to: {output_path}")


def main():
    import sys

    # Get source from command line or use default
    if len(sys.argv) > 1:
        source = Path(sys.argv[1])
        # Generate output name from source
        output_name = f"icon-{source.stem[-4:]}.png"
    else:
        source = Path.home() / "Downloads" / "Gemini_Generated_Image_qlos6fqlos6fqlos.png"
        output_name = "icon-new.png"

    output_dir = Path("/Users/pietz/Private/caipi/src-tauri/icons")

    # Create main 1024x1024 icon
    create_app_icon(
        source_path=source,
        output_path=output_dir / output_name,
        canvas_size=1024,
        icon_size=824,
        corner_radius=185,
        add_drop_shadow=True,
        background_color=(255, 255, 255, 255),  # White background
    )

    # Also create in-app logo
    create_inapp_logo(
        source_path=source,
        output_path=Path("/Users/pietz/Private/caipi/src/lib/assets/caipi-logo.png"),
        size=512,
    )

    print("\nIcons created!")


if __name__ == "__main__":
    main()
