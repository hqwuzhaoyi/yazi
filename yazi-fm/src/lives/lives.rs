use std::{mem, sync::Arc};

use mlua::{Scope, Table};
use tracing::error;
use yazi_config::LAYOUT;
use yazi_plugin::{elements::RectRef, LUA};
use yazi_shared::{Defer, RoCell};

use crate::Ctx;

pub(super) static SCOPE: RoCell<&mlua::Scope> = RoCell::new();

pub(crate) struct Lives;

impl Lives {
	pub(crate) fn register() -> mlua::Result<()> {
		super::Config::register(&LUA)?;
		super::File::register(&LUA)?;
		super::Files::register(&LUA)?;
		super::Folder::register(&LUA)?;
		super::Mode::register(&LUA)?;
		super::Preview::register(&LUA)?;
		super::Selected::register(&LUA)?;
		super::Tab::register(&LUA)?;
		super::Tabs::register(&LUA)?;
		super::Tasks::register(&LUA)?;
		super::Yanked::register(&LUA)?;

		Ok(())
	}

	pub(crate) fn scope<'a, T>(
		cx: &'a Ctx,
		f: impl FnOnce(&Scope<'a, 'a>) -> mlua::Result<T>,
	) -> mlua::Result<T> {
		let result = LUA.scope(|scope| {
			let _defer = Defer::new(|| SCOPE.drop());
			SCOPE.init(unsafe { mem::transmute(scope) });
			LUA.set_named_registry_value("cx", scope.create_any_userdata_ref(cx)?)?;

			let globals = LUA.globals();
			globals.raw_set(
				"cx",
				LUA.create_table_from([
					("active", super::Tab::make(cx.manager.active())?),
					("tabs", super::Tabs::make(&cx.manager.tabs)?),
					("tasks", super::Tasks::make(&cx.tasks)?),
					("yanked", scope.create_any_userdata_ref(&cx.manager.yanked)?),
				])?,
			)?;

			let ret = f(scope)?;

			LAYOUT.store(Arc::new(yazi_config::Layout {
				header:  *globals.raw_get::<_, Table>("Header")?.raw_get::<_, RectRef>("area")?,
				parent:  *globals.raw_get::<_, Table>("Parent")?.raw_get::<_, RectRef>("area")?,
				current: *globals.raw_get::<_, Table>("Current")?.raw_get::<_, RectRef>("area")?,
				preview: *globals.raw_get::<_, Table>("Preview")?.raw_get::<_, RectRef>("area")?,
				status:  *globals.raw_get::<_, Table>("Status")?.raw_get::<_, RectRef>("area")?,
			}));

			Ok(ret)
		});

		if let Err(ref e) = result {
			error!("{e}");
		}
		result
	}
}
