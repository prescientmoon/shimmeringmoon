use freetype::{
	bitmap::PixelMode,
	face::{KerningMode, LoadFlag},
	ffi::{FT_Err_Ok, FT_Set_Var_Design_Coordinates, FT_GLYPH_BBOX_PIXELS},
	Face, FtResult, Stroker, StrokerLineCap, StrokerLineJoin,
};
use num::traits::Euclid;

use crate::{assets::FREETYPE_LIB, context::Error};

// {{{ Config types
pub type Color = (u8, u8, u8, u8);

#[derive(Debug, Clone, Copy)]
pub enum Align {
	Start,
	Center,
	End,
}

#[derive(Debug, Clone, Copy)]
pub struct TextStyle {
	pub size: u32,
	pub weight: u32,
	pub color: Color,
	pub h_align: Align,
	pub v_align: Align,
}
// }}}
// {{{ BitmapCanvas
pub struct BitmapCanvas {
	pub buffer: Box<[u8]>,
	pub width: u32,
}

impl BitmapCanvas {
	// {{{ Draw pixel
	pub fn set_pixel(&mut self, pos: (u32, u32), color: Color) {
		let index = 3 * (pos.1 * self.width + pos.0) as usize;
		let alpha = color.3 as u32;
		self.buffer[index + 0] =
			((alpha * color.0 as u32 + (255 - alpha) * self.buffer[index + 0] as u32) / 255) as u8;
		self.buffer[index + 1] =
			((alpha * color.1 as u32 + (255 - alpha) * self.buffer[index + 1] as u32) / 255) as u8;
		self.buffer[index + 2] =
			((alpha * color.2 as u32 + (255 - alpha) * self.buffer[index + 2] as u32) / 255) as u8;
	}
	// }}}
	// {{{ Draw RBG image
	/// Draws a bitmap image
	pub fn blit_rbg(&mut self, pos: (i32, i32), (iw, ih): (u32, u32), src: &[u8]) {
		let height = self.buffer.len() as u32 / 3 / self.width;
		for dx in 0..iw {
			for dy in 0..ih {
				let x = pos.0 + dx as i32;
				let y = pos.1 + dy as i32;
				if x >= 0 && (x as u32) < self.width && y >= 0 && (y as u32) < height {
					let r = src[(dx + dy * iw) as usize * 3];
					let g = src[(dx + dy * iw) as usize * 3 + 1];
					let b = src[(dx + dy * iw) as usize * 3 + 2];

					let color = (r, g, b, 255);

					self.set_pixel((x as u32, y as u32), color);
				}
			}
		}
	}
	// }}}
	// {{{ Draw RGBA image
	/// Draws a bitmap image taking care of the alpha channel.
	pub fn blit_rbga(&mut self, pos: (i32, i32), (iw, ih): (u32, u32), src: &[u8]) {
		let height = self.buffer.len() as u32 / 3 / self.width;
		for dx in 0..iw {
			for dy in 0..ih {
				let x = pos.0 + dx as i32;
				let y = pos.1 + dy as i32;
				if x >= 0 && (x as u32) < self.width && y >= 0 && (y as u32) < height {
					let r = src[(dx + dy * iw) as usize * 4];
					let g = src[(dx + dy * iw) as usize * 4 + 1];
					let b = src[(dx + dy * iw) as usize * 4 + 2];
					let a = src[(dx + dy * iw) as usize * 4 + 3];

					let color = (r, g, b, a);

					self.set_pixel((x as u32, y as u32), color);
				}
			}
		}
	}
	// }}}
	// {{{ Fill
	/// Fill with solid color
	pub fn fill(&mut self, pos: (i32, i32), (iw, ih): (u32, u32), color: Color) {
		let height = self.buffer.len() as u32 / 3 / self.width;
		for dx in 0..iw {
			for dy in 0..ih {
				let x = pos.0 + dx as i32;
				let y = pos.1 + dy as i32;
				if x >= 0 && (x as u32) < self.width && y >= 0 && (y as u32) < height {
					self.set_pixel((x as u32, y as u32), color);
				}
			}
		}
	}
	// }}}
	// {{{ Draw text
	// TODO: perform gamma correction on the color interpolation.
	/// Render text
	pub fn text(
		&mut self,
		pos: (i32, i32),
		face: &mut Face,
		style: TextStyle,
		text: &str,
	) -> Result<(), Error> {
		// {{{ Control weight
		unsafe {
			let raw = face.raw_mut() as *mut _;
			let slice = [(style.weight as i64) << 16];

			// {{{ Debug logging
			// let mut amaster = 0 as *mut FT_MM_Var;
			// FT_Get_MM_Var(raw, &mut amaster as *mut _);
			// println!("{:?}", *amaster);
			// println!("{:?}", *(*amaster).axis);
			// println!("{:?}", *(*amaster).namedstyle);
			// }}}

			// Set variable weight
			let err = FT_Set_Var_Design_Coordinates(raw, 3, slice.as_ptr());
			if err != FT_Err_Ok {
				let err: FtResult<_> = Err(err.into());
				err?;
			}
		}
		// }}}
		face.set_char_size((style.size << 6) as isize, 0, 0, 0)?;

		// {{{ Compute layout
		let mut pen_x = 0;
		let kerning = face.has_kerning();
		let mut previous = None;
		let mut data = Vec::new();

		for c in text.chars() {
			let glyph_index = face
				.get_char_index(c as usize)
				.ok_or_else(|| format!("Could not get glyph index for char {:?}", c))?;

			if let Some(previous) = previous
				&& kerning
			{
				let delta = face.get_kerning(previous, glyph_index, KerningMode::KerningDefault)?;
				pen_x += delta.x >> 6; // we shift to get rid of sub-pixel accuracy
			}

			face.load_glyph(glyph_index, LoadFlag::DEFAULT)?;

			data.push((pen_x, face.glyph().get_glyph()?));
			pen_x += face.glyph().advance().x >> 6;
			previous = Some(glyph_index);
		}

		// }}}
		// {{{ Find bounding box
		let mut x_min = 32000;
		let mut y_min = 32000;
		let mut x_max = -32000;
		let mut y_max = -32000;

		for (pen_x, glyph) in &data {
			let mut bbox = glyph.get_cbox(FT_GLYPH_BBOX_PIXELS);

			bbox.xMin += pen_x;
			bbox.xMax += pen_x;

			if bbox.xMin < x_min {
				x_min = bbox.xMin
			}

			if bbox.xMax > x_max {
				x_max = bbox.xMax
			}

			if bbox.yMin < y_min {
				y_min = bbox.yMin
			}

			if bbox.yMax > y_max {
				y_max = bbox.yMax
			}
		}

		// Check that we really grew the string bbox
		if x_min > x_max {
			x_min = 0;
			x_max = 0;
			y_min = 0;
			y_max = 0;
		}

		// println!("{}, {} - {}, {}", x_min, y_min, x_max, y_max);

		// }}}
		// {{{ Render glyphs
		for (pos_x, glyph) in &data {
			let b_glyph = glyph.to_bitmap(freetype::RenderMode::Normal, None)?;
			let bitmap = b_glyph.bitmap();
			let pixel_mode = bitmap.pixel_mode()?;
			assert_eq!(pixel_mode, PixelMode::Gray);
			println!("starting to stroke");

			// {{{ Blit border
			let stroker = FREETYPE_LIB.with(|lib| lib.new_stroker())?;
			stroker.set(1 << 6, StrokerLineCap::Round, StrokerLineJoin::Round, 0);
			let sglyph = glyph.stroke(&stroker)?;
			let sb_glyph = sglyph.to_bitmap(freetype::RenderMode::Normal, None)?;
			let sbitmap = sb_glyph.bitmap();
			let spixel_mode = sbitmap.pixel_mode()?;
			assert_eq!(spixel_mode, PixelMode::Gray);

			let iw = sbitmap.width();
			let ih = sbitmap.rows();
			println!("pitch {}, width {}, height {}", sbitmap.pitch(), iw, ih);
			let height = self.buffer.len() as u32 / 3 / self.width;
			let src = sbitmap.buffer();
			for dx in 0..iw {
				for dy in 0..ih {
					let x = pos.0 + *pos_x as i32 + dx as i32 + sb_glyph.left();
					let y = pos.1 + dy as i32 - sb_glyph.top();
					if x >= 0 && (x as u32) < self.width && y >= 0 && (y as u32) < height {
						let gray = src[(dx + dy * iw) as usize];

						let r = 255 - style.color.0;
						let g = 255 - style.color.1;
						let b = 255 - style.color.2;
						let a = gray;

						let color = (r, g, b, a);

						self.set_pixel((x as u32, y as u32), color);
					}
				}
			}
			// }}}
			// {{{ Blit
			let iw = bitmap.width();
			let ih = bitmap.rows();
			let height = self.buffer.len() as u32 / 3 / self.width;
			let src = bitmap.buffer();

			for dx in 0..iw {
				for dy in 0..ih {
					let x = pos.0 + *pos_x as i32 + dx as i32 + b_glyph.left();
					let y = pos.1 + dy as i32 - b_glyph.top();
					if x >= 0 && (x as u32) < self.width && y >= 0 && (y as u32) < height {
						let gray = src[(dx + dy * iw) as usize];

						let r = style.color.0;
						let g = style.color.1;
						let b = style.color.2;
						let a = gray;

						let color = (r, g, b, a);

						self.set_pixel((x as u32, y as u32), color);
					}
				}
			}
			// }}}
		}
		// }}}

		Ok(())
	}
	// }}}

