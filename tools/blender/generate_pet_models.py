"""Build the Wormhole Pie pets in Blender and bake them to transparent PNGs.

The application deliberately ships the PNG bake rather than a real-time 3D
runtime.  The saved .blend keeps the source armatures and a keyed preview
timeline so the motions remain editable and auditable in Blender.
"""

from __future__ import annotations

from datetime import datetime, timezone
from pathlib import Path
import hashlib
import json
import math
import shutil

import bpy
from mathutils import Vector


ROOT = Path(__file__).resolve().parents[2]
OUTPUT_ROOT = ROOT / "public" / "pets" / "blender-rendered"
PUBLIC_MANIFEST_PATH = OUTPUT_ROOT / "manifest.json"
RUNTIME_MANIFEST_PATH = ROOT / "src" / "pet" / "blenderMotionMap.json"
BLEND_PATH = ROOT / "assets" / "blender" / "wormhole-pets.blend"
ACTIONS_PATH = ROOT / "public" / "pets" / "pai" / "actions.json"

PIPELINE_VERSION = "2.0.0"
RENDER_SIZE = 256
FRAME_COUNT = 4
SPECIES = ("corgi", "star-cat", "moon-rabbit", "river-otter", "bear-cub")
SPECIES_ALIASES = {"cloud-fox": "corgi"}
STAGES = ("seedling", "growing", "evolved")
ROUTES = ("companion", "creator", "guardian")
MOTION_FAMILIES = ("idle", "move", "joy", "rest", "focus", "file", "alert")
FRAME_VALUES = range(1, FRAME_COUNT + 1)

PALETTES = {
    "corgi": ((0.88, 0.36, 0.09, 1), (1.0, 0.83, 0.52, 1)),
    "star-cat": ((0.34, 0.27, 0.68, 1), (0.79, 0.75, 1.0, 1)),
    "moon-rabbit": ((0.96, 0.52, 0.64, 1), (1.0, 0.88, 0.92, 1)),
    "river-otter": ((0.38, 0.23, 0.14, 1), (0.79, 0.60, 0.40, 1)),
    "bear-cub": ((0.67, 0.40, 0.19, 1), (0.94, 0.72, 0.45, 1)),
}

ROUTE_DEFINITIONS = {
    "companion": {"accent": (0.94, 0.27, 0.48, 1), "accessory": "heart"},
    "creator": {"accent": (1.0, 0.63, 0.10, 1), "accessory": "orbit"},
    "guardian": {"accent": (0.20, 0.48, 0.86, 1), "accessory": "shield"},
}

ACTION_TO_FAMILY = {
    "idle_breathe": "idle",
    "blink": "idle",
    "ear_twitch": "idle",
    "tail_flick": "idle",
    "yawn": "idle",
    "idle_lookaround": "idle",
    "cursor_notice": "idle",
    "hover_interest": "idle",
    "walk": "move",
    "drag": "move",
    "run": "move",
    "hop": "move",
    "edge_balance": "move",
    "release_bounce": "move",
    "click_feedback": "joy",
    "task_success": "joy",
    "affection": "joy",
    "double_click_excited": "joy",
    "pet_loop": "joy",
    "tickle_laugh": "joy",
    "high_five": "joy",
    "emotion_happy": "joy",
    "emotion_surprised": "joy",
    "dance": "joy",
    "snack_eat": "joy",
    "sleep": "rest",
    "rest_reminder": "rest",
    "hide_peek": "rest",
    "stretch": "rest",
    "groom": "rest",
    "listen": "focus",
    "think": "focus",
    "typing_watch": "focus",
    "voice_wake": "focus",
    "speak_loop": "focus",
    "file_eat": "file",
    "file_notice": "file",
    "file_reject": "file",
    "trash_success": "file",
    "task_failure": "alert",
    "permission_wait": "alert",
    "emotion_sad": "alert",
    "emotion_angry": "alert",
    "trash_fail_cough": "alert",
}

REQUIRED_BONES = (
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
)


def reset_scene():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for data in (
        bpy.data.collections,
        bpy.data.armatures,
        bpy.data.meshes,
        bpy.data.curves,
        bpy.data.materials,
        bpy.data.cameras,
        bpy.data.lights,
        bpy.data.actions,
    ):
        for item in list(data):
            data.remove(item)


def clean_output_root():
    resolved = OUTPUT_ROOT.resolve()
    expected_parent = (ROOT / "public" / "pets").resolve()
    if resolved.parent != expected_parent or resolved.name != "blender-rendered":
        raise RuntimeError(f"Refusing to clean unexpected output path: {resolved}")
    if resolved.exists():
        shutil.rmtree(resolved)
    resolved.mkdir(parents=True, exist_ok=True)


def material(name, color, metallic=0.0, roughness=0.58, emission_strength=0.0):
    value = bpy.data.materials.get(name)
    if value:
        return value
    value = bpy.data.materials.new(name)
    value.diffuse_color = color
    value.use_nodes = True
    shader = value.node_tree.nodes.get("Principled BSDF")
    if shader:
        base_color = shader.inputs.get("Base Color")
        roughness_input = shader.inputs.get("Roughness")
        metallic_input = shader.inputs.get("Metallic")
        emission_color = shader.inputs.get("Emission Color") or shader.inputs.get("Emission")
        emission = shader.inputs.get("Emission Strength")
        if base_color:
            base_color.default_value = color
        if roughness_input:
            roughness_input.default_value = roughness
        if metallic_input:
            metallic_input.default_value = metallic
        if emission_color and emission_strength:
            emission_color.default_value = color
        if emission and emission_strength:
            emission.default_value = emission_strength
    return value


