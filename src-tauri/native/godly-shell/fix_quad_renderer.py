import sys

with open('src/ui/quad_renderer.rs', 'r') as f:
    lines = f.readlines()

content = ''.join(lines)

# Fix 1: quad_vertices_sdf_gradient - add param and use it in closure
# The function signature ends with border_color: [f32; 4], before ) -> [QuadVertex; 6]
# And the closure needs lighting_intensity added

# Fix 2: quad_vertices_sdf_gradient_h - same pattern

# Fix 3: quad_vertices_sdf_rotated - add param and use it in closure

# Strategy: work line by line, find the specific locations

result = []
i = 0
while i < len(lines):
    line = lines[i]

    # Detect the gradient function signatures that need fixing
    # Pattern: "    border_color: [f32; 4]," followed by ") -> [QuadVertex; 6] {"
    # But only for the gradient/gradient_h functions (not quad_vertices_sdf which already has it)

    # For gradient and gradient_h: after border_color param, add lighting_intensity param
    if (line.strip() == 'border_color: [f32; 4],' and
        i + 1 < len(lines) and
        lines[i+1].strip() == ') -> [QuadVertex; 6] {'):
        # Check if next param is already lighting_intensity
        result.append(line)
        result.append('    lighting_intensity: f32,\n')
        i += 1
        continue

    # For rotated: after blur_radius param, add lighting_intensity param
    if (line.strip() == 'blur_radius: f32,' and
        i + 1 < len(lines) and
        lines[i+1].strip() == ') -> [QuadVertex; 6] {'):
        result.append(line)
        result.append('    lighting_intensity: f32,\n')
        i += 1
        continue

    # Fix the closures that are missing lighting_intensity
    # Pattern: "        rotation: 0.0," or "        rotation," followed by "    };"
    if (line.strip() in ('rotation: 0.0,', 'rotation,') and
        i + 1 < len(lines) and
        lines[i+1].strip() == '};'):
        # Check if lighting_intensity is already there (quad_vertices_sdf already has it)
        # Look backwards to see which function we're in
        result.append(line)
        # Add lighting_intensity
        indent = '        '
        if line.strip() == 'rotation: 0.0,':
            result.append(indent + 'lighting_intensity,\n')
        else:
            result.append(indent + 'lighting_intensity,\n')
        i += 1
        continue

    result.append(line)
    i += 1

with open('src/ui/quad_renderer.rs', 'w') as f:
    f.writelines(result)

print("Done fixing quad_renderer.rs")
