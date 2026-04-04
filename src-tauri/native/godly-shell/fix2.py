with open('src/ui/quad_renderer.rs', 'r') as f:
    content = f.read()

# Fix gradient_h closure: add lighting_intensity after rotation: 0.0
# This is the only occurrence of this exact pattern
old1 = """        blur_radius: 0.0,
        rotation: 0.0,
    };

    // Horizontal: left=fill_color_left, right=fill_color_right"""

new1 = """        blur_radius: 0.0,
        rotation: 0.0,
        lighting_intensity,
    };

    // Horizontal: left=fill_color_left, right=fill_color_right"""

count1 = content.count(old1)
print(f"gradient_h closure pattern found {count1} time(s)")
content = content.replace(old1, new1)

# Fix rotated closure: add lighting_intensity after rotation
old2 = """        blur_radius,
        rotation,
    };

    [
        v([x0, y0], [lx0, ly0]),"""

new2 = """        blur_radius,
        rotation,
        lighting_intensity,
    };

    [
        v([x0, y0], [lx0, ly0]),"""

count2 = content.count(old2)
print(f"rotated closure pattern found {count2} time(s)")
content = content.replace(old2, new2)

with open('src/ui/quad_renderer.rs', 'w') as f:
    f.write(content)

print("Done")