def attach(obj, collection, value):
    for current in list(obj.users_collection):
        current.objects.unlink(obj)
    collection.objects.link(obj)
    if value is not None and hasattr(obj.data, "materials"):
        obj.data.materials.append(value)
    if hasattr(obj.data, "polygons"):
        for polygon in obj.data.polygons:
            polygon.use_smooth = True
    return obj


def sphere(collection, name, location, scale, value, subdivisions=2):
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=subdivisions, radius=1, location=location)
    obj = bpy.context.object
    obj.name = name
    obj.scale = scale
    return attach(obj, collection, value)


def cone(collection, name, location, scale, value, rotation=(0, 0, 0), vertices=5):
    bpy.ops.mesh.primitive_cone_add(
        vertices=vertices,
        radius1=1,
        radius2=0,
        depth=2,
        location=location,
        rotation=rotation,
    )
    obj = bpy.context.object
    obj.name = name
    obj.scale = scale
    return attach(obj, collection, value)


def cylinder(collection, name, location, scale, value, rotation=(0, 0, 0), vertices=12):
    bpy.ops.mesh.primitive_cylinder_add(
        vertices=vertices,
        radius=1,
        depth=2,
        location=location,
        rotation=rotation,
    )
    obj = bpy.context.object
    obj.name = name
    obj.scale = scale
    return attach(obj, collection, value)


def cube(collection, name, location, scale, value, rotation=(0, 0, 0)):
    bpy.ops.mesh.primitive_cube_add(location=location, rotation=rotation)
    obj = bpy.context.object
    obj.name = name
    obj.scale = scale
    return attach(obj, collection, value)


def torus(collection, name, location, major_radius, minor_radius, value, rotation=(0, 0, 0)):
    bpy.ops.mesh.primitive_torus_add(
        major_radius=major_radius,
        minor_radius=minor_radius,
        major_segments=20,
        minor_segments=6,
        location=location,
        rotation=rotation,
    )
    obj = bpy.context.object
    obj.name = name
    return attach(obj, collection, value)


def create_armature(collection, key):
    data = bpy.data.armatures.new(f"ARM_{key}")
    rig = bpy.data.objects.new(f"RIG_{key}", data)
    collection.objects.link(rig)
    rig.show_in_front = True
    rig.display_type = "WIRE"
    bpy.ops.object.select_all(action="DESELECT")
    rig.select_set(True)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.mode_set(mode="EDIT")

    definitions = {
        "root": ((0, 0, 0.05), (0, 0, 0.58), None),
        "spine": ((0, 0, 0.58), (0, 0, 1.66), "root"),
        "head": ((0, 0, 1.66), (0, 0, 2.55), "spine"),
        "arm.L": ((0, 0, 1.43), (-0.72, -0.05, 1.08), "spine"),
        "arm.R": ((0, 0, 1.43), (0.72, -0.05, 1.08), "spine"),
        "leg.L": ((0, 0, 0.66), (-0.39, -0.04, 0.28), "root"),
        "leg.R": ((0, 0, 0.66), (0.39, -0.04, 0.28), "root"),
        "ear.L": ((-0.24, 0, 2.42), (-0.48, 0, 3.02), "head"),
        "ear.R": ((0.24, 0, 2.42), (0.48, 0, 3.02), "head"),
        "tail": ((0, 0.08, 0.92), (0.72, 0.18, 0.62), "spine"),
        "accessory": ((0, 0, 1.48), (0, -0.72, 1.48), "spine"),
        "prop": ((0, 0, 2.42), (0, 0, 3.30), "head"),
    }
    edit_bones = {}
    for name, (head, tail, parent_name) in definitions.items():
        bone = data.edit_bones.new(name)
        bone.head = head
        bone.tail = tail
        if parent_name:
            bone.parent = edit_bones[parent_name]
        edit_bones[name] = bone

    bpy.ops.object.mode_set(mode="POSE")
    for pose_bone in rig.pose.bones:
        pose_bone.rotation_mode = "XYZ"
    bpy.ops.object.mode_set(mode="OBJECT")
    return rig


def parent_to_bone(obj, rig, bone_name):
    if obj.type == "MESH":
        bpy.ops.object.select_all(action="DESELECT")
        obj.select_set(True)
        bpy.context.view_layer.objects.active = obj
        # Separate primitive objects have separate origins. Applying all object
        # transforms places their vertices in the armature's shared coordinate
        # space before the rigid 100% bone weighting is created.
        bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    obj.parent = rig
    obj.parent_type = "OBJECT"
    obj.matrix_parent_inverse = rig.matrix_world.inverted()
    if obj.type == "MESH":
        group = obj.vertex_groups.new(name=bone_name)
        group.add(range(len(obj.data.vertices)), 1.0, "REPLACE")
        armature = obj.modifiers.new(name="Rigid_Armature", type="ARMATURE")
        armature.object = rig
        armature.use_deform_preserve_volume = False
    return obj