	#[inline]
	pub fn new(width: u32, height: u32) -> Self {
		let buffer = vec![u8::MAX; 8 * 3 * (width * height) as usize].into_boxed_slice();
		Self { buffer, width }
	}
}
// }}}
// {{{ Layout types
#[derive(Clone, Copy, Debug)]
pub struct LayoutBox {
	relative_to: Option<(LayoutBoxId, i32, i32)>,
	pub width: u32,
	pub height: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LayoutBoxId(usize);

#[derive(Default, Debug)]
pub struct LayoutManager {
	boxes: Vec<LayoutBox>,
}

pub struct LayoutDrawer {
	pub layout: LayoutManager,
	pub canvas: BitmapCanvas,
}

impl LayoutManager {
	// {{{ Trivial box creation
	pub fn make_box(&mut self, width: u32, height: u32) -> LayoutBoxId {
		let id = self.boxes.len();
		self.boxes.push(LayoutBox {
			relative_to: None,
			width,
			height,
		});

		LayoutBoxId(id)
	}

	pub fn make_relative_box(
		&mut self,
		to: LayoutBoxId,
		x: i32,
		y: i32,
		width: u32,
		height: u32,
	) -> LayoutBoxId {
		let id = self.make_box(width, height);
		self.edit_to_relative(id, to, x, y);

		id
	}
	// }}}
	// {{{ Chage box to be relative
	pub fn edit_to_relative(
		&mut self,
		id: LayoutBoxId,
		id_relative_to: LayoutBoxId,
		x: i32,
		y: i32,
	) {
		let current = self.boxes[id.0];
		let to = self.boxes[id_relative_to.0];
		if let Some((current_points_to, dx, dy)) = current.relative_to
			&& current_points_to != id_relative_to
		{
			self.edit_to_relative(current_points_to, id_relative_to, x - dx, y - dy);
		} else {
			self.boxes[id.0].relative_to = Some((id_relative_to, x, y));
		}

		{
			let a = self.lookup(id);
			let b = self.lookup(id_relative_to);
			assert_eq!((a.0 - b.0, a.1 - b.1), (x, y));
		}
	}
	// }}}
	// {{{ Margins
	#[inline]
	pub fn margin(&mut self, id: LayoutBoxId, t: i32, r: i32, b: i32, l: i32) -> LayoutBoxId {
		let inner = self.boxes[id.0];
		let out = self.make_box(
			(inner.width as i32 + l + r) as u32,
			(inner.height as i32 + t + b) as u32,
		);
		self.edit_to_relative(id, out, l, t);
		out
	}

