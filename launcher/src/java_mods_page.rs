//! The selected instance's Java mods on Home (`horizon_launcher::java_mods`): the Fabric jars
//! in its `mods-java/`, which of them failed in the last game, Add (or drop jars on the tab),
//! Open folder and Remove (to the trash, after a confirmation). The tab shows only for an
//! instance that has a `mods-java/` folder: Java mods are opt-in (a game built with them).
use super::*;
use gpui::ExternalPaths;
use horizon_launcher::java_mods::{self, Failure, JavaMod};
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct JavaModsTab {
    home: Option<PathBuf>,
    list: Vec<JavaMod>,
    failures: Vec<Failure>,
    fresh: bool,
    /// The jar whose Remove waits for its confirmation, by file name.
    confirm_remove: Option<String>,
    /// What the last action did, or why it failed.
    note: Option<Result<String, String>>,
}

impl JavaModsTab {
    /// Read again on the next frame: a game may have run (its log) or the folder changed.
    pub(super) fn stale(&mut self) {
        self.fresh = false;
    }
}

impl Launcher {
    fn load_java_mods(&mut self, home: PathBuf) {
        if self.java_mods.home.as_ref() != Some(&home) {
            self.java_mods = JavaModsTab {
                home: Some(home.clone()),
                ..JavaModsTab::default()
            };
        }
        if !self.java_mods.fresh {
            self.java_mods.list = java_mods::list(&home);
            self.java_mods.failures = java_mods::failures(&home);
            self.java_mods.fresh = true;
        }
    }

    /// Whether the selected instance has a `mods-java/` folder (the tab shows).
    pub(super) fn has_java_mods_tab(&self) -> bool {
        self.selected_instance()
            .is_some_and(|i| java_mods::dir(&i.home).is_dir())
    }

    /// How many Java mods the selected instance has.
    pub(super) fn java_mods_count(&mut self) -> usize {
        let Some(home) = self.selected_instance().map(|i| i.home.clone()) else {
            return 0;
        };
        self.load_java_mods(home);
        self.java_mods.list.len()
    }

    fn add_java_mods(&mut self, home: PathBuf, paths: Vec<PathBuf>) {
        let mut added = Vec::new();
        let mut failed = Vec::new();
        for path in paths {
            match java_mods::add(&home, &path) {
                Ok(m) => added.push(m.title().to_string()),
                Err(e) => failed.push(e),
            }
        }
        self.java_mods.note = Some(if failed.is_empty() {
            Ok(trf(
                "Added {names}: they load the next time the game starts.",
                &[("names", &added.join(", "))],
            ))
        } else {
            Err(failed.join("\n"))
        });
        self.java_mods.stale();
    }