def create_route_accessories(collection, rig, key, stage, route_materials):
    route_scale = {"seedling": 0.80, "growing": 0.94, "evolved": 1.08}[stage]
    route_objects = {route: [] for route in ROUTES}

    companion = route_materials["companion"]
    heart_parts = [
        sphere(collection, f"{key}_companion_heart_L", (-0.105, -0.79, 1.55), (0.16, 0.07, 0.16), companion, 1),
        sphere(collection, f"{key}_companion_heart_R", (0.105, -0.79, 1.55), (0.16, 0.07, 0.16), companion, 1),
        cone(
            collection,
            f"{key}_companion_heart_tip",
            (0, -0.79, 1.40),
            (0.22, 0.08, 0.26),
            companion,
            rotation=(math.pi, 0, math.radians(45)),
            vertices=4,
        ),
    ]
    for obj in heart_parts:
        obj.scale *= route_scale
        parent_to_bone(obj, rig, "accessory")
    route_objects["companion"].extend(heart_parts)

    creator = route_materials["creator"]
    orbit_parts = [
        torus(
            collection,
            f"{key}_creator_orbit",
            (0, -0.12, 2.14),
            0.88 * route_scale,
            0.035 * route_scale,
            creator,
            rotation=(math.radians(90), 0, math.radians(-12)),
        ),
        sphere(
            collection,
            f"{key}_creator_orb",
            (0.67 * route_scale, -0.72, 2.42),
            (0.11, 0.07, 0.11),
            creator,
            1,
        ),
    ]
    for obj in orbit_parts:
        parent_to_bone(obj, rig, "head")
    route_objects["creator"].extend(orbit_parts)

    guardian = route_materials["guardian"]
    shield_parts = [
        cylinder(
            collection,
            f"{key}_guardian_shield",
            (0, -0.82, 1.42),
            (0.32 * route_scale, 0.055, 0.39 * route_scale),
            guardian,
            rotation=(math.radians(90), 0, 0),
            vertices=6,
        ),
        sphere(
            collection,
            f"{key}_guardian_core",
            (0, -0.94, 1.43),
            (0.095, 0.045, 0.095),
            route_materials["creator"],
            1,
        ),
    ]
    for obj in shield_parts:
        parent_to_bone(obj, rig, "accessory")
    route_objects["guardian"].extend(shield_parts)
    return route_objects


def create_family_props(collection, rig, key, materials):
    family_objects = {family: [] for family in MOTION_FAMILIES}

    joy_parts = [
        sphere(collection, f"{key}_joy_spark_L", (-0.73, -0.63, 2.63), (0.09, 0.045, 0.09), materials["gold"], 1),
        sphere(collection, f"{key}_joy_spark_R", (0.73, -0.63, 2.70), (0.075, 0.04, 0.075), materials["gold"], 1),
    ]
    for obj in joy_parts:
        parent_to_bone(obj, rig, "head")
    family_objects["joy"].extend(joy_parts)

    focus_parts = [
        sphere(collection, f"{key}_focus_orb", (0, -0.62, 3.18), (0.13, 0.075, 0.13), materials["focus"], 2),
        torus(
            collection,
            f"{key}_focus_ring",
            (0, -0.60, 3.18),
            0.22,
            0.022,
            materials["gold"],
            rotation=(math.radians(90), 0, 0),
        ),
    ]
    for obj in focus_parts:
        parent_to_bone(obj, rig, "prop")
    family_objects["focus"].extend(focus_parts)

    file_parts = [
        cube(collection, f"{key}_file_card", (0, -0.90, 1.10), (0.34, 0.055, 0.42), materials["paper"], rotation=(math.radians(-4), 0, 0)),
        cone(
            collection,
            f"{key}_file_fold",
            (0.24, -0.97, 1.42),
            (0.10, 0.03, 0.10),
            materials["focus"],
            rotation=(math.radians(90), 0, math.radians(45)),
            vertices=3,
        ),
    ]
    for obj in file_parts:
        parent_to_bone(obj, rig, "accessory")
    family_objects["file"].extend(file_parts)

    alert_parts = [
        cylinder(collection, f"{key}_alert_bar", (0, -0.69, 3.08), (0.065, 0.045, 0.22), materials["alert"], vertices=8),
        sphere(collection, f"{key}_alert_dot", (0, -0.69, 2.72), (0.085, 0.05, 0.085), materials["alert"], 1),
    ]
    for obj in alert_parts:
        parent_to_bone(obj, rig, "prop")
    family_objects["alert"].extend(alert_parts)
    return family_objects


