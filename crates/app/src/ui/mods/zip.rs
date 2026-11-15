use super::Mods;
use crate::ui::notices;
use crate::words::put;
use gpui::{Context, PathPromptOptions, Window};
use rimstack_core::say;
use std::path::PathBuf;

impl Mods {
    /// A .zip chosen in the system's dialog.
    pub(super) fn pick_zip(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(put(&say!("Install from zip…")).into()),
        });
        cx.spawn_in(window, async move |mods, cx| {
            let Ok(Ok(Some(paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = mods.update(cx, |mods, cx| mods.install_zip(path, cx));
        })
        .detach();
    }

    /// Unpacked into the game's Mods folder, then the mods are read again.
    pub(super) fn install_zip(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        // A folder or another archive: the unpacker would say no, at the cost of opening it.
        let zipped = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"));
        if !zipped {
            let said = say!("{} is not a .zip", path.display().to_string());
            notices::say(put(&said), notices::Tone::Err, cx);
            return;
        }
        let app = crate::ui::app(cx);
        crate::jobs::run(
            "install a mod from a zip",
            cx,
            move || {
                let landed = app.install_zip(&path);
                if landed.is_ok() {
                    app.rescan();
                }
                landed
            },
            |_, landed, cx| match landed {
                Ok(folder) => {
                    let name = folder
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let said = say!("{} is now with the local mods", name);
                    notices::say(put(&said), notices::Tone::Ok, cx);
                }
                Err(problem) => {
                    notices::say(crate::words::said_of(&problem), notices::Tone::Err, cx)
                }
            },
        );
    }
}