    fn choose_java_mods(&mut self, home: PathBuf, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(tr("Add").into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await {
                let _ = this.update(cx, |this, cx| {
                    this.add_java_mods(home, paths);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn remove_java_mod(&mut self, home: PathBuf, file: String) {
        self.java_mods.confirm_remove = None;
        self.java_mods.note = Some(
            java_mods::remove(&home, &file)
                .map(|m| trf("{name} moved to the trash.", &[("name", m.title())])),
        );
        self.java_mods.stale();
    }

    /// Home's Java mods tab: its buttons (beside the tabs) and its list; None without an
    /// instance.
    pub(super) fn java_mods_section(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<(Div, Stateful<Div>)> {
        let t = self.theme;
        let home = self.selected_instance()?.home.clone();
        self.load_java_mods(home.clone());
        let folder = java_mods::dir(&home);
        let pick = home.clone();
        let buttons = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(
                button("java-mods-add", "Add", Kind::Ghost, &t).on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.choose_java_mods(pick.clone(), cx);
                    },
                )),
            )
            .child(
                button("java-mods-folder", "Open folder", Kind::Ghost, &t)
                    .on_click(move |_, _, cx| cx.open_with_system(&folder)),
            )
            .child(
                button("java-mods-refresh", "Refresh", Kind::Ghost, &t).on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.java_mods.stale();
                        cx.notify();
                    },
                )),
            );
        let mut section = div().flex().flex_col().gap(px(8.));
        match &self.java_mods.note {
            Some(Ok(m)) => {
                section = section.child(text(theme::CAPTION).text_color(t.muted).child(m.clone()))
            }
            Some(Err(e)) => section = section.child(notice(e.clone(), &t)),
            None => {}
        }
        // Failures not tied to one mod (Java or Fabric didn't start) come first.
        for failure in self.java_mods.failures.iter().filter(|f| f.name.is_none()) {
            section = section.child(notice(failure.text.clone(), &t));
        }
        if self.java_mods.list.is_empty() {
            section = section.child(text(theme::CAPTION).text_color(t.muted).child(tr(
                "No Java mods yet: add Fabric mods' jars, or drop them here.",
            )));
        }
        let rows = self
            .java_mods
            .list
            .clone()
            .into_iter()
            .enumerate()
            .map(|(i, m)| {
                let failed: Vec<&Failure> = self
                    .java_mods
                    .failures
                    .iter()
                    .filter(|f| f.name.as_deref() == Some(m.title()))
                    .collect();
                let detail = match &m.version {
                    Some(v) => format!("{v} · {}", m.file),
                    None => m.file.clone(),
                };
                let mut info = div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        blocks::pixel(15., FontWeight::MEDIUM)
                            .text_color(t.ink)
                            .truncate()
                            .child(m.title().to_string()),
                    )
                    .child(
                        text(theme::CAPTION)
                            .text_color(t.muted)
                            .truncate()
                            .child(detail),
                    );
                for f in &failed {
                    info = info.child(
                        text(theme::CAPTION)
                            .text_color(t.danger)
                            .child(f.text.clone()),
                    );
                }
                let mut actions = div().flex().items_center().gap(px(6.));
                if self.java_mods.confirm_remove.as_deref() == Some(m.file.as_str()) {
                    let (home, file) = (home.clone(), m.file.clone());
                    actions = actions
                        .child(
                            button(
                                SharedString::from(format!("java-mod-keep-{i}")),
                                "Keep",
                                Kind::Ghost,
                                &t,
                            )
                            .on_click(cx.listener(
                                |this, _: &ClickEvent, _, cx| {
                                    this.java_mods.confirm_remove = None;
                                    cx.notify();
                                },
                            )),
                        )
                        .child(
                            button(
                                SharedString::from(format!("java-mod-trash-{i}")),
                                "Move to trash",
                                Kind::Secondary,
                                &t,
                            )
                            .on_click(cx.listener(
                                move |this, _: &ClickEvent, _, cx| {
                                    this.remove_java_mod(home.clone(), file.clone());
                                    cx.notify();
                                },
                            )),
                        );
                } else {
                    let file = m.file.clone();
                    actions = actions.child(
                        button(
                            SharedString::from(format!("java-mod-remove-{i}")),
                            "Remove",
                            Kind::Ghost,
                            &t,
                        )
                        .on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                this.java_mods.confirm_remove = Some(file.clone());
                                cx.notify();
                            },
                        )),
                    );
                }
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .px(px(10.))
                    .py(px(6.))
                    .border_2()
                    .border_color(if failed.is_empty() {
                        t.outline
                    } else {
                        t.danger
                    })
                    .bg(t.well)
                    .child(info)
                    .child(actions)
            });
        let drop_home = home.clone();
        Some((
            buttons,
            div()
                .id("java-mods-drop")
                .border_2()
                .border_color(gpui::transparent_black())
                .drag_over::<ExternalPaths>(move |s, _, _, _| s.border_color(t.accent))
                .on_drop(cx.listener(move |this, paths: &ExternalPaths, _, cx| {
                    this.add_java_mods(drop_home.clone(), paths.paths().to_vec());
                    // Not the window's import as well.
                    cx.stop_propagation();
                    cx.notify();
                }))
                .child(section.children(rows)),
        ))
    }
}