def build_pet(species, stage, index):
    key = f"{species}_{stage}"
    collection = bpy.data.collections.new(f"PET_{key}")
    bpy.context.scene.collection.children.link(collection)
    rig = create_armature(collection, key)
    primary_color, accent_color = PALETTES[species]
    primary = material(f"{species}_primary", primary_color)
    accent = material(f"{species}_accent", accent_color)
    dark = material("features_dark", (0.045, 0.035, 0.065, 1), roughness=0.32)
    gold = material("evolution_gold", (1.0, 0.60, 0.10, 1), metallic=0.32, roughness=0.25)
    sprout_green = material("sprout_green", (0.18, 0.66, 0.26, 1))
    paper = material("file_paper", (0.92, 0.96, 1.0, 1), roughness=0.42)
    focus = material("focus_glow", (0.28, 0.72, 1.0, 1), metallic=0.10, roughness=0.22, emission_strength=0.35)
    alert = material("alert_red", (0.95, 0.15, 0.12, 1), roughness=0.28, emission_strength=0.20)
    route_materials = {
        route: material(
            f"route_{route}",
            ROUTE_DEFINITIONS[route]["accent"],
            metallic=0.22 if route != "companion" else 0.05,
            roughness=0.25,
            emission_strength=0.12,
        )
        for route in ROUTES
    }
    materials = {"gold": gold, "paper": paper, "focus": focus, "alert": alert}
    stage_scale = {"seedling": 0.83, "growing": 0.96, "evolved": 1.06}[stage]
    body = sphere(collection, f"{key}_body", (0, 0, 1.12), (0.72, 0.53, 0.92), primary)
    belly = sphere(collection, f"{key}_belly", (0, -0.48, 1.12), (0.48, 0.15, 0.64), accent)
    head = sphere(collection, f"{key}_head", (0, 0, 2.08), (0.78, 0.65, 0.70), primary)
    left_foot = sphere(collection, f"{key}_foot_L", (-0.38, -0.12, 0.36), (0.34, 0.42, 0.26), accent)
    right_foot = sphere(collection, f"{key}_foot_R", (0.38, -0.12, 0.36), (0.34, 0.42, 0.26), accent)
    left_arm = sphere(collection, f"{key}_arm_L", (-0.66, -0.18, 1.20), (0.22, 0.25, 0.53), primary, subdivisions=1)
    right_arm = sphere(collection, f"{key}_arm_R", (0.66, -0.18, 1.20), (0.22, 0.25, 0.53), primary, subdivisions=1)
    muzzle = sphere(collection, f"{key}_muzzle", (0, -0.65, 1.93), (0.47, 0.25, 0.34), accent)
    left_eye = sphere(collection, f"{key}_eye_L", (-0.27, -0.78, 2.22), (0.105, 0.075, 0.14), dark)
    right_eye = sphere(collection, f"{key}_eye_R", (0.27, -0.78, 2.22), (0.105, 0.075, 0.14), dark)
    nose = sphere(collection, f"{key}_nose", (0, -0.91, 1.98), (0.105, 0.07, 0.08), dark)
    mouth = sphere(collection, f"{key}_mouth", (0, -0.84, 1.80), (0.12, 0.045, 0.045), dark, subdivisions=1)

    for obj in (body, belly):
        parent_to_bone(obj, rig, "spine")
    for obj in (head, muzzle, left_eye, right_eye, nose, mouth):
        parent_to_bone(obj, rig, "head")
    parent_to_bone(left_foot, rig, "leg.L")
    parent_to_bone(right_foot, rig, "leg.R")
    parent_to_bone(left_arm, rig, "arm.L")
    parent_to_bone(right_arm, rig, "arm.R")

    if species in {"star-cat", "corgi"}:
        width = 0.47 if species == "corgi" else 0.39
        ear_l = cone(collection, f"{key}_ear_L", (-width, -0.02, 2.63), (0.34, 0.28, 0.49), primary, rotation=(0, 0, math.radians(-8)), vertices=4)
        ear_r = cone(collection, f"{key}_ear_R", (width, -0.02, 2.63), (0.34, 0.28, 0.49), primary, rotation=(0, 0, math.radians(8)), vertices=4)
    elif species == "moon-rabbit":
        ear_l = sphere(collection, f"{key}_ear_L", (-0.27, 0.0, 2.98), (0.24, 0.22, 0.78), primary)
        ear_r = sphere(collection, f"{key}_ear_R", (0.27, 0.0, 2.98), (0.24, 0.22, 0.78), primary)
        inner_l = sphere(collection, f"{key}_inner_ear_L", (-0.27, -0.20, 2.99), (0.10, 0.05, 0.53), accent)
        inner_r = sphere(collection, f"{key}_inner_ear_R", (0.27, -0.20, 2.99), (0.10, 0.05, 0.53), accent)
        parent_to_bone(inner_l, rig, "ear.L")
        parent_to_bone(inner_r, rig, "ear.R")
    else:
        radius = 0.30 if species == "bear-cub" else 0.23
        ear_l = sphere(collection, f"{key}_ear_L", (-0.48, -0.01, 2.52), (radius, 0.22, radius), primary)
        ear_r = sphere(collection, f"{key}_ear_R", (0.48, -0.01, 2.52), (radius, 0.22, radius), primary)
    parent_to_bone(ear_l, rig, "ear.L")
    parent_to_bone(ear_r, rig, "ear.R")

    if species == "star-cat":
        star = sphere(collection, f"{key}_forehead_star", (0, -0.80, 2.48), (0.11, 0.05, 0.11), gold, 1)
        parent_to_bone(star, rig, "head")
        tail = torus(collection, f"{key}_tail", (0.55, 0.08, 0.92), 0.43, 0.105, primary, rotation=(math.radians(82), math.radians(10), math.radians(15)))
    elif species == "river-otter":
        tail = cylinder(collection, f"{key}_tail", (0.57, 0.17, 0.82), (0.19, 0.19, 0.68), primary, rotation=(math.radians(58), 0, math.radians(-22)))
    elif species == "moon-rabbit":
        tail = sphere(collection, f"{key}_tail", (0.60, 0.18, 0.86), (0.25, 0.23, 0.25), accent)
    elif species == "bear-cub":
        tail = sphere(collection, f"{key}_tail", (0.56, 0.15, 0.83), (0.19, 0.17, 0.19), accent)
    else:
        tail = cylinder(collection, f"{key}_tail", (0.55, 0.14, 0.86), (0.18, 0.18, 0.34), primary, rotation=(math.radians(68), 0, math.radians(-25)), vertices=10)
    parent_to_bone(tail, rig, "tail")

    if stage == "seedling":
        stem = cylinder(collection, f"{key}_sprout_stem", (0, 0, 2.90), (0.045, 0.045, 0.22), sprout_green)
        leaf = sphere(collection, f"{key}_sprout_leaf", (0.13, -0.02, 3.07), (0.20, 0.08, 0.11), sprout_green)
        parent_to_bone(stem, rig, "head")
        parent_to_bone(leaf, rig, "head")
    elif stage == "growing":
        collar = torus(collection, f"{key}_growth_collar", (0, 0, 1.58), 0.58, 0.055, gold, rotation=(math.radians(90), 0, 0))
        charm = sphere(collection, f"{key}_growth_charm", (0, -0.62, 1.53), (0.13, 0.06, 0.15), gold)
        parent_to_bone(collar, rig, "spine")
        parent_to_bone(charm, rig, "accessory")
    else:
        halo = torus(collection, f"{key}_evolved_halo", (0, 0.05, 3.05), 0.55, 0.055, gold)
        core = sphere(collection, f"{key}_evolved_core", (0, -0.80, 1.54), (0.16, 0.07, 0.20), gold)
        wing_l = cone(collection, f"{key}_wing_L", (-0.82, 0.12, 1.47), (0.31, 0.13, 0.55), accent, rotation=(0, math.radians(-18), math.radians(-35)), vertices=5)
        wing_r = cone(collection, f"{key}_wing_R", (0.82, 0.12, 1.47), (0.31, 0.13, 0.55), accent, rotation=(0, math.radians(18), math.radians(35)), vertices=5)
        parent_to_bone(halo, rig, "head")
        parent_to_bone(core, rig, "accessory")
        parent_to_bone(wing_l, rig, "spine")
        parent_to_bone(wing_r, rig, "spine")

    route_objects = create_route_accessories(collection, rig, key, stage, route_materials)
    family_objects = create_family_props(collection, rig, key, materials)
    rig.scale = (stage_scale, stage_scale, stage_scale)
    expression_objects = (left_eye, right_eye, mouth)
    expression_geometry = {}
    for obj in expression_objects:
        coordinates = tuple(vertex.co.copy() for vertex in obj.data.vertices)
        center = sum(coordinates, Vector()) / len(coordinates)
        expression_geometry[obj.name] = {"coordinates": coordinates, "center": center}
    rig["species"] = species
    rig["stage"] = stage
    rig["asset_index"] = index
    rig["pipeline_version"] = PIPELINE_VERSION
    rig["motion_families"] = json.dumps(MOTION_FAMILIES)
    rig["route_accessories"] = json.dumps({route: ROUTE_DEFINITIONS[route]["accessory"] for route in ROUTES})
    return {
        "collection": collection,
        "rig": rig,
        "species": species,
        "stage": stage,
        "route_objects": route_objects,
        "family_objects": family_objects,
        "expression_objects": expression_objects,
        "expression_geometry": expression_geometry,
    }


