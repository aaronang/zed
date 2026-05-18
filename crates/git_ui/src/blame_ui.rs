use crate::{
    commit_tooltip::{CommitAvatar, CommitTooltip},
    commit_view::CommitView,
};
use editor::{BlameRenderer, Editor};
use git::{blame::BlameEntry, commit::ParsedCommitMessage};
use gpui::{AnyView, ClipboardItem, Entity, Hsla, MouseButton, Subscription, TextStyle, WeakEntity, prelude::*};
use project::{git_store::Repository, project_settings::ProjectSettings};
use settings::Settings as _;
use ui::{ContextMenu, prelude::*};
use workspace::Workspace;

const GIT_BLAME_MAX_AUTHOR_CHARS_DISPLAYED: usize = 20;

pub struct GitBlameRenderer;

impl BlameRenderer for GitBlameRenderer {
    fn max_author_length(&self) -> usize {
        GIT_BLAME_MAX_AUTHOR_CHARS_DISPLAYED
    }

    fn render_blame_entry(
        &self,
        style: &TextStyle,
        blame_entry: BlameEntry,
        details: Option<ParsedCommitMessage>,
        repository: Entity<Repository>,
        workspace: WeakEntity<Workspace>,
        editor: Entity<Editor>,
        ix: usize,
        sha_color: Hsla,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let relative_timestamp = blame_entry_relative_timestamp(&blame_entry);
        let short_commit_id = blame_entry.sha.display_short();
        let author_name = blame_entry.author.as_deref().unwrap_or("<no name>");
        let name = util::truncate_and_trailoff(author_name, GIT_BLAME_MAX_AUTHOR_CHARS_DISPLAYED);

        let avatar = if ProjectSettings::get_global(cx).git.blame.show_avatar {
            let author_email = blame_entry.author_mail.as_ref().map(|email| {
                SharedString::from(
                    email
                        .trim_start_matches('<')
                        .trim_end_matches('>')
                        .to_string(),
                )
            });
            Some(
                CommitAvatar::new(
                    &blame_entry.sha.to_string().into(),
                    author_email,
                    details.as_ref().and_then(|it| it.remote.as_ref()),
                )
                .render(window, cx),
            )
        } else {
            None
        };

        Some(
            div()
                .mr_2()
                .child(
                    h_flex()
                        .id(("blame", ix))
                        .w_full()
                        .gap_2()
                        .justify_between()
                        .font(style.font())
                        .line_height(style.line_height)
                        .text_color(cx.theme().status().hint)
                        .child(
                            h_flex()
                                .gap_2()
                                .child(div().text_color(sha_color).child(short_commit_id))
                                .children(avatar)
                                .child(name),
                        )
                        .child(relative_timestamp)
                        .hover(|style| style.bg(cx.theme().colors().element_hover))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Right, {
                            let blame_entry = blame_entry.clone();
                            let details = details.clone();
                            let editor = editor.clone();
                            move |event, window, cx| {
                                cx.stop_propagation();

                                deploy_blame_entry_context_menu(
                                    &blame_entry,
                                    details.as_ref(),
                                    editor.clone(),
                                    event.position,
                                    window,
                                    cx,
                                );
                            }
                        })
                        .on_click({
                            let blame_entry = blame_entry.clone();
                            let repository = repository.clone();
                            let workspace = workspace.clone();
                            move |_, window, cx| {
                                CommitView::open(
                                    blame_entry.sha.to_string(),
                                    repository.downgrade(),
                                    workspace.clone(),
                                    None,
                                    None,
                                    window,
                                    cx,
                                )
                            }
                        })
                        .when(!editor.read(cx).has_mouse_context_menu(), |el| {
                            el.hoverable_tooltip(move |_window, cx| {
                                cx.new(|cx| {
                                    CommitTooltip::blame_entry(
                                        &blame_entry,
                                        details.clone(),
                                        repository.clone(),
                                        workspace.clone(),
                                        cx,
                                    )
                                })
                                .into()
                            })
                        }),
                )
                .into_any(),
        )
    }

    fn render_inline_blame_entry(
        &self,
        style: &TextStyle,
        blame_entry: BlameEntry,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let relative_timestamp = blame_entry_relative_timestamp(&blame_entry);
        let author = blame_entry.author.as_deref().unwrap_or_default();
        let summary_enabled = ProjectSettings::get_global(cx)
            .git
            .inline_blame
            .show_commit_summary;

        let text = match blame_entry.summary.as_ref() {
            Some(summary) if summary_enabled => {
                format!("{}, {} - {}", author, relative_timestamp, summary)
            }
            _ => format!("{}, {}", author, relative_timestamp),
        };

        Some(
            h_flex()
                .id("inline-blame")
                .w_full()
                .font(style.font())
                .text_color(cx.theme().status().hint)
                .line_height(style.line_height)
                .child(Icon::new(IconName::FileGit).color(Color::Hint))
                .child(text)
                .gap_2()
                .into_any(),
        )
    }

    fn create_blame_popover(
        &self,
        blame: BlameEntry,
        details: Option<ParsedCommitMessage>,
        repository: Entity<Repository>,
        workspace: WeakEntity<Workspace>,
        cx: &mut App,
    ) -> Option<AnyView> {
        Some(
            cx.new(|cx| CommitTooltip::blame_entry(&blame, details, repository, workspace, cx))
                .into(),
        )
    }

    fn open_blame_commit(
        &self,
        blame_entry: BlameEntry,
        repository: Entity<Repository>,
        workspace: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut App,
    ) {
        CommitView::open(
            blame_entry.sha.to_string(),
            repository.downgrade(),
            workspace,
            None,
            None,
            window,
            cx,
        )
    }
}

fn deploy_blame_entry_context_menu(
    blame_entry: &BlameEntry,
    details: Option<&ParsedCommitMessage>,
    editor: Entity<Editor>,
    position: gpui::Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let context_menu = ContextMenu::build(window, cx, move |menu, _, _| {
        let sha = format!("{}", blame_entry.sha);
        menu.on_blur_subscription(Subscription::new(|| {}))
            .entry("Copy Commit SHA", None, move |_, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(sha.clone()));
            })
            .when_some(
                details.and_then(|details| details.permalink.clone()),
                |this, url| {
                    this.entry("Open Permalink", None, move |_, cx| {
                        cx.open_url(url.as_str())
                    })
                },
            )
    });

    editor.update(cx, move |editor, cx| {
        editor.hide_blame_popover(false, cx);
        editor.deploy_mouse_context_menu(position, context_menu, window, cx);
        cx.notify();
    });
}

fn blame_entry_relative_timestamp(blame_entry: &BlameEntry) -> String {
    match blame_entry.author_offset_date_time() {
        Ok(timestamp) => {
            let local_offset =
                time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
            time_format::format_localized_timestamp(
                timestamp,
                time::OffsetDateTime::now_utc(),
                local_offset,
                time_format::TimestampFormat::Relative,
            )
        }
        Err(_) => "Error parsing date".to_string(),
    }
}
