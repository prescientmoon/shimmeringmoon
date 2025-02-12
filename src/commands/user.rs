use anyhow::anyhow;

use crate::{
	context::{Error, ErrorKind, PoiseContext, TagError, TaggedError},
	user::User,
};

use super::discord::MessageContext;

// {{{ Toplevel
/// User management
#[poise::command(
	prefix_command,
	slash_command,
	subcommands("register", "pookify", "bind", "unbind", "friend"),
	subcommand_required
)]
pub async fn user(_ctx: PoiseContext<'_>) -> Result<(), Error> {
	Ok(())
}
// }}}
// {{{ Register
async fn register_impl<C: MessageContext>(
	ctx: &mut C,
	target_user: poise::serenity_prelude::User,
) -> Result<(), TaggedError> {
	let user = User::from_context(ctx)?;
	user.assert_is_pookie()?;

	match User::by_discord_id(ctx.data(), target_user.id) {
		Ok(_) => {
			ctx.reply("An account for this user already exists!")
				.await?;
		}
		Err(error) if error.kind == ErrorKind::Internal => return Err(error),
		Err(_) => {
			let rows_changed = ctx
				.data()
				.db
				.get()?
				.prepare_cached("INSERT INTO users(discord_id) VALUES (?)")?
				.execute([&target_user.id.to_string()])?;

			assert!(rows_changed > 0);
			ctx.reply("Succesfully created user account!").await?;
		}
	}

	Ok(())
}

/// Create an account for another discord user
#[poise::command(
	prefix_command,
	slash_command,
	context_menu_command = "Register user",
	install_context = "Guild",
	interaction_context = "Guild|BotDm|PrivateChannel"
)]
async fn register(
	mut ctx: PoiseContext<'_>,
	user: poise::serenity_prelude::User,
) -> Result<(), Error> {
    ctx.defer().await?;
	let res = register_impl(&mut ctx, user).await;
	ctx.handle_error(res).await?;
	Ok(())
}
// }}}
// {{{ Pookify
async fn pookify_impl<C: MessageContext>(
	ctx: &mut C,
	target_user: poise::serenity_prelude::User,
) -> Result<(), TaggedError> {
	let user = User::from_context(ctx)?;
	user.assert_is_admin()?;

	let user = User::by_discord_id(ctx.data(), target_user.id)?;
	if user.is_pookie || user.is_admin {
		ctx.reply("This user is already a pookie of mine!").await?;
	} else {
		ctx.data()
			.db
			.get()?
			.prepare_cached("UPDATE users SET is_pookie=1 WHERE id=?")?
			.execute([user.id])?;

		ctx.reply("Succesfully added user to my pookie list!")
			.await?;
	}

	Ok(())
}

/// Add the given user to my pookie list
#[poise::command(
	prefix_command,
	slash_command,
	context_menu_command = "Pookify user",
	install_context = "Guild",
	interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn pookify(
	mut ctx: PoiseContext<'_>,
	user: poise::serenity_prelude::User,
) -> Result<(), Error> {
    ctx.defer().await?;
	let res = pookify_impl(&mut ctx, user).await;
	ctx.handle_error(res).await?;
	Ok(())
}
// }}}
// {{{ Bind
async fn bind_impl<C: MessageContext>(ctx: &mut C, username: String) -> Result<(), TaggedError> {
	let user = User::from_context(ctx)?;

	let result = crate::private_server::users(
		ctx.data(),
		crate::private_server::UsersQueryOptions {
			query: Some(crate::private_server::UsersQuery {
				name: Some(&username),
				..Default::default()
			}),
		},
	)
	.await?
	.into_iter()
	.next()
	.unwrap();

	ctx.data()
		.db
		.get()?
		.prepare_cached("UPDATE users SET private_server_id=? WHERE id=?")?
		.execute((result.user_id, user.id))?;

	ctx.reply("Succesfully bound account!").await?;

	Ok(())
}

/// Bind your account to an account on the associated private server
#[poise::command(prefix_command, slash_command)]
async fn bind(mut ctx: PoiseContext<'_>, username: String) -> Result<(), Error> {
	let res = bind_impl(&mut ctx, username).await;
	ctx.handle_error(res).await?;
	Ok(())
}
// }}}
// {{{ Friend code
async fn friend_code_impl<C: MessageContext>(
	ctx: &mut C,
	target_user: poise::serenity_prelude::User,
) -> Result<(), TaggedError> {
	User::from_context(ctx)?;

	let target = User::by_discord_id(ctx.data(), target_user.id)?;

	let user_id = target
		.private_server_id
		.ok_or_else(|| anyhow!("This person hasn't bound their discord account to any account on the associated private server...").tag(ErrorKind::User))?;

	let result = crate::private_server::users(
		ctx.data(),
		crate::private_server::UsersQueryOptions {
			query: Some(crate::private_server::UsersQuery {
				user_id: Some(user_id),
				..Default::default()
			}),
		},
	)
	.await?
	.into_iter()
	.next()
	.unwrap();

	ctx.reply(&format!(
		"You can add `{}` as a friend using the code `{}`",
		&result.name, &result.user_code
	))
	.await?;

	Ok(())
}

/// Lookup the friend code of the given user
#[poise::command(prefix_command, slash_command)]
async fn friend(
	mut ctx: PoiseContext<'_>,
	user: poise::serenity_prelude::User,
) -> Result<(), Error> {
	let res = friend_code_impl(&mut ctx, user).await;
	ctx.handle_error(res).await?;
	Ok(())
}
// }}}
// {{{ Unbind
async fn unbind_impl<C: MessageContext>(ctx: &mut C) -> Result<(), TaggedError> {
	let user = User::from_context(ctx)?;

	if user.private_server_id.is_some() {
		ctx.data()
			.db
			.get()?
			.prepare_cached("UPDATE users SET private_server_id=NULL WHERE id=?")?
			.execute([user.id])?;

		ctx.reply("Succesfully unbound account.").await?;
	} else {
		ctx.reply("There's no account to unbind 🤔").await?;
	}

	Ok(())
}

/// Unbind your account from an account on the associated private server
#[poise::command(prefix_command, slash_command)]
async fn unbind(mut ctx: PoiseContext<'_>) -> Result<(), Error> {
	let res = unbind_impl(&mut ctx).await;
	ctx.handle_error(res).await?;
	Ok(())
}
// }}}
