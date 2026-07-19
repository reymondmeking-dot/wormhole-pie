"""Fail-fast structural inspection for wormhole-pets.blend."""

from __future__ import annotations

import json

import bpy


SPECIES = ("corgi", "star-cat", "moon-rabbit", "river-otter", "bear-cub")
STAGES = ("seedling", "growing", "evolved")
REQUIRED_BONES = {
    "root",
    "spine",
    "head",
    "arm.L",
    "arm.R",
    "leg.L",
    "leg.R",
    "ear.L",
    "ear.R",
    "tail",
    "accessory",
    "prop",
}


def main():
    expected_models = len(SPECIES) * len(STAGES)
    armatures = sorted((obj for obj in bpy.data.objects if obj.type == "ARMATURE"), key=lambda obj: obj.name)
    errors = []
    if len(armatures) != expected_models:
        errors.append(f"expected {expected_models} armatures, found {len(armatures)}")
    action_names = []
    min_bones = None
    for rig in armatures:
        bones = set(rig.data.bones.keys())
        min_bones = len(bones) if min_bones is None else min(min_bones, len(bones))
        missing = REQUIRED_BONES - bones
        if missing:
            errors.append(f"{rig.name} missing bones {sorted(missing)}")
        action = rig.animation_data.action if rig.animation_data else None
        if action is None:
            errors.append(f"{rig.name} has no animation action")
        else:
            action_names.append(action.name)
            if action.frame_range[1] < 64:
                errors.append(f"{action.name} frame range ends before alert clip: {tuple(action.frame_range)}")
    expected_collections = {f"PET_{species}_{stage}" for species in SPECIES for stage in STAGES}
    missing_collections = expected_collections - set(bpy.data.collections.keys())
    if missing_collections:
        errors.append(f"missing model collections {sorted(missing_collections)}")
    if errors:
        raise SystemExit("\n".join(f"ERROR: {error}" for error in errors))
    summary = {
        "blend": bpy.data.filepath,
        "blenderVersion": bpy.app.version_string,
        "modelCollections": len(expected_collections),
        "armatures": len(armatures),
        "minimumBonesPerArmature": min_bones,
        "animationActions": len(action_names),
        "timeline": [bpy.context.scene.frame_start, bpy.context.scene.frame_end],
        "markers": {marker.name: marker.frame for marker in bpy.context.scene.timeline_markers},
        "objects": len(bpy.data.objects),
    }
    print("WORMHOLE_BLEND_VALIDATION=" + json.dumps(summary, ensure_ascii=False, sort_keys=True))


if __name__ == "__main__":
    main()
