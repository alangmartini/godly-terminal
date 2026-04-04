#!/usr/bin/env python
"""Fix all compilation errors related to missing lighting_intensity parameter."""

# Fix builder.rs
with open('src/ui/builder.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# 1. Add lighting_intensity param to fill_sdf signature
old_sig = """    fn fill_sdf(
        &mut self,
        rect: Rect,
        color: [f32; 4],
        radii: [f32; 4],
        border_width: f32,
        border_color: [f32; 4],
        blur_radius: f32,
    ) {
        let verts = quad_vertices_sdf(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.vw,
            self.vh,
            color,
            radii,
            border_width,
            border_color,
            blur_radius,
            self.lighting,
        );
        self.emit_sdf(&verts);
    }"""

new_sig = """    fn fill_sdf(
        &mut self,
        rect: Rect,
        color: [f32; 4],
        radii: [f32; 4],
        border_width: f32,
        border_color: [f32; 4],
        blur_radius: f32,
        lighting_intensity: f32,
    ) {
        let verts = quad_vertices_sdf(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.vw,
            self.vh,
            color,
            radii,
            border_width,
            border_color,
            blur_radius,
            lighting_intensity,
        );
        self.emit_sdf(&verts);
    }"""

c = content.count(old_sig)
print(f"fill_sdf signature: found {c}")
content = content.replace(old_sig, new_sig)

# 2. Fix all fill_sdf callers that have 6 args (missing lighting_intensity)
# Pattern: self.fill_sdf(rect, color, [...], border_width, border_color, blur);
# where blur is the last arg before );
# We need to find lines ending with a 6-arg call and add , 1.0

# Let's work line by line for the simple one-liners
lines = content.split('\n')
new_lines = []
i = 0
while i < len(lines):
    line = lines[i]
    # Check for single-line fill_sdf calls with 6 args (missing 7th)
    stripped = line.strip()
    if 'self.fill_sdf(' in stripped and stripped.endswith(');'):
        # Count commas to determine arg count
        # Extract the args part
        start = stripped.index('self.fill_sdf(') + len('self.fill_sdf(')
        end = stripped.rindex(');')
        args_str = stripped[start:end]
        # Count top-level commas (not inside brackets)
        depth = 0
        comma_count = 0
        for ch in args_str:
            if ch in '([':
                depth += 1
            elif ch in ')]':
                depth -= 1
            elif ch == ',' and depth == 0:
                comma_count += 1
        # 6 args = 5 commas, 7 args = 6 commas
        if comma_count == 5:
            # Missing lighting_intensity, add 1.0
            line = line.replace(');', ', 1.0);', 1)
            print(f"  Fixed single-line fill_sdf at line {i+1}")
        new_lines.append(line)
        i += 1
        continue

    # Check for multi-line fill_sdf calls
    if 'self.fill_sdf(' in stripped and not stripped.endswith(');'):
        # Collect lines until we find the closing );
        block = [line]
        j = i + 1
        while j < len(lines):
            block.append(lines[j])
            if lines[j].strip().endswith(');'):
                break
            j += 1

        # Join and count args
        block_str = '\n'.join(block)
        start = block_str.index('self.fill_sdf(') + len('self.fill_sdf(')
        end = block_str.rindex(');')
        args_str = block_str[start:end]
        depth = 0
        comma_count = 0
        for ch in args_str:
            if ch in '([':
                depth += 1
            elif ch in ')]':
                depth -= 1
            elif ch == ',' and depth == 0:
                comma_count += 1

        if comma_count == 5:
            # Find the last line that has the closing );
            # Insert 1.0 before );
            last_line = block[-1]
            # Add 1.0 arg before the closing
            # The line before ); should get a comma, and we add a new line with 1.0,
            # Actually, simpler: find the ); in the last line and add 1.0 before it
            # But the pattern is usually:
            #     0.0,
            # );
            # We need to add a line with 1.0,

            # Find the ); line and the line before it
            for k in range(len(block)-1, -1, -1):
                if block[k].strip() == ');':
                    # The arg before this should end with a comma
                    # Add 1.0 line before );
                    indent = ' ' * (len(block[k]) - len(block[k].lstrip()) + 4)
                    block.insert(k, indent + '1.0,')
                    print(f"  Fixed multi-line fill_sdf at line {i+1}")
                    break
                elif block[k].strip().endswith(');'):
                    # Last arg and ); on same line like "0.0,\n        );"
                    # Actually this case: the ); is on the last line with content
                    # e.g., "            0.0,"  then  "        );"
                    # We need to add "            1.0," before ");"
                    indent = ' ' * (len(block[k]) - len(block[k].lstrip()))
                    # Check if previous line ends with comma (it's an arg)
                    prev = block[k-1].rstrip()
                    if prev.endswith(','):
                        # Good, add new arg line before );
                        block.insert(k, indent + '    1.0,')
                        print(f"  Fixed multi-line fill_sdf (case 2) at line {i+1}")
                    break

        new_lines.extend(block)
        i = j + 1
        continue

    new_lines.append(line)
    i += 1

content = '\n'.join(new_lines)

with open('src/ui/builder.rs', 'w', encoding='utf-8') as f:
    f.write(content)

print("builder.rs done")

# Fix main.rs
with open('src/main.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# Find quad_vertices_sdf_gradient call missing lighting_intensity
old_main = """                                        &ui::quad_renderer::quad_vertices_sdf_gradient(
                                            cx,
                                            cy,
                                            cwidth,
                                            cheight,
                                            vw,
                                            vh,
                                            cursor_top,
                                            cursor_bot,
                                            [radius; 4],
                                            0.0,
                                            [0.0; 4],
                                        ),"""

new_main = """                                        &ui::quad_renderer::quad_vertices_sdf_gradient(
                                            cx,
                                            cy,
                                            cwidth,
                                            cheight,
                                            vw,
                                            vh,
                                            cursor_top,
                                            cursor_bot,
                                            [radius; 4],
                                            0.0,
                                            [0.0; 4],
                                            1.0,
                                        ),"""

c = content.count(old_main)
print(f"main.rs gradient call: found {c}")
content = content.replace(old_main, new_main)

with open('src/main.rs', 'w', encoding='utf-8') as f:
    f.write(content)

print("main.rs done")