def reset_pose(model):
    for pose_bone in model["rig"].pose.bones:
        pose_bone.location = (0, 0, 0)
        pose_bone.rotation_euler = (0, 0, 0)
        pose_bone.scale = (1, 1, 1)
    for obj in model["expression_objects"]:
        geometry = model["expression_geometry"][obj.name]
        for vertex, coordinate in zip(obj.data.vertices, geometry["coordinates"]):
            vertex.co = coordinate
        obj.data.update()


def set_eye_height(model, multiplier):
    for eye in model["expression_objects"][:2]:
        geometry = model["expression_geometry"][eye.name]
        center = geometry["center"]
        for vertex, coordinate in zip(eye.data.vertices, geometry["coordinates"]):
            vertex.co = (coordinate.x, coordinate.y, center.z + (coordinate.z - center.z) * multiplier)
        eye.data.update()


def set_mouth(model, width, height):
    mouth = model["expression_objects"][2]
    geometry = model["expression_geometry"][mouth.name]
    center = geometry["center"]
    for vertex, coordinate in zip(mouth.data.vertices, geometry["coordinates"]):
        vertex.co = (
            center.x + (coordinate.x - center.x) * width,
            coordinate.y,
            center.z + (coordinate.z - center.z) * height,
        )
    mouth.data.update()


