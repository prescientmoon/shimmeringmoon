pub mod two_columns;

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
