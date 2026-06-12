"""
Generate icon.ico (Windows) and icon.png (Mac/Linux) for Imaginify.
Run automatically by the build scripts; you can also run it standalone.
"""

from PIL import Image, ImageDraw


def make_icon(size: int = 512) -> Image.Image:
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Rounded blue square background (matches the app's accent colour)
    radius = size // 5
    draw.rounded_rectangle(
        [(0, 0), (size, size)],
        radius=radius,
        fill=(59, 130, 246),  # #3b82f6
    )

    # Inner white card (like a polaroid)
    margin = size // 7
    inner_box = [(margin, margin), (size - margin, size - margin)]
    draw.rounded_rectangle(inner_box, radius=radius // 2, fill=(255, 255, 255))

    # Sun
    sun_r = size // 12
    sun_x = size - margin - size // 5
    sun_y = margin + size // 5
    draw.ellipse(
        [(sun_x - sun_r, sun_y - sun_r), (sun_x + sun_r, sun_y + sun_r)],
        fill=(251, 191, 36),  # warm yellow
    )

    # Two overlapping mountains
    mountain_far = (96, 165, 250)
    mountain_near = (37, 99, 235)
    base_y = size - margin - size // 12
    draw.polygon(
        [
            (margin + size // 12, base_y),
            (size // 2 + size // 16, margin + size // 4),
            (size - margin - size // 12, base_y),
        ],
        fill=mountain_far,
    )
    draw.polygon(
        [
            (margin + size // 16, base_y),
            (size // 3, margin + size // 3 + size // 12),
            (size // 2 + size // 6, base_y),
        ],
        fill=mountain_near,
    )

    # Ground line under the mountains so the inner card stays clean
    draw.rectangle(
        [(margin, base_y), (size - margin, size - margin)],
        fill=(255, 255, 255),
    )

    return img


def main():
    img = make_icon(1024)

    # Multi-resolution .ico for Windows
    ico_sizes = [(s, s) for s in (16, 32, 48, 64, 128, 256)]
    img.save("icon.ico", format="ICO", sizes=ico_sizes)
    print("Wrote icon.ico")

    # 1024px PNG for Mac/Linux (build script can convert to .icns)
    img.save("icon.png")
    print("Wrote icon.png")


if __name__ == "__main__":
    main()
