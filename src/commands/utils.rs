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
	($ctx:expr) => {
		match crate::user::User::from_context($ctx).await {
			Ok(user) => user,
			Err(_) => {
				$ctx.say("You are not an user in my database, sorry!")
					.await?;
				return Ok(());
			}
		}
	};
}
