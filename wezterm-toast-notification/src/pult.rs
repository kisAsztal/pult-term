//! pult: notifications with buttons, identified by a `tag`. There is at most one notification per tag: a new one
//! replaces the old one, `close` removes it. On a click the callback receives the id of the button (`default` for a
//! click on the body) and, if the server sends one, the xdg-activation token that can raise the window on Wayland.

use std::time::Duration;

pub struct ActionNotification {
    pub tag: String,
    pub title: String,
    pub message: String,
    /// (id, label); `default` is a click on the body and is not shown as a button
    pub actions: Vec<(String, String)>,
    pub timeout: Option<Duration>,
}

pub type ActionCallback = Box<dyn FnOnce(String, Option<String>) + Send>;

pub use imp::{close, show};

#[cfg(all(not(target_os = "macos"), not(windows)))]
mod imp {
    use super::*;
    use crate::dbus::NotificationsProxy;
    use futures_util::future::{AbortHandle, Abortable};
    use futures_util::stream::StreamExt;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use zvariant::Value;

    /// tag → (notification id, handle that stops the listener)
    static LIVE: Mutex<Option<HashMap<String, (u32, AbortHandle)>>> = Mutex::new(None);

    fn with_live<R>(f: impl FnOnce(&mut HashMap<String, (u32, AbortHandle)>) -> R) -> R {
        let mut live = LIVE.lock().unwrap_or_else(|e| e.into_inner());
        f(live.get_or_insert_with(HashMap::new))
    }

    fn take(tag: &str) -> Option<(u32, AbortHandle)> {
        with_live(|map| map.remove(tag))
    }

    fn forget(tag: &str, nid: u32) {
        with_live(|map| {
            if map.get(tag).map(|(n, _)| *n) == Some(nid) {
                map.remove(tag);
            }
        });
    }

    pub fn show(n: ActionNotification, on_action: ActionCallback) {
        std::thread::spawn(move || {
            if let Err(err) = async_io::block_on(run(n, on_action)) {
                log::error!("pult notification: {:#}", err);
            }
        });
    }

    pub fn close(tag: String) {
        let Some((nid, abort)) = take(&tag) else { return };
        abort.abort();
        std::thread::spawn(move || {
            let res = async_io::block_on(async move {
                let connection = zbus::ConnectionBuilder::session()?.build().await?;
                let proxy = NotificationsProxy::new(&connection).await?;
                proxy.close_notification(nid).await
            });
            if let Err(err) = res {
                log::error!("pult notification close: {:#}", err);
            }
        });
    }

    async fn run(n: ActionNotification, on_action: ActionCallback) -> anyhow::Result<()> {
        let connection = zbus::ConnectionBuilder::session()?.build().await?;
        let proxy = NotificationsProxy::new(&connection).await?;
        // subscribe to the signals before showing the notification, so none can be missed
        let mut invoked = proxy.receive_action_invoked().await?.fuse();
        let mut closed = proxy.receive_notification_closed().await?.fuse();
        let mut tokens = proxy.receive_activation_token().await?.fuse();

        let previous = take(&n.tag);
        if let Some((_, abort)) = &previous {
            abort.abort();
        }
        let actions: Vec<&str> = n.actions.iter().flat_map(|(k, l)| [k.as_str(), l.as_str()]).collect();
        let mut hints = HashMap::new();
        hints.insert("desktop-entry", Value::from("pult-term"));
        hints.insert("urgency", Value::U8(1));
        let nid = proxy
            .notify(
                "pult-term",
                previous.map(|(nid, _)| nid).unwrap_or(0),
                "pult-term",
                &n.title,
                &n.message,
                &actions,
                &hints,
                n.timeout.map(|d| d.as_millis() as i32).unwrap_or(0),
            )
            .await?;

        let (abort, reg) = AbortHandle::new_pair();
        with_live(|map| map.insert(n.tag.clone(), (nid, abort)));

        let tag = n.tag.clone();
        let wait = async move {
            let mut token = None;
            loop {
                futures_util::select! {
                    sig = tokens.next() => if let Some(sig) = sig {
                        let a = sig.args()?;
                        if a.nid == nid { token = Some(a.token.to_string()); }
                    },
                    sig = invoked.next() => if let Some(sig) = sig {
                        let a = sig.args()?;
                        if a.nid == nid {
                            forget(&tag, nid);
                            on_action(a.action_key.to_string(), token);
                            return Ok::<(), anyhow::Error>(());
                        }
                    },
                    sig = closed.next() => if let Some(sig) = sig {
                        if sig.args()?.nid == nid {
                            forget(&tag, nid);
                            return Ok(());
                        }
                    },
                }
            }
        };
        match Abortable::new(Box::pin(wait), reg).await {
            Ok(res) => res,
            Err(_aborted) => Ok(()),
        }
    }
}

#[cfg(any(target_os = "macos", windows))]
mod imp {
    use super::*;

    /// No buttons here (yet): a plain notification.
    pub fn show(n: ActionNotification, _on_action: ActionCallback) {
        crate::show(crate::ToastNotification { title: n.title, message: n.message, url: None, timeout: n.timeout });
    }

    pub fn close(_tag: String) {}
}