def apply_pose(model, family, frame_index):
    reset_pose(model)
    pose = model["rig"].pose.bones
    root = pose["root"]
    spine = pose["spine"]
    head = pose["head"]
    arm_l = pose["arm.L"]
    arm_r = pose["arm.R"]
    leg_l = pose["leg.L"]
    leg_r = pose["leg.R"]
    ear_l = pose["ear.L"]
    ear_r = pose["ear.R"]
    tail = pose["tail"]
    prop = pose["prop"]
    radians = math.radians

    if family == "idle":
        breath = (0.0, 0.018, 0.038, 0.014)[frame_index]
        sway = (-2.0, -0.5, 1.8, 0.4)[frame_index]
        root.scale = (1 + breath * 0.18, 1 + breath * 0.18, 1 + breath * 0.40)
        spine.scale = (1, 1, 1 + breath)
        head.rotation_euler[1] = radians(sway)
        ear_l.rotation_euler[1] = radians(-sway * 0.7)
        ear_r.rotation_euler[1] = radians(-sway * 0.7)
        tail.rotation_euler[1] = radians((4, 1, -5, -1)[frame_index])
        if frame_index == 2:
            set_eye_height(model, 0.18)
    elif family == "move":
        step = (-1, 1, -1, 1)[frame_index]
        lift = (0.0, 0.12, 0.025, 0.11)[frame_index]
        root.location.z = lift
        root.rotation_euler[1] = radians(step * 3.4)
        spine.rotation_euler[1] = radians(step * -2.0)
        arm_l.rotation_euler[1] = radians(step * 27)
        arm_r.rotation_euler[1] = radians(step * -27)
        leg_l.rotation_euler[1] = radians(step * -24)
        leg_r.rotation_euler[1] = radians(step * 24)
        tail.rotation_euler[1] = radians(step * 18)
        head.rotation_euler[1] = radians(step * 2.0)
    elif family == "joy":
        hop = (0.0, 0.14, 0.30, 0.08)[frame_index]
        cheer = (8, 30, 50, 18)[frame_index]
        root.location.z = hop
        root.scale = (1.0, 1.0, (1.0, 1.02, 1.06, 1.01)[frame_index])
        arm_l.rotation_euler[1] = radians(-cheer)
        arm_r.rotation_euler[1] = radians(cheer)
        leg_l.rotation_euler[1] = radians((0, -10, 16, -4)[frame_index])
        leg_r.rotation_euler[1] = radians((0, 10, -16, 4)[frame_index])
        head.rotation_euler[1] = radians((-4, 4, -5, 2)[frame_index])
        ear_l.rotation_euler[1] = radians((-3, -8, 8, 1)[frame_index])
        ear_r.rotation_euler[1] = radians((3, 8, -8, -1)[frame_index])
        tail.rotation_euler[1] = radians((-8, 12, -16, 6)[frame_index])
        set_eye_height(model, (1.0, 0.72, 0.45, 0.82)[frame_index])
        set_mouth(model, (1.0, 1.15, 1.30, 1.08)[frame_index], (1.0, 1.3, 1.65, 1.15)[frame_index])
    elif family == "rest":
        settle = (0.0, 0.08, 0.17, 0.12)[frame_index]
        root.location.z = -settle
        root.scale = (1 + settle * 0.35, 1, 1 - settle * 0.70)
        spine.rotation_euler[1] = radians((0, 5, 11, 8)[frame_index])
        head.location.z = (0, -0.06, -0.15, -0.10)[frame_index]
        head.rotation_euler[1] = radians((0, -7, -15, -10)[frame_index])
        arm_l.rotation_euler[1] = radians((0, -8, -16, -12)[frame_index])
        arm_r.rotation_euler[1] = radians((0, 8, 16, 12)[frame_index])
        ear_l.rotation_euler[1] = radians((0, 5, 12, 8)[frame_index])
        ear_r.rotation_euler[1] = radians((0, 5, 12, 8)[frame_index])
        set_eye_height(model, (0.75, 0.45, 0.16, 0.28)[frame_index])
        set_mouth(model, 0.75, 0.55)
    elif family == "focus":
        head.rotation_euler[1] = radians((-9, -3, 7, 1)[frame_index])
        spine.rotation_euler[1] = radians((2, -2, 3, 0)[frame_index])
        arm_l.rotation_euler[1] = radians((-12, -28, -36, -20)[frame_index])
        arm_r.rotation_euler[1] = radians((12, 28, 36, 20)[frame_index])
        ear_l.rotation_euler[1] = radians((5, -3, -8, 2)[frame_index])
        ear_r.rotation_euler[1] = radians((-5, 3, 8, -2)[frame_index])
        prop.rotation_euler[2] = radians((0, 16, 32, 48)[frame_index])
        root.location.z = (0, 0.025, 0.04, 0.015)[frame_index]
        set_eye_height(model, (1.0, 1.08, 1.15, 1.04)[frame_index])
    elif family == "file":
        root.rotation_euler[1] = radians((-2, 1, 3, 0)[frame_index])
        head.rotation_euler[1] = radians((4, -3, -7, 1)[frame_index])
        arm_l.rotation_euler[1] = radians((-10, -30, -48, -20)[frame_index])
        arm_r.rotation_euler[1] = radians((10, 30, 48, 20)[frame_index])
        pose["accessory"].location.z = (0.0, 0.06, 0.11, 0.025)[frame_index]
        pose["accessory"].scale = ((1, 1, 1), (1.04, 1.04, 1.04), (0.92, 0.92, 0.92), (1, 1, 1))[frame_index]
        set_eye_height(model, (1.0, 0.90, 0.72, 0.95)[frame_index])
        set_mouth(model, (1.0, 1.15, 1.45, 1.05)[frame_index], (1.0, 1.2, 1.45, 1.05)[frame_index])
    elif family == "alert":
        recoil = (0.0, 0.10, 0.22, 0.045)[frame_index]
        root.location.z = recoil
        root.scale = ((1, 1, 1), (1.05, 1.0, 0.94), (0.96, 1.0, 1.08), (1, 1, 1))[frame_index]
        head.rotation_euler[1] = radians((-8, 10, -9, 4)[frame_index])
        spine.rotation_euler[1] = radians((0, -6, 7, -2)[frame_index])
        arm_l.rotation_euler[1] = radians((0, 32, -25, 10)[frame_index])
        arm_r.rotation_euler[1] = radians((0, -32, 25, -10)[frame_index])
        ear_l.rotation_euler[1] = radians((0, -14, 12, -5)[frame_index])
        ear_r.rotation_euler[1] = radians((0, 14, -12, 5)[frame_index])
        prop.rotation_euler[1] = radians((0, -10, 12, -4)[frame_index])
        set_eye_height(model, (1.0, 1.22, 1.38, 1.10)[frame_index])
        set_mouth(model, (1.0, 0.8, 0.65, 0.9)[frame_index], (1.0, 1.5, 2.0, 1.2)[frame_index])
    else:
        raise ValueError(f"Unknown motion family: {family}")


