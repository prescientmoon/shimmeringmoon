/// Performs "Ok-wrapping" on the result of an expression.
/// This is compatible with [`Result`], [`Option`], [`ControlFlow`], and any type that
/// implements the unstable [`std::ops::Try`] trait.
///
/// The destination type must be specified with a type ascription somewhere.
#[macro_export]
macro_rules! wrap_ok {
	($e:expr) => {
		::core::iter::empty().try_fold($e, |_, __x: ::core::convert::Infallible| match __x {})
	};
}

#[macro_export]
macro_rules! try_block {
    { $($token:tt)* } => {
        (|| $crate::wrap_ok!({
            $($token)*
        }))()
    }
}

#[macro_export]
macro_rules! async_try_block {
    { $($token:tt)* } => {
        (async || $crate::wrap_ok!({
            $($token)*
        }))().await
    }
}
