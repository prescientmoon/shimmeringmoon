#[macro_export]
macro_rules! edit_reply {
    ($ctx:expr, $handle:expr, $($arg:tt)*) => {{
        let content = format!($($arg)*);
        let edited = CreateReply::default()
            .reply(true)
            .content(content);
        $handle.edit($ctx, edited)
    }};
}

#[macro_export]
macro_rules! get_user {
	($ctx:expr) => {{
		crate::reply_errors!($ctx, crate::user::User::from_context($ctx).await)
	}};
}

#[macro_export]
macro_rules! reply_errors {
	($ctx:expr, $value:expr) => {
		match $value {
			Ok(v) => v,
			Err(err) => {
				$ctx.reply(format!("{err}")).await?;
				return Ok(());
			}
		}
	};
}
