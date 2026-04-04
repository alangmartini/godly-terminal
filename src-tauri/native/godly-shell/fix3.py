import os

# Fix builder.rs - add lighting_intensity param to fill_sdf and pass it through
with open('src/ui/builder.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# 1. Add lighting_intensity parameter to fill_sdf
old = """    fn fill_sdf(
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
        );
        self.emit_sdf(&verts);
    }"""

new = """    fn fill_sdf(
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

count = content.count(old)
print(f"fill_sdf pattern found {count} time(s)")
content = content.replace(old, new)

# 2. Fix callers of fill_sdf that don't pass lighting_intensity (6 args -> 7 args)
# These are calls like: self.fill_sdf(rect, color, [radius; 4], 0.0, [0.0; 4], 0.0);
# They need 1.0 added as the last arg

# fill_rounded: self.fill_sdf(rect, color, [radius; 4], 0.0, [0.0; 4], 0.0);
old2 = "self.fill_sdf(rect, color, [radius; 4], 0.0, [0.0; 4], 0.0);"
new2 = "self.fill_sdf(rect, color, [radius; 4], 0.0, [0.0; 4], 0.0, 1.0);"
count2 = content.count(old2)
print(f"fill_rounded caller found {count2} time(s)")
content = content.replace(old2, new2)

with open('src/ui/builder.rs', 'w', encoding='utf-8') as f:
    f.write(content)

print("builder.rs updated")

# Fix main.rs - add 1.0 to quad_vertices_sdf_gradient call
with open('src/main.rs', 'r', encoding='utf-8') as f:
    content = f.read()

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

count_main = content.count(old_main)
print(f"main.rs gradient caller found {count_main} time(s)")
content = content.replace(old_main, new_main)

with open('src/main.rs', 'w', encoding='utf-8') as f:
    f.write(content)

print("main.rs updated")
