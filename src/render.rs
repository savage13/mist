//Functions related to rendering information to the SDL window

use sdl2::video::Window;
use sdl2::pixels::Color;
use sdl2::rect::{Rect, Point};
use sdl2::render::{TextureQuery, Texture, Canvas};

//draws split names that have been made into textures previously
pub fn render_rows(on_screen: &Vec<Texture>, canvas: &mut Canvas<Window>, window_width: u32) {
	let mut row = Rect::new(0, 0, 0, 0);
	let mut y = 0;
	for item in on_screen {
		let TextureQuery{width, height, ..} = item.query();
		row.set_y(y);
		row.set_width(width);
		row.set_height(height);
		canvas.set_draw_color(Color::WHITE);
		canvas.copy(&item, None, Some(row)).unwrap();
		canvas.set_draw_color(Color::GRAY);
		canvas.draw_line(Point::new(0, y + height as i32 + 3), Point::new(window_width as i32, y + height as i32 + 3)).unwrap();
		y += height as i32 + 5;
		canvas.set_draw_color(Color::BLACK);
	}
}