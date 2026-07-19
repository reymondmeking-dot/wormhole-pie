# Blender pet bake pipeline

The desktop runtime remains a lightweight transparent 2D sprite window. The
source characters are authored as Blender 3D models with real armatures, then
baked to 256×256 RGBA PNG frames. No Blender runtime or real-time 3D engine is
required on an end user's machine.

## Rebuild with Blender 5.1

Run from the repository root in PowerShell:

```powershell
& 'C:\Program Files\Blender Foundation\Blender 5.1\blender.exe' `
  --background --factory-startup `
  --python tools\blender\generate_pet_models.py
```

The generator recreates only `public/pets/blender-rendered`, writes the editable
source file to `assets/blender/wormhole-pets.blend`, and updates both manifest
copies:

- `public/pets/blender-rendered/manifest.json`
- `src/pet/blenderMotionMap.json`

The manifest records the pipeline version, Blender build, generator SHA-256,
render parameters, route accessories, motion-family ranges, and the complete
44-action mapping.

## Asset contract

Primary frames use:

```text
public/pets/blender-rendered/{species}/{stage}/{route}/{motionFamily}/{frame}.png
```

The five shipped species, three stages, three evolution routes, seven reusable
motion families, and four frames produce 1,260 primary PNGs. Another 60
`{species}/{stage}/{frame}.png` copies preserve the previous idle-only path.

Evolution routes have visible 3D-authored distinctions:

- `companion`: heart accessory and rose accent
- `creator`: orbit accessory and amber accent
- `guardian`: shield accessory and blue accent

The seven motion families are `idle`, `move`, `joy`, `rest`, `focus`, `file`,
and `alert`. They cover every action declared in `public/pets/pai/actions.json`,
so runtime actions do not need to switch back to the old Pai art style during
normal operation.

## Validation

Validate every PNG, alpha channel, dimension, action mapping, output count, and
generator hash; this also creates a representative review sheet:

```powershell
python tools\blender\validate_pet_renders.py
```

Inspect the saved Blender source for armatures, required bones, animation
actions, timeline ranges, and model collections:

```powershell
& 'C:\Program Files\Blender Foundation\Blender 5.1\blender.exe' `
  --background assets\blender\wormhole-pets.blend `
  --python tools\blender\inspect_pet_blend.py
```

The generated preview is `assets/blender/pet-render-preview.png`.