	#[inline]
	pub fn margin_xy(&mut self, inner: LayoutBoxId, x: i32, y: i32) -> LayoutBoxId {
		self.margin(inner, y, x, y, x)
	}

	#[inline]
	pub fn margin_uniform(&mut self, inner: LayoutBoxId, amount: i32) -> LayoutBoxId {
		self.margin(inner, amount, amount, amount, amount)
	}
	// }}}
	// {{{ Glueing
	#[inline]
	pub fn glue_horizontally(
		&mut self,
		first_id: LayoutBoxId,
		second_id: LayoutBoxId,
	) -> LayoutBoxId {
		let first = self.boxes[first_id.0];
		let second = self.boxes[second_id.0];
		let id = self.make_box(first.width.max(second.width), first.height + second.height);

		self.edit_to_relative(first_id, id, 0, 0);
		self.edit_to_relative(second_id, id, 0, first.height as i32);
		id
	}

	#[inline]
	pub fn glue_vertically(
		&mut self,
		first_id: LayoutBoxId,
		second_id: LayoutBoxId,
	) -> LayoutBoxId {
		let first = self.boxes[first_id.0];
		let second = self.boxes[second_id.0];
		let id = self.make_box(first.width + second.width, first.height.max(second.height));

		self.edit_to_relative(first_id, id, 0, 0);
		self.edit_to_relative(second_id, id, first.width as i32, 0);
		id
	}
	// }}}
	// {{{ Repeating
	pub fn repeated_evenly(
		&mut self,
		id: LayoutBoxId,
		amount: (u32, u32),
	) -> (LayoutBoxId, impl Iterator<Item = (i32, i32)>) {
		let inner = self.boxes[id.0];
		let outer_id = self.make_box(inner.width * amount.0, inner.height * amount.1);
		self.edit_to_relative(id, outer_id, 0, 0);

		(
			outer_id,
			(0..amount.0 * amount.1).into_iter().map(move |i| {
				let (y, x) = i.div_rem_euclid(&amount.0);
				((x * inner.width) as i32, (y * inner.height) as i32)
			}),
		)
	}
	// }}}
	// {{{ Lookup box
	pub fn lookup(&self, id: LayoutBoxId) -> (i32, i32, u32, u32) {
		let current = self.boxes[id.0];
		if let Some((to, dx, dy)) = current.relative_to {
			let (x, y, _, _) = self.lookup(to);
			(x + dx, y + dy, current.width, current.height)
		} else {
			(0, 0, current.width, current.height)
		}
	}

