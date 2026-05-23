pub const APP_ICON_SIZE: u32 = 32;

pub fn app_icon_rgba() -> Vec<u8> {
    let mut pixels = vec![0_u8; (APP_ICON_SIZE * APP_ICON_SIZE * 4) as usize];

    draw_ellipse(&mut pixels, 14, 16, 12, 10, [78, 54, 38, 255]);
    draw_ellipse(&mut pixels, 14, 16, 10, 8, [224, 174, 96, 255]);
    draw_circle(&mut pixels, 10, 13, 3, [245, 235, 211, 255]);
    draw_circle(&mut pixels, 15, 10, 2, [210, 73, 65, 255]);
    draw_circle(&mut pixels, 20, 13, 2, [61, 139, 91, 255]);
    draw_circle(&mut pixels, 17, 20, 2, [57, 101, 188, 255]);
    draw_circle(&mut pixels, 9, 19, 2, [236, 206, 82, 255]);

    draw_brush(&mut pixels);
    pixels
}

fn draw_brush(pixels: &mut [u8]) {
    for step in 0..11 {
        let x = 18 + step;
        let y = 22 + step / 2;
        draw_circle(pixels, x, y, 1, [112, 72, 42, 255]);
        draw_circle(pixels, x + 1, y + 1, 1, [112, 72, 42, 255]);
    }
    draw_circle(pixels, 18, 22, 2, [42, 45, 52, 255]);
    draw_circle(pixels, 17, 21, 1, [238, 238, 230, 255]);
}

fn draw_ellipse(
    pixels: &mut [u8],
    center_x: i32,
    center_y: i32,
    radius_x: i32,
    radius_y: i32,
    color: [u8; 4],
) {
    let radius_x_sq = radius_x * radius_x;
    let radius_y_sq = radius_y * radius_y;
    let limit = radius_x_sq * radius_y_sq;

    for y in (center_y - radius_y)..=(center_y + radius_y) {
        for x in (center_x - radius_x)..=(center_x + radius_x) {
            let dx = x - center_x;
            let dy = y - center_y;
            if dx * dx * radius_y_sq + dy * dy * radius_x_sq <= limit {
                set_pixel(pixels, x, y, color);
            }
        }
    }
}

fn draw_circle(pixels: &mut [u8], center_x: i32, center_y: i32, radius: i32, color: [u8; 4]) {
    let radius_sq = radius * radius;
    for y in (center_y - radius)..=(center_y + radius) {
        for x in (center_x - radius)..=(center_x + radius) {
            let dx = x - center_x;
            let dy = y - center_y;
            if dx * dx + dy * dy <= radius_sq {
                set_pixel(pixels, x, y, color);
            }
        }
    }
}

fn set_pixel(pixels: &mut [u8], x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= APP_ICON_SIZE as i32 || y >= APP_ICON_SIZE as i32 {
        return;
    }

    let offset = ((y as u32 * APP_ICON_SIZE + x as u32) * 4) as usize;
    pixels[offset..offset + 4].copy_from_slice(&color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_icon_has_expected_rgba_shape() {
        let pixels = app_icon_rgba();

        assert_eq!(pixels.len(), (APP_ICON_SIZE * APP_ICON_SIZE * 4) as usize);
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
    }
}
