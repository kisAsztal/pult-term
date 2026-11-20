//! pult: desktop notifications with buttons, from Lua.
//!
//! ```lua
//! window:pult_notify { tag = 'pane-12', title = '…', message = '…',
//!                      actions = { { 'default', '' }, { 'open', 'Open' } }, timeout = 0 }
//! window:pult_notify_close('pane-12')
//! wezterm.on('pult-notification-action', function(window, tag, action) … end)
//! window:pult_background('/path/to/image.png')   -- swap the background image without a config reload
//! ```
//! On a click the window is raised (on Wayland with the notification's xdg-activation token; not for buttons whose
//! id starts with `bg-`), then the event runs.

use crate::scripting::guiwin::GuiWin;
use crate::termwindow::background::{pult_swap_background_image, reload_background_image};
use crate::termwindow::{TermWindow, TermWindowNotif};
use mlua::UserDataMethods;
use wezterm_toast_notification::pult::{self as notif, ActionNotification};
use window::WindowOps;

pub fn add_lua_methods<'lua, M: UserDataMethods<'lua, GuiWin>>(methods: &mut M) {
    // capability flag: the tab bar animates the spinner and the pulse itself (render/fancy_tab_bar.rs `pult_animate`)
    methods.add_method("pult_tab_anim", |_, _, ()| Ok(true));
    methods.add_method("pult_notify", |_, this, opts: mlua::Table| {
        let tag: String = opts.get("tag")?;
        let actions: Vec<Vec<String>> = opts.get::<_, Option<_>>("actions")?.unwrap_or_default();
        let n = ActionNotification {
            tag,
            title: opts.get::<_, Option<String>>("title")?.unwrap_or_default(),
            message: opts.get::<_, Option<String>>("message")?.unwrap_or_default(),
            actions: actions
                .into_iter()
                .filter_map(|a| Some((a.first()?.clone(), a.get(1).cloned().unwrap_or_default())))
                .collect(),
            timeout: opts
                .get::<_, Option<u64>>("timeout")?
                .filter(|t| *t > 0)
                .map(std::time::Duration::from_millis),
        };
        let window = this.window.clone();
        let tag = n.tag.clone();
        notif::show(
            n,
            Box::new(move |action, token| {
                window.notify(TermWindowNotif::Apply(Box::new(move |tw: &mut TermWindow| {
                    // a `bg-` button (e.g. approve) does not raise the window
                    if let Some(w) = tw.window.as_ref().filter(|_| !action.starts_with("bg-")) {
                        match token {
                            Some(token) => w.activate_with_token(token),
                            None => w.focus(),
                        }
                    }
                    tw.pult_emit_notification_action(tag, action);
                })));
            }),
        );
        Ok(())
    });
    methods.add_method("pult_notify_close", |_, _, tag: String| {
        notif::close(tag);
        Ok(())
    });
    // the image of the first file layer of `background`; nil returns to the configured one
    methods.add_method("pult_background", |_, this, path: Option<String>| {
        this.window.notify(TermWindowNotif::Apply(Box::new(move |tw: &mut TermWindow| {
            if tw.pult_background != path {
                tw.pult_background = path;
                if tw.pult_background.is_none() {
                    tw.window_background = reload_background_image(
                        &tw.config,
                        &tw.window_background,
                        &tw.dimensions,
                        &tw.render_metrics,
                    );
                }
                tw.pult_apply_background();
                if let Some(w) = tw.window.as_ref() {
                    w.invalidate();
                }
            }
        })));
        Ok(())
    });
}

impl TermWindow {
    pub(super) fn pult_apply_background(&mut self) {
        if let Some(path) = &self.pult_background {
            pult_swap_background_image(&mut self.window_background, path);
        }
    }

    fn pult_emit_notification_action(&mut self, tag: String, action: String) {
        let window = GuiWin::new(self);
        promise::spawn::spawn(config::with_lua_config_on_main_thread(move |lua| async move {
            if let Some(lua) = lua {
                let args = lua.pack_multi((window, tag, action))?;
                if let Err(err) =
                    config::lua::emit_event(&lua, ("pult-notification-action".to_string(), args)).await
                {
                    log::error!("while processing pult-notification-action event: {:#}", err);
                }
            }
            Ok(())
        }))
        .detach();
    }
}