def set_variant_visibility(model, route, family):
    for route_name, objects in model["route_objects"].items():
        visible = route_name == route
        for obj in objects:
            obj.hide_render = not visible
            obj.hide_set(not visible)
    for family_name, objects in model["family_objects"].items():
        visible = family_name == family
        for obj in objects:
            obj.hide_render = not visible
            obj.hide_set(not visible)


def point_at(obj, target):
    obj.rotation_euler = (Vector(target) - obj.location).to_track_quat("-Z", "Y").to_euler()


def setup_scene():
    scene = bpy.context.scene
    try:
        scene.render.engine = "BLENDER_EEVEE_NEXT"
    except TypeError:
        scene.render.engine = "BLENDER_EEVEE"
    scene.render.resolution_x = RENDER_SIZE
    scene.render.resolution_y = RENDER_SIZE
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = True
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.image_settings.color_depth = "8"
    scene.render.image_settings.compression = 90
    scene.render.use_file_extension = True
    scene.view_settings.look = "AgX - Medium High Contrast"
    scene.world.color = (0.035, 0.035, 0.05)
    scene.camera = None

    bpy.ops.object.camera_add(location=(0, -8.9, 2.68))
    camera = bpy.context.object
    camera.name = "Pet_Render_Camera"
    camera.data.type = "ORTHO"
    camera.data.ortho_scale = 4.65
    point_at(camera, (0, 0, 1.60))
    scene.camera = camera

    bpy.ops.object.light_add(type="AREA", location=(-3.8, -4.4, 6.2))
    key = bpy.context.object
    key.name = "Key_Light"
    key.data.energy = 760
    key.data.shape = "DISK"
    key.data.size = 4.4
    point_at(key, (0, 0, 1.4))

    bpy.ops.object.light_add(type="AREA", location=(3.6, -1.4, 3.8))
    fill = bpy.context.object
    fill.name = "Fill_Light"
    fill.data.energy = 430
    fill.data.size = 3.0
    point_at(fill, (0, 0, 1.5))

    bpy.ops.object.light_add(type="AREA", location=(0, 2.2, 5.6))
    rim = bpy.context.object
    rim.name = "Rim_Light"
    rim.data.energy = 580
    rim.data.size = 2.6
    point_at(rim, (0, 0, 1.7))

    scene["wormhole_pipeline_version"] = PIPELINE_VERSION
    scene["render_contract"] = f"{RENDER_SIZE}px RGBA PNG"
    return scene


def render_assets(scene, models):
    for model in models:
        model["collection"].hide_render = True
    for model_index, model in enumerate(models, start=1):
        model["collection"].hide_render = False
        species = model["species"]
        stage = model["stage"]
        print(f"[wormhole] rendering model {model_index}/{len(models)}: {species}/{stage}")
        for route in ROUTES:
            for family in MOTION_FAMILIES:
                set_variant_visibility(model, route, family)
                output = OUTPUT_ROOT / species / stage / route / family
                output.mkdir(parents=True, exist_ok=True)
                for frame_number in FRAME_VALUES:
                    apply_pose(model, family, frame_number - 1)
                    bpy.context.view_layer.update()
                    scene.render.filepath = str(output / f"{frame_number:02d}.png")
                    bpy.ops.render.render(write_still=True)
        reset_pose(model)
        set_variant_visibility(model, "companion", "idle")
        model["collection"].hide_render = True


def create_legacy_compatibility_copies():
    for species in SPECIES:
        for stage in STAGES:
            legacy_dir = OUTPUT_ROOT / species / stage
            legacy_dir.mkdir(parents=True, exist_ok=True)
            for frame_number in FRAME_VALUES:
                source = legacy_dir / "companion" / "idle" / f"{frame_number:02d}.png"
                shutil.copy2(source, legacy_dir / f"{frame_number:02d}.png")


def bake_preview_timeline(scene, models):
    family_ranges = {}
    scene.timeline_markers.clear()
    for family_index, family in enumerate(MOTION_FAMILIES):
        first = family_index * 10 + 1
        last = first + FRAME_COUNT - 1
        family_ranges[family] = [first, last]
        scene.timeline_markers.new(family.upper(), frame=first)

    for model in models:
        rig = model["rig"]
        rig.animation_data_create()
        action = bpy.data.actions.new(f"CLIPS_{model['species']}_{model['stage']}")
        rig.animation_data.action = action
        action["family_ranges"] = json.dumps(family_ranges, sort_keys=True)
        for family in MOTION_FAMILIES:
            first = family_ranges[family][0]
            for offset in range(FRAME_COUNT):
                apply_pose(model, family, offset)
                frame = first + offset
                for pose_bone in rig.pose.bones:
                    pose_bone.keyframe_insert(data_path="location", frame=frame, group=pose_bone.name)
                    pose_bone.keyframe_insert(data_path="rotation_euler", frame=frame, group=pose_bone.name)
                    pose_bone.keyframe_insert(data_path="scale", frame=frame, group=pose_bone.name)
        reset_pose(model)

    scene.frame_start = 1
    scene.frame_end = max(last for _, last in family_ranges.values())
    scene.frame_set(1)
    return family_ranges