	#[inline]
	pub fn width(&self, id: LayoutBoxId) -> u32 {
		self.boxes[id.0].width
	}

	#[inline]
	pub fn height(&self, id: LayoutBoxId) -> u32 {
		self.boxes[id.0].height
	}

	#[inline]
	pub fn position_relative_to(&self, id: LayoutBoxId, pos: (i32, i32)) -> (i32, i32) {
		let current = self.lookup(id);
		((pos.0 as i32 + current.0), (pos.1 as i32 + current.1))
	}
	// }}}
}

impl LayoutDrawer {
	pub fn new(layout: LayoutManager, canvas: BitmapCanvas) -> Self {
		Self { layout, canvas }
	}

	// {{{ Drawing
	// {{{ Draw pixel
	pub fn set_pixel(&mut self, id: LayoutBoxId, pos: (u32, u32), color: Color) {
		let pos = self
			.layout
			.position_relative_to(id, (pos.0 as i32, pos.1 as i32));
		self.canvas.set_pixel((pos.0 as u32, pos.1 as u32), color);
	}
	// }}}
	// {{{ Draw RGB image
	/// Draws a bitmap image
	pub fn blit_rbg(&mut self, id: LayoutBoxId, pos: (i32, i32), dims: (u32, u32), src: &[u8]) {
		let pos = self.layout.position_relative_to(id, pos);
		self.canvas.blit_rbg(pos, dims, src);
	}
	// }}}
	// {{{ Draw RGBA image
	/// Draws a bitmap image taking care of the alpha channel.
	pub fn blit_rbga(&mut self, id: LayoutBoxId, pos: (i32, i32), dims: (u32, u32), src: &[u8]) {
		let pos = self.layout.position_relative_to(id, pos);
		self.canvas.blit_rbga(pos, dims, src);
	}
	// }}}
	// {{{ Fill
	/// Fills with solid color
	pub fn fill(&mut self, id: LayoutBoxId, color: Color) {
		let current = self.layout.lookup(id);
		self.canvas
			.fill((current.0, current.1), (current.2, current.3), color);
	}
	// }}}
	// {{{ Draw text
	/// Render text
	pub fn text(
		&mut self,
		id: LayoutBoxId,
		pos: (i32, i32),
		face: &mut Face,
		style: TextStyle,
		text: &str,
	) -> Result<(), Error> {
		let pos = self.layout.position_relative_to(id, pos);
		self.canvas.text(pos, face, style, text)
	}
	// }}}
	// }}}
}
// }}}
