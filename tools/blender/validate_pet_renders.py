"""Validate the generated PNG contract and build a compact review sheet."""

from __future__ import annotations

from pathlib import Path
import argparse
import hashlib
import json

from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parents[2]
OUTPUT_ROOT = ROOT / "public" / "pets" / "blender-rendered"
MANIFEST_PATH = OUTPUT_ROOT / "manifest.json"
ACTIONS_PATH = ROOT / "public" / "pets" / "pai" / "actions.json"
GENERATOR_PATH = ROOT / "tools" / "blender" / "generate_pet_models.py"
DEFAULT_PREVIEW_PATH = ROOT / "assets" / "blender" / "pet-render-preview.png"


def expected_paths(manifest):
    frame_count = manifest["render"]["frameCount"]
    for species in manifest["species"]:
        for stage in manifest["stages"]:
            for route in manifest["routes"]:
                for family in manifest["motionFamilies"]:
                    for frame in range(1, frame_count + 1):
                        yield OUTPUT_ROOT / species / stage / route / family / f"{frame:02d}.png"
            for frame in range(1, frame_count + 1):
                yield OUTPUT_ROOT / species / stage / f"{frame:02d}.png"


def checkerboard(size, cell=12):
    image = Image.new("RGBA", size, (238, 239, 245, 255))
    draw = ImageDraw.Draw(image)
    for y in range(0, size[1], cell):
        for x in range(0, size[0], cell):
            if (x // cell + y // cell) % 2:
                draw.rectangle((x, y, x + cell - 1, y + cell - 1), fill=(215, 218, 228, 255))
    return image


def paste_sprite(sheet, sprite_path, box, label):
    x, y, width, height = box
    cell = checkerboard((width, height))
    with Image.open(sprite_path) as source:
        sprite = source.convert("RGBA")
        sprite.thumbnail((width - 18, height - 30), Image.Resampling.LANCZOS)
        target_x = (width - sprite.width) // 2
        target_y = max(2, height - 24 - sprite.height)
        cell.alpha_composite(sprite, (target_x, target_y))
    draw = ImageDraw.Draw(cell)
    draw.rectangle((0, height - 22, width, height), fill=(24, 27, 38, 215))
    draw.text((6, height - 18), label, fill=(255, 255, 255, 255))
    sheet.alpha_composite(cell, (x, y))


def build_preview(manifest, output_path):
    cell_width = 164
    cell_height = 184
    columns = len(manifest["motionFamilies"])
    rows = len(manifest["routes"]) + 1
    sheet = Image.new("RGBA", (columns * cell_width, rows * cell_height), (249, 249, 252, 255))
    review_species = "star-cat"
    for row, route in enumerate(manifest["routes"]):
        for column, family in enumerate(manifest["motionFamilies"]):
            source = OUTPUT_ROOT / review_species / "evolved" / route / family / "03.png"
            paste_sprite(
                sheet,
                source,
                (column * cell_width, row * cell_height, cell_width, cell_height),
                f"{route} / {family}",
            )
    species_row = len(manifest["routes"])
    for column, species in enumerate(manifest["species"]):
        source = OUTPUT_ROOT / species / "growing" / "companion" / "idle" / "02.png"
        paste_sprite(
            sheet,
            source,
            (column * cell_width, species_row * cell_height, cell_width, cell_height),
            f"{species} / growing",
        )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    sheet.convert("RGB").save(output_path, format="PNG", optimize=True)


def validate(preview_path):
    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    errors = []
    expected = list(expected_paths(manifest))
    actual = sorted(OUTPUT_ROOT.rglob("*.png"))
    expected_set = {path.resolve() for path in expected}
    actual_set = {path.resolve() for path in actual}
    missing = sorted(expected_set - actual_set)
    unexpected = sorted(actual_set - expected_set)
    if missing:
        errors.append(f"missing {len(missing)} PNG files; first={missing[0]}")
    if unexpected:
        errors.append(f"unexpected {len(unexpected)} PNG files; first={unexpected[0]}")
    if len(actual) != manifest["render"]["totalAssetCount"]:
        errors.append(
            f"manifest expects {manifest['render']['totalAssetCount']} PNGs, found {len(actual)}"
        )

    frame_digests = {}
    transparent_images = 0
    for path in expected:
        if not path.exists():
            continue
        with Image.open(path) as image:
            if image.format != "PNG":
                errors.append(f"not PNG: {path}")
                continue
            if image.size != (manifest["render"]["width"], manifest["render"]["height"]):
                errors.append(f"wrong dimensions {image.size}: {path}")
            rgba = image.convert("RGBA")
            alpha = rgba.getchannel("A")
            minimum, maximum = alpha.getextrema()
            if minimum != 0 or maximum == 0:
                errors.append(f"invalid alpha extrema {(minimum, maximum)}: {path}")
            else:
                transparent_images += 1
            corners = (
                alpha.getpixel((0, 0)),
                alpha.getpixel((alpha.width - 1, 0)),
                alpha.getpixel((0, alpha.height - 1)),
                alpha.getpixel((alpha.width - 1, alpha.height - 1)),
            )
            if any(corners):
                errors.append(f"non-transparent canvas corner {corners}: {path}")
        relative_parts = path.relative_to(OUTPUT_ROOT).parts
        if len(relative_parts) == 5 and relative_parts[-1].lower().endswith(".png"):
            key = path.parent
            frame_digests.setdefault(key, set()).add(hashlib.sha256(path.read_bytes()).hexdigest())

    static_sets = [path for path, digests in frame_digests.items() if len(digests) < 2]
    if static_sets:
        errors.append(f"{len(static_sets)} motion sets have four identical frames; first={static_sets[0]}")

    actions_manifest = json.loads(ACTIONS_PATH.read_text(encoding="utf-8"))
    expected_actions = set(actions_manifest["actions"])
    mapped_actions = set(manifest["actions"])
    if expected_actions != mapped_actions:
        errors.append(
            f"action map mismatch missing={sorted(expected_actions - mapped_actions)} "
            f"extra={sorted(mapped_actions - expected_actions)}"
        )
    invalid_families = set(manifest["actions"].values()) - set(manifest["motionFamilies"])
    if invalid_families:
        errors.append(f"action map contains invalid families: {sorted(invalid_families)}")

    script_hash = hashlib.sha256(GENERATOR_PATH.read_bytes()).hexdigest()
    if script_hash != manifest["generator"]["scriptSha256"]:
        errors.append(
            f"generator hash mismatch manifest={manifest['generator']['scriptSha256']} actual={script_hash}"
        )

    if errors:
        raise SystemExit("\n".join(f"ERROR: {error}" for error in errors))

    build_preview(manifest, preview_path)
    total_bytes = sum(path.stat().st_size for path in actual)
    summary = {
        "pngCount": len(actual),
        "dimensions": [manifest["render"]["width"], manifest["render"]["height"]],
        "rgbaTransparentCount": transparent_images,
        "actionCount": len(mapped_actions),
        "motionFamilyCount": len(manifest["motionFamilies"]),
        "routeCount": len(manifest["routes"]),
        "speciesCount": len(manifest["species"]),
        "stageCount": len(manifest["stages"]),
        "totalBytes": total_bytes,
        "totalMiB": round(total_bytes / 1024 / 1024, 2),
        "preview": str(preview_path),
    }
    print(json.dumps(summary, ensure_ascii=False, indent=2))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--preview", type=Path, default=DEFAULT_PREVIEW_PATH)
    args = parser.parse_args()
    validate(args.preview.resolve())


if __name__ == "__main__":
    main()