def arrange_source(models):
    for index, model in enumerate(models):
        model["collection"].hide_render = False
        model["rig"].location = ((index % len(SPECIES) - 2) * 3.25, (index // len(SPECIES)) * 3.75, 0)
        set_variant_visibility(model, "companion", "idle")


def validate_action_coverage():
    actions_manifest = json.loads(ACTIONS_PATH.read_text(encoding="utf-8"))
    expected = set(actions_manifest["actions"])
    mapped = set(ACTION_TO_FAMILY)
    if expected != mapped:
        missing = sorted(expected - mapped)
        extra = sorted(mapped - expected)
        raise RuntimeError(f"Motion family map mismatch. Missing={missing}; extra={extra}")
    invalid_families = sorted(set(ACTION_TO_FAMILY.values()) - set(MOTION_FAMILIES))
    if invalid_families:
        raise RuntimeError(f"Unknown motion families in action map: {invalid_families}")


def validate_source_models(models):
    if len(models) != len(SPECIES) * len(STAGES):
        raise RuntimeError(f"Expected {len(SPECIES) * len(STAGES)} models, got {len(models)}")
    for model in models:
        bone_names = set(model["rig"].data.bones.keys())
        missing = set(REQUIRED_BONES) - bone_names
        if missing:
            raise RuntimeError(f"{model['species']}/{model['stage']} is missing bones: {sorted(missing)}")
        if model["rig"].animation_data is None or model["rig"].animation_data.action is None:
            raise RuntimeError(f"{model['species']}/{model['stage']} has no keyed animation action")


def build_manifest(family_ranges):
    script_hash = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    action_map_json = json.dumps(ACTION_TO_FAMILY, sort_keys=True, separators=(",", ":"))
    action_map_hash = hashlib.sha256(action_map_json.encode("utf-8")).hexdigest()
    build_hash = bpy.app.build_hash.decode("utf-8") if isinstance(bpy.app.build_hash, bytes) else str(bpy.app.build_hash)
    primary_count = len(SPECIES) * len(STAGES) * len(ROUTES) * len(MOTION_FAMILIES) * FRAME_COUNT
    compatibility_count = len(SPECIES) * len(STAGES) * FRAME_COUNT
    command = (
        "& 'C:\\Program Files\\Blender Foundation\\Blender 5.1\\blender.exe' "
        "--background --factory-startup --python tools\\blender\\generate_pet_models.py"
    )
    return {
        "schemaVersion": 2,
        "pipelineVersion": PIPELINE_VERSION,
        "generatedAtUtc": datetime.now(timezone.utc).isoformat(),
        "generator": {
            "script": "tools/blender/generate_pet_models.py",
            "scriptSha256": script_hash,
            "command": command,
        },
        "blender": {
            "version": bpy.app.version_string,
            "buildHash": build_hash,
        },
        "render": {
            "engine": bpy.context.scene.render.engine,
            "width": RENDER_SIZE,
            "height": RENDER_SIZE,
            "format": "PNG",
            "colorMode": "RGBA",
            "transparent": True,
            "frameCount": FRAME_COUNT,
            "pathPattern": "{species}/{stage}/{route}/{motionFamily}/{frame}.png",
            "legacyCompatibilityPattern": "{species}/{stage}/{frame}.png",
            "primaryAssetCount": primary_count,
            "compatibilityAssetCount": compatibility_count,
            "totalAssetCount": primary_count + compatibility_count,
        },
        "source": {
            "blend": "assets/blender/wormhole-pets.blend",
            "modelCount": len(SPECIES) * len(STAGES),
            "armaturePerModel": True,
            "bones": list(REQUIRED_BONES),
            "timelineRanges": family_ranges,
        },
        "species": list(SPECIES),
        "speciesAliases": SPECIES_ALIASES,
        "stages": list(STAGES),
        "routes": {
            route: {
                "accessory": ROUTE_DEFINITIONS[route]["accessory"],
                "accentRgba": list(ROUTE_DEFINITIONS[route]["accent"]),
            }
            for route in ROUTES
        },
        "motionFamilies": list(MOTION_FAMILIES),
        "actions": ACTION_TO_FAMILY,
        "actionMapSha256": action_map_hash,
        "runtimeContract": {
            "sourceType": "Blender-authored 3D armature baked to 2D PNG frames",
            "realTime3d": False,
            "fallback": "requested route/family -> companion family -> companion idle -> legacy Pai frames",
        },
    }


def write_manifest(manifest):
    encoded = json.dumps(manifest, ensure_ascii=False, indent=2) + "\n"
    PUBLIC_MANIFEST_PATH.parent.mkdir(parents=True, exist_ok=True)
    RUNTIME_MANIFEST_PATH.parent.mkdir(parents=True, exist_ok=True)
    PUBLIC_MANIFEST_PATH.write_text(encoded, encoding="utf-8")
    RUNTIME_MANIFEST_PATH.write_text(encoded, encoding="utf-8")


def main():
    validate_action_coverage()
    clean_output_root()
    BLEND_PATH.parent.mkdir(parents=True, exist_ok=True)
    reset_scene()
    scene = setup_scene()
    models = []
    for index, (stage, species) in enumerate((stage, species) for stage in STAGES for species in SPECIES):
        models.append(build_pet(species, stage, index))
    render_assets(scene, models)
    create_legacy_compatibility_copies()
    family_ranges = bake_preview_timeline(scene, models)
    validate_source_models(models)
    arrange_source(models)
    scene.render.filepath = ""
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND_PATH))
    manifest = build_manifest(family_ranges)
    write_manifest(manifest)
    print(
        "[wormhole] complete: "
        f"{manifest['render']['totalAssetCount']} PNG files, "
        f"{manifest['source']['modelCount']} armatures, "
        f"{len(ACTION_TO_FAMILY)} actions -> {len(MOTION_FAMILIES)} motion families"
    )


if __name__ == "__main__":
    main()
