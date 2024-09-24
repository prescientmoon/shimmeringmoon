use shimmeringmoon::context::UserContext;

#[derive(Clone, Copy)]
pub struct AppContext {
	pub ctx: &'static UserContext,
}

impl AppContext {
	pub fn new(ctx: &'static UserContext) -> Self {
		Self { ctx }
	}
}
