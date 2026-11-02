use super::app::{self, App, id_of};
use super::{alert, confirm, copy_text, error_box, open_browser, prompt, styled_dialog};
use crate::api::{collection_from_response, collections_from_response, flatten_profile};
use crate::render::{build_status_body_html, escape_html, get_quoted_status, links_in_status, status_plain_text, strip_html};
use crate::rows::{display_name, source_status};
use crate::sounds;
use crate::theme;
use crate::timeutil::{clock_time, long_date};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;
use wxdragon::event::{WebViewEventData, WebViewEvents};
use wxdragon::prelude::*;
use wxdragon::sizers::staticbox_sizer::StaticBoxSizerBuilder;
use wxdragon::widgets::WebView;

pub fn settings(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let (dialog, panel, sizer) = styled_dialog(&frame, "Settings", 400, 200);
    let label = StaticText::builder(&panel).with_label("Select Sound Pack:").build();
    let packs = crate::config::sound_packs();
    let current = crate::config::sound_pack();
    let selection = packs.iter().position(|pack| pack == &current).unwrap_or(0) as u32;
    let choice = Choice::builder(&panel)
        .with_choices(packs.clone())
        .with_selection(Some(selection))
        .build();
    let save = Button::builder(&panel).with_label("&Save").build();
    let cancel = Button::builder(&panel).with_label("&Cancel").with_id(ID_CANCEL).build();
    theme::paint(&label);
    theme::paint(&choice);
    theme::paint(&save);
    theme::paint(&cancel);
    sizer.add(&label, 0, SizerFlag::All | SizerFlag::Expand, 5);
    sizer.add(&choice, 0, SizerFlag::All | SizerFlag::Expand, 5);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&save, 0, SizerFlag::All, 5);
    buttons.add(&cancel, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    let dialog_save = dialog;
    save.on_click(move |_| {
        let selected = choice.get_string_selection().unwrap_or_else(|| "default".to_string());
        if let Err(error) = crate::config::set_sound_pack(&selected) {
            error_box(&dialog_save, &error, "Settings");
            return;
        }
        alert(&dialog_save, "Settings saved. Sound changes will take effect on next restart or action.", "Settings Saved");
        dialog_save.end_modal(ID_OK);
    });
    dialog.show_modal();
    dialog.destroy();
}

pub fn notification_policy(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let policy = match app.borrow().api.notification_policy() {
        Ok(policy) => policy,
        Err(error) => {
            error_box(&frame, &format!("Error loading notification filters: {error}"), "Notification Filters");
            return;
        }
    };
    let fields = [
        ("for_not_following", "People you do not follow"),
        ("for_not_followers", "People who do not follow you"),
        ("for_new_accounts", "New accounts"),
        ("for_private_mentions", "Private mentions"),
        ("for_limited_accounts", "Limited accounts"),
        ("for_bots", "Bot accounts"),
    ];
    let (dialog, panel, sizer) = styled_dialog(&frame, "Notification Filters", 520, 430);
    if policy.get("summary").is_some() {
        let pending = policy.pointer("/summary/pending_notifications_count").and_then(Value::as_i64).unwrap_or(0);
        let requests = policy.pointer("/summary/pending_requests_count").and_then(Value::as_i64).unwrap_or(0);
        let summary = StaticText::builder(&panel)
            .with_label(&format!("Filtered notifications: {pending}; notification requests: {requests}"))
            .build();
        theme::paint(&summary);
        sizer.add(&summary, 0, SizerFlag::All, 10);
    }
    let grid = FlexGridSizer::builder(0, 2).with_vgap(8).with_hgap(8).build();
    grid.add_growable_col(1, 1);
    let mut choices = Vec::new();
    let options = ["Accept", "Filter", "Drop"];
    let values = ["accept", "filter", "drop"];
    for (key, label) in fields {
        if key == "for_bots" && policy.get(key).is_none() {
            continue;
        }
        let current = policy.get(key).and_then(Value::as_str).unwrap_or("accept");
        let index = values.iter().position(|item| *item == current).unwrap_or(0) as u32;
        let label = StaticText::builder(&panel).with_label(&format!("{label}:")).build();
        let choice = Choice::builder(&panel)
            .with_choices(options.iter().map(|item| (*item).to_string()).collect())
            .with_selection(Some(index))
            .build();
        theme::paint(&label);
        theme::paint(&choice);
        grid.add(&label, 0, SizerFlag::AlignCenterVertical, 0);
        grid.add(&choice, 1, SizerFlag::Expand, 0);
        choices.push((key, choice));
    }
    sizer.add_sizer(&grid, 1, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 10);
    let save = Button::builder(&panel).with_label("&OK").with_id(ID_OK).build();
    let cancel = Button::builder(&panel).with_label("&Cancel").with_id(ID_CANCEL).build();
    theme::paint(&save);
    theme::paint(&cancel);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&save, 0, SizerFlag::All, 5);
    buttons.add(&cancel, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 10);
    panel.set_sizer(sizer, true);
    let dialog_save = dialog;
    save.on_click(move |_| dialog_save.end_modal(ID_OK));
    if dialog.show_modal() == ID_OK {
        let mut updated = json!({});
        for (key, choice) in &choices {
            let index = choice.get_selection().unwrap_or(0) as usize;
            updated[key] = json!(values[index]);
        }
        match app.borrow().api.update_notification_policy(&updated) {
            Ok(_) => alert(&frame, "Notification filters saved.", "Notification Filters"),
            Err(error) => error_box(&frame, &format!("Error saving notification filters: {error}"), "Notification Filters"),
        }
    }
    dialog.destroy();
}

pub fn find(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let key = {
        let state = app.borrow();
        let Some(key) = app::selected_key(&state) else {
            alert(&frame, "No timeline selected or timeline is empty.", "Find");
            return;
        };
        if state.timelines.get(&key).is_none_or(|items| items.is_empty()) {
            alert(&frame, "No timeline selected or timeline is empty.", "Find");
            return;
        }
        key
    };
    let Some(query) = prompt(&frame, "Search text:", "Find in Timeline") else {
        return;
    };
    let query = query.to_lowercase();
    let state = app.borrow();
    let items = state.timelines.get(&key).cloned().unwrap_or_default();
    drop(state);
    let mut results = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let status = if key == "notifications" {
            item.get("status").cloned().unwrap_or(Value::Null)
        } else {
            item.clone()
        };
        if status.is_null() {
            continue;
        }
        let source = source_status(&status);
        let content = status_plain_text(&source).to_lowercase();
        let author = display_name(source.get("account").unwrap_or(&Value::Null)).to_lowercase();
        let spoiler = source.get("spoiler_text").and_then(Value::as_str).unwrap_or("").to_lowercase();
        if content.contains(&query) || author.contains(&query) || spoiler.contains(&query) {
            results.push((index, status));
        }
    }
    if results.is_empty() {
        alert(&frame, &format!("No posts found matching '{query}'."), "Find");
        return;
    }
    if results.len() == 1 {
        focus_row(app, results[0].0);
        return;
    }
    let (dialog, panel, sizer) = styled_dialog(&frame, &format!("Find Results ({} matches)", results.len()), 600, 400);
    let list = ListBox::builder(&panel).with_size(Size::new(-1, 300)).build();
    for (_, status) in &results {
        let source = source_status(status);
        let author = display_name(source.get("account").unwrap_or(&Value::Null));
        let content = status_plain_text(&source).chars().take(120).collect::<String>();
        list.append(&format!("{author}: {content}"));
    }
    theme::paint(&list);
    let go = Button::builder(&panel).with_label("&Go to Post").build();
    let close = Button::builder(&panel).with_label("&Close").with_id(ID_CANCEL).build();
    theme::paint(&go);
    theme::paint(&close);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 10);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&go, 0, SizerFlag::All, 5);
    buttons.add(&close, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    let dialog_go = dialog;
    let app_go = app.clone();
    let results_go = results.clone();
    go.on_click(move |_| {
        if let Some(index) = list.get_selection() {
            focus_row(&app_go, results_go[index as usize].0);
            dialog_go.end_modal(ID_OK);
        }
    });
    list.on_item_double_clicked({
        let app_go = app.clone();
        let dialog_go = dialog;
        move |_| {
            if let Some(index) = list.get_selection() {
                focus_row(&app_go, results[index as usize].0);
                dialog_go.end_modal(ID_OK);
            }
        }
    });
    dialog.show_modal();
    dialog.destroy();
}

fn focus_row(app: &Rc<RefCell<App>>, index: usize) {
    let posts = app.borrow().posts;
    posts.set_item_state(index as i64, ListItemState::Selected, ListItemState::Selected);
    posts.ensure_visible(index as i64);
    posts.set_focus();
}

pub fn scheduled(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let (dialog, panel, sizer) = styled_dialog(&frame, "Scheduled Posts", 600, 400);
    let list = ListBox::builder(&panel).build();
    let items = match app.borrow().api.call("GET", "/api/v1/scheduled_statuses", vec![("limit".to_string(), "40".to_string())]) {
        Ok(value) => crate::api::as_array(value),
        Err(error) => {
            error_box(&frame, &format!("Error loading scheduled posts: {error}"), "Error");
            Vec::new()
        }
    };
    for item in &items {
        list.append(&scheduled_label(item));
    }
    theme::paint(&list);
    let edit = Button::builder(&panel).with_label("&Edit Time").build();
    let delete = Button::builder(&panel).with_label("&Delete").build();
    let close = Button::builder(&panel).with_label("&Close").with_id(ID_CANCEL).build();
    theme::paint(&edit);
    theme::paint(&delete);
    theme::paint(&close);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 10);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&edit, 0, SizerFlag::All, 5);
    buttons.add(&delete, 0, SizerFlag::All, 5);
    buttons.add(&close, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    let items = Rc::new(RefCell::new(items));
    let api = app.borrow().api.clone();
    let dialog_edit = dialog;
    let items_edit = items.clone();
    edit.on_click(move |_| {
        let Some(index) = list.get_selection() else { return };
        let Some(when) = prompt(&dialog_edit, "New scheduled time (YYYY-MM-DD HH:MM):", "Reschedule") else { return };
        let mut items = items_edit.borrow_mut();
        let Some(id) = items.get(index as usize).map(id_of) else { return };
        let stamp = when.trim().replace(' ', "T") + ":00Z";
        match api.call("PUT", &format!("/api/v1/scheduled_statuses/{id}"), vec![("scheduled_at".to_string(), stamp.clone())]) {
            Ok(_) => {
                if let Some(item) = items.get_mut(index as usize) {
                    item["scheduled_at"] = json!(stamp);
                    rebuild_list(&list, &items.iter().map(scheduled_label).collect::<Vec<_>>());
                    list.set_selection(index, true);
                }
            }
            Err(error) => error_box(&dialog_edit, &format!("Error: {error}"), "Error"),
        }
    });
    let api = app.borrow().api.clone();
    delete.on_click(move |_| {
        let Some(index) = list.get_selection() else { return };
        if !confirm(&dialog, "Delete this scheduled post?", "Confirm") {
            return;
        }
        let mut items = items.borrow_mut();
        let Some(id) = items.get(index as usize).map(id_of) else { return };
        match api.call("DELETE", &format!("/api/v1/scheduled_statuses/{id}"), Vec::new()) {
            Ok(_) => {
                items.remove(index as usize);
                rebuild_list(&list, &items.iter().map(scheduled_label).collect::<Vec<_>>());
            }
            Err(error) => error_box(&dialog, &format!("Error: {error}"), "Error"),
        }
    });
    dialog.show_modal();
    dialog.destroy();
}

fn scheduled_label(item: &Value) -> String {
    let when = item.get("scheduled_at").and_then(Value::as_str).unwrap_or("").replace('T', " ").trim_end_matches('Z').trim_end_matches(":00").to_string();
    let text = item.pointer("/params/text").and_then(Value::as_str).unwrap_or("");
    let text = text.chars().take(100).collect::<String>();
    format!("[{when}] {text}")
}

fn rebuild_list(list: &ListBox, labels: &[String]) {
    list.clear();
    for label in labels {
        list.append(label);
    }
}

pub fn explore(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let (dialog, panel, sizer) = styled_dialog(&frame, "Explore / Discover", 700, 500);
    let notebook = Notebook::builder(&panel).build();
    let posts_page = Panel::builder(&notebook).build();
    let tags_page = Panel::builder(&notebook).build();
    let links_page = Panel::builder(&notebook).build();
    theme::paint(&posts_page);
    theme::paint(&tags_page);
    theme::paint(&links_page);
    theme::paint(&notebook);
    let posts = ListBox::builder(&posts_page).build();
    let tags = ListBox::builder(&tags_page).build();
    let links = ListBox::builder(&links_page).build();
    let open_tag = Button::builder(&tags_page).with_label("&Open Hashtag Timeline").build();
    let open_link = Button::builder(&links_page).with_label("&Open Link in Browser").build();
    theme::paint(&posts);
    theme::paint(&tags);
    theme::paint(&links);
    theme::paint(&open_tag);
    theme::paint(&open_link);
    let posts_sizer = BoxSizer::builder(Orientation::Vertical).build();
    posts_sizer.add(&posts, 1, SizerFlag::Expand | SizerFlag::All, 5);
    posts_page.set_sizer(posts_sizer, true);
    let tags_sizer = BoxSizer::builder(Orientation::Vertical).build();
    tags_sizer.add(&tags, 1, SizerFlag::Expand | SizerFlag::All, 5);
    tags_sizer.add(&open_tag, 0, SizerFlag::All, 5);
    tags_page.set_sizer(tags_sizer, true);
    let links_sizer = BoxSizer::builder(Orientation::Vertical).build();
    links_sizer.add(&links, 1, SizerFlag::Expand | SizerFlag::All, 5);
    links_sizer.add(&open_link, 0, SizerFlag::All, 5);
    links_page.set_sizer(links_sizer, true);
    notebook.add_page(&posts_page, "Trending Posts", true, None);
    notebook.add_page(&tags_page, "Trending Hashtags", false, None);
    notebook.add_page(&links_page, "Trending Links", false, None);
    let api = app.borrow().api.clone();
    let trending_posts = api.call("GET", "/api/v1/trends/statuses", vec![("limit".to_string(), "20".to_string())]).map(crate::api::as_array).unwrap_or_default();
    for post in &trending_posts {
        let author = display_name(post.get("account").unwrap_or(&Value::Null));
        let content = status_plain_text(post).chars().take(150).collect::<String>();
        posts.append(&format!("{author}: {content}"));
    }
    let trending_tags = api.call("GET", "/api/v1/trends/tags", vec![("limit".to_string(), "20".to_string())]).map(crate::api::as_array).unwrap_or_default();
    for tag in &trending_tags {
        let uses = tag.get("history").and_then(Value::as_array).and_then(|history| history.first()).and_then(|item| item.get("uses")).and_then(Value::as_str).unwrap_or("0");
        tags.append(&format!("#{} ({uses} recent uses)", tag.get("name").and_then(Value::as_str).unwrap_or("")));
    }
    let trending_links = api.call("GET", "/api/v1/trends/links", vec![("limit".to_string(), "20".to_string())]).map(crate::api::as_array).unwrap_or_default();
    for link in &trending_links {
        links.append(&format!("{} - {}", link.get("title").and_then(Value::as_str).unwrap_or("Untitled"), link.get("url").and_then(Value::as_str).unwrap_or("")));
    }
    let close = Button::builder(&panel).with_label("&Close").with_id(ID_CANCEL).build();
    theme::paint(&close);
    sizer.add(&notebook, 1, SizerFlag::Expand | SizerFlag::All, 5);
    sizer.add(&close, 0, SizerFlag::AlignRight | SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    let app_posts = app.clone();
    let dialog_posts = dialog;
    posts.on_item_double_clicked(move |_| {
        if let Some(index) = posts.get_selection() {
            post_details_for(&app_posts, &dialog_posts, trending_posts[index as usize].clone());
        }
    });
    let app_tags = app.clone();
    let dialog_tags = dialog;
    let open_selected_tag = Rc::new(move || {
        if let Some(index) = tags.get_selection() {
            let name = trending_tags[index as usize].get("name").and_then(Value::as_str).unwrap_or("");
            let key = format!("hashtag:{name}");
            app::ensure_timeline(&app_tags, &key, &format!("#{name}"));
            app::begin_load(&app_tags, &key);
            dialog_tags.end_modal(ID_CANCEL);
        }
    });
    let open_tag_action = open_selected_tag.clone();
    open_tag.on_click(move |_| open_tag_action());
    tags.on_item_double_clicked(move |_| open_selected_tag());
    let open_selected_link = Rc::new(move || {
        if let Some(index) = links.get_selection()
            && let Some(url) = trending_links[index as usize].get("url").and_then(Value::as_str)
        {
            open_browser(url);
        }
    });
    let open_link_action = open_selected_link.clone();
    open_link.on_click(move |_| open_link_action());
    links.on_item_double_clicked(move |_| open_selected_link());
    dialog.show_modal();
    dialog.destroy();
}

pub fn lists(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let (dialog, panel, sizer) = styled_dialog(&frame, "Lists", 500, 450);
    let list = ListBox::builder(&panel).build();
    let user_lists = app.borrow().api.call("GET", "/api/v1/lists", Vec::new()).map(crate::api::as_array).unwrap_or_default();
    for item in &user_lists {
        list.append(item.get("title").and_then(Value::as_str).unwrap_or("Untitled"));
    }
    theme::paint(&list);
    let open = Button::builder(&panel).with_label("&Open Timeline").build();
    let create = Button::builder(&panel).with_label("&Create List").build();
    let delete = Button::builder(&panel).with_label("&Delete List").build();
    let manage = Button::builder(&panel).with_label("&Manage Members").build();
    let close = Button::builder(&panel).with_label("C&lose").with_id(ID_CANCEL).build();
    for button in [&open, &create, &delete, &manage, &close] {
        theme::paint(button);
    }
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 10);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    for button in [&open, &create, &delete, &manage, &close] {
        buttons.add(button, 0, SizerFlag::All, 3);
    }
    sizer.add_sizer(&buttons, 0, SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    let api = app.borrow().api.clone();
    let app_open = app.clone();
    let dialog_open = dialog;
    let lists_open = std::sync::Arc::new(std::sync::Mutex::new(user_lists.clone()));
    let stored = lists_open.clone();
    let open_selected = Rc::new(move || {
        let Some(index) = list.get_selection() else { return };
        let lists = stored.lock().ok();
        let Some(lists) = lists else { return };
        let Some(item) = lists.get(index as usize) else { return };
        let key = format!("list:{}", id_of(item));
        let title = item.get("title").and_then(Value::as_str).unwrap_or("List");
        app::ensure_timeline(&app_open, &key, &format!("List: {title}"));
        app::begin_load(&app_open, &key);
        dialog_open.end_modal(ID_CANCEL);
    });
    let open_action = open_selected.clone();
    open.on_click(move |_| open_action());
    list.on_item_double_clicked(move |_| open_selected());
    let dialog_create = dialog;
    let stored = lists_open.clone();
    create.on_click(move |_| {
        let Some(name) = prompt(&dialog_create, "List name:", "Create List") else { return };
        match api.call("POST", "/api/v1/lists", vec![("title".to_string(), name.trim().to_string())]) {
            Ok(created) => {
                list.append(created.get("title").and_then(Value::as_str).unwrap_or("Untitled"));
                if let Ok(mut lists) = stored.lock() {
                    lists.push(created);
                }
            }
            Err(error) => error_box(&dialog_create, &format!("Error: {error}"), "Error"),
        }
    });
    let api = app.borrow().api.clone();
    let stored = lists_open.clone();
    delete.on_click(move |_| {
        let Some(index) = list.get_selection() else { return };
        let title = stored.lock().ok().and_then(|lists| lists.get(index as usize).and_then(|item| item.get("title").and_then(Value::as_str).map(|value| value.to_string()))).unwrap_or_default();
        if !confirm(&dialog, &format!("Delete list '{title}'?"), "Confirm") {
            return;
        }
        let id = stored.lock().ok().and_then(|lists| lists.get(index as usize).map(id_of)).unwrap_or_default();
        match api.call("DELETE", &format!("/api/v1/lists/{id}"), Vec::new()) {
            Ok(_) => {
                if let Ok(mut lists) = stored.lock() {
                    lists.remove(index as usize);
                    rebuild_list(&list, &lists.iter().map(|item| item.get("title").and_then(Value::as_str).unwrap_or("Untitled").to_string()).collect::<Vec<_>>());
                }
            }
            Err(error) => error_box(&dialog, &format!("Error: {error}"), "Error"),
        }
    });
    let app_manage = app.clone();
    manage.on_click(move |_| {
        let Some(index) = list.get_selection() else { return };
        let Some(item) = lists_open.lock().ok().and_then(|lists| lists.get(index as usize).cloned()) else { return };
        manage_members(&app_manage, &dialog, &item);
    });
    let _ = user_lists;
    dialog.show_modal();
    dialog.destroy();
}

fn manage_members(app: &Rc<RefCell<App>>, parent: &impl WxWidget, list_value: &Value) {
    let title = list_value.get("title").and_then(Value::as_str).unwrap_or("List");
    let (dialog, panel, sizer) = styled_dialog(parent, &format!("Members of '{title}'"), 500, 400);
    let list = ListBox::builder(&panel).build();
    let id = id_of(list_value);
    let members = app.borrow().api.call("GET", &format!("/api/v1/lists/{id}/accounts"), vec![("limit".to_string(), "80".to_string())]).map(crate::api::as_array).unwrap_or_default();
    for account in &members {
        list.append(&app::account_label_line(account));
    }
    theme::paint(&list);
    let add = Button::builder(&panel).with_label("&Add User").build();
    let remove = Button::builder(&panel).with_label("&Remove User").build();
    let close = Button::builder(&panel).with_label("&Close").with_id(ID_CANCEL).build();
    theme::paint(&add);
    theme::paint(&remove);
    theme::paint(&close);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 10);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&add, 0, SizerFlag::All, 5);
    buttons.add(&remove, 0, SizerFlag::All, 5);
    buttons.add(&close, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    let members = Rc::new(RefCell::new(members));
    let api = app.borrow().api.clone();
    let dialog_add = dialog;
    let list_id = id.clone();
    let members_add = members.clone();
    add.on_click(move |_| {
        let Some(query) = prompt(&dialog_add, "Account to add:", "Add User") else { return };
        let Ok(results) = api.search_v2(query.trim(), &[("type", "accounts".to_string()), ("limit", "10".to_string())]) else { return };
        let accounts = results.get("accounts").cloned().map(crate::api::as_array).unwrap_or_default();
        if accounts.is_empty() {
            alert(&dialog_add, "No users found.", "Search");
            return;
        }
        let account = if accounts.len() == 1 { accounts[0].clone() } else if let Some(chosen) = choose_account(&dialog_add, &accounts) { chosen } else { return };
        let account_id = id_of(&account);
        if let Err(error) = api.call("POST", &format!("/api/v1/lists/{list_id}/accounts"), vec![("account_ids[]".to_string(), account_id)]) {
            error_box(&dialog_add, &format!("Error: {error}"), "Error");
        } else {
            list.append(&app::account_label_line(&account));
            members_add.borrow_mut().push(account);
        }
    });
    let api = app.borrow().api.clone();
    remove.on_click(move |_| {
        let Some(index) = list.get_selection() else { return };
        let mut members = members.borrow_mut();
        let Some(account) = members.get(index as usize) else { return };
        if let Err(error) = api.call("DELETE", &format!("/api/v1/lists/{id}/accounts"), vec![("account_ids[]".to_string(), id_of(account))]) {
            error_box(&dialog, &format!("Error: {error}"), "Error");
        } else {
            members.remove(index as usize);
            list.delete(index);
        }
    });
    dialog.show_modal();
    dialog.destroy();
}

pub fn collections(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let me = id_of(&app.borrow().me);
    let (dialog, panel, sizer) = styled_dialog(&frame, "Collections", 620, 430);
    let list = ListBox::builder(&panel).build();
    theme::paint(&list);
    let view = Button::builder(&panel).with_label("&View").build();
    let create = Button::builder(&panel).with_label("&Create").build();
    let edit = Button::builder(&panel).with_label("&Edit").build();
    let delete = Button::builder(&panel).with_label("&Delete").build();
    let add_author = Button::builder(&panel).with_label("Add Selected &Author").build();
    let revoke = Button::builder(&panel).with_label("&Revoke Inclusion").build();
    let close = Button::builder(&panel).with_label("C&lose").with_id(ID_CANCEL).build();
    for button in [&view, &create, &edit, &delete, &add_author, &revoke, &close] {
        theme::paint(button);
    }
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 10);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    for button in [&view, &create, &edit, &delete, &add_author, &revoke, &close] {
        buttons.add(button, 0, SizerFlag::All, 3);
    }
    sizer.add_sizer(&buttons, 0, SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    let entries = Rc::new(RefCell::new(Vec::<(String, Value)>::new()));
    refresh_collections(app, &list, &entries, &me);
    let app_view = app.clone();
    let entries_view = entries.clone();
    view.on_click(move |_| {
        if let Some((kind, collection)) = selected_entry(&list, &entries_view) {
            show_collection(&app_view, &collection, kind == "owned");
        }
    });
    let entries_click = entries.clone();
    let app_click = app.clone();
    list.on_item_double_clicked(move |_| {
        if let Some((kind, collection)) = selected_entry(&list, &entries_click) {
            show_collection(&app_click, &collection, kind == "owned");
        }
    });
    let app_create = app.clone();
    let entries_create = entries.clone();
    let me_create = me.clone();
    create.on_click(move |_| {
        if let Some(data) = collection_form(&dialog, "Create Collection", &Value::Null, true)
            && !data.get("name").and_then(Value::as_str).unwrap_or("").is_empty()
        {
            match app_create.borrow().api.call_json("POST", "/api/v1/collections", data) {
                Ok(_) => refresh_collections(&app_create, &list, &entries_create, &me_create),
                Err(error) => error_box(&dialog, &format!("Error creating collection: {error}"), "Collections"),
            }
        }
    });
    let app_edit = app.clone();
    let entries_edit = entries.clone();
    let me_edit = me.clone();
    edit.on_click(move |_| {
        let Some((kind, collection)) = selected_entry(&list, &entries_edit) else { return };
        if kind != "owned" {
            alert(&dialog, "Only your own collections can be edited.", "Collections");
            return;
        }
        if let Some(data) = collection_form(&dialog, "Edit Collection", &collection, false) {
            let path = format!("/api/v1/collections/{}", id_of(&collection));
            match app_edit.borrow().api.call_json("PATCH", &path, data) {
                Ok(_) => refresh_collections(&app_edit, &list, &entries_edit, &me_edit),
                Err(error) => error_box(&dialog, &format!("Error updating collection: {error}"), "Collections"),
            }
        }
    });
    let app_delete = app.clone();
    let entries_delete = entries.clone();
    let me_delete = me.clone();
    delete.on_click(move |_| {
        let Some((kind, collection)) = selected_entry(&list, &entries_delete) else { return };
        if kind != "owned" {
            alert(&dialog, "Only your own collections can be deleted.", "Collections");
            return;
        }
        let name = collection.get("name").and_then(Value::as_str).unwrap_or("Untitled");
        if !confirm(&dialog, &format!("Delete collection '{name}'?"), "Collections") {
            return;
        }
        match app_delete.borrow().api.call("DELETE", &format!("/api/v1/collections/{}", id_of(&collection)), Vec::new()) {
            Ok(_) => refresh_collections(&app_delete, &list, &entries_delete, &me_delete),
            Err(error) => error_box(&dialog, &format!("Error deleting collection: {error}"), "Collections"),
        }
    });
    let app_add = app.clone();
    let entries_add = entries.clone();
    let me_add = me.clone();
    add_author.on_click(move |_| {
        let Some((kind, collection)) = selected_entry(&list, &entries_add) else { return };
        if kind != "owned" {
            alert(&dialog, "Select one of your own collections first.", "Collections");
            return;
        }
        let Some((status, _)) = app::selected_status(&app_add.borrow()) else {
            alert(&dialog, "Select a post from the author you want to add first.", "Collections");
            return;
        };
        let account_id = id_of(source_status(&status).get("account").unwrap_or(&Value::Null));
        match app_add.borrow().api.call_json("POST", &format!("/api/v1/collections/{}/items", id_of(&collection)), json!({"account_id": account_id})) {
            Ok(_) => {
                alert(&dialog, "Author added to the collection.", "Collections");
                refresh_collections(&app_add, &list, &entries_add, &me_add);
            }
            Err(error) => error_box(&dialog, &format!("Error adding author: {error}"), "Collections"),
        }
    });
    let app_revoke = app.clone();
    let entries_revoke = entries;
    revoke.on_click(move |_| {
        let Some((kind, collection)) = selected_entry(&list, &entries_revoke) else { return };
        if kind != "included" {
            alert(&dialog, "Select a collection you are featured in first.", "Collections");
            return;
        }
        let Some(item) = collection.get("items").and_then(Value::as_array).and_then(|items| items.iter().find(|item| item.get("account_id").and_then(Value::as_str) == Some(me.as_str()))) else {
            alert(&dialog, "This collection response did not include your collection item ID.", "Collections");
            return;
        };
        if !confirm(&dialog, "Remove yourself from this collection?", "Collections") {
            return;
        }
        let path = format!("/api/v1/collections/{}/items/{}/revoke", id_of(&collection), id_of(item));
        if let Err(error) = app_revoke.borrow().api.call("POST", &path, Vec::new()) {
            error_box(&dialog, &format!("Error revoking inclusion: {error}"), "Collections");
        } else {
            refresh_collections(&app_revoke, &list, &entries_revoke, &me);
        }
    });
    dialog.show_modal();
    dialog.destroy();
}

fn refresh_collections(app: &Rc<RefCell<App>>, list: &ListBox, entries: &Rc<RefCell<Vec<(String, Value)>>>, me: &str) {
    let api = app.borrow().api.clone();
    let owned = api.call("GET", &format!("/api/v1/accounts/{me}/collections"), vec![("limit".to_string(), "80".to_string())]).map(|value| collections_from_response(&value)).unwrap_or_default();
    let included = api.call("GET", &format!("/api/v1/accounts/{me}/in_collections"), vec![("limit".to_string(), "80".to_string())]).map(|value| collections_from_response(&value)).unwrap_or_default();
    let mut stored = Vec::new();
    list.clear();
    for collection in owned {
        list.append(&collection_label(&collection, "Mine: "));
        stored.push(("owned".to_string(), collection));
    }
    for collection in included {
        if collection.get("account_id").and_then(Value::as_str) == Some(me) {
            continue;
        }
        list.append(&collection_label(&collection, "Featured in: "));
        stored.push(("included".to_string(), collection));
    }
    if !stored.is_empty() {
        list.set_selection(0, true);
    }
    *entries.borrow_mut() = stored;
}

fn selected_entry(list: &ListBox, entries: &Rc<RefCell<Vec<(String, Value)>>>) -> Option<(String, Value)> {
    let index = list.get_selection()? as usize;
    entries.borrow().get(index).cloned()
}

fn collection_label(collection: &Value, prefix: &str) -> String {
    let name = collection.get("name").and_then(Value::as_str).unwrap_or("Untitled collection");
    let count = collection.get("item_count").and_then(Value::as_i64).unwrap_or(0);
    let tag = collection.pointer("/tag/name").and_then(Value::as_str).or_else(|| collection.get("tag_name").and_then(Value::as_str)).unwrap_or("");
    let tag = if tag.is_empty() { String::new() } else { format!(", #{}", tag.trim_start_matches('#')) };
    let sensitive = if collection.get("sensitive").and_then(Value::as_bool).unwrap_or(false) { ", sensitive" } else { "" };
    format!("{prefix}{name} ({count} accounts{tag}{sensitive})")
}

pub fn show_collection(app: &Rc<RefCell<App>>, collection: &Value, can_manage: bool) {
    let frame = app.borrow().frame;
    let loaded = app.borrow().api.call("GET", &format!("/api/v1/collections/{}", id_of(collection)), Vec::new());
    let (accounts, full) = match loaded {
        Ok(value) => (
            value.get("accounts").cloned().map(crate::api::as_array).unwrap_or_default(),
            collection_from_response(value),
        ),
        Err(error) => {
            error_box(&frame, &format!("Error loading collection: {error}"), "Collections");
            return;
        }
    };
    let name = full.get("name").and_then(Value::as_str).unwrap_or("Untitled");
    let (dialog, panel, sizer) = styled_dialog(&frame, &format!("Collection: {name}"), 560, 420);
    let info = TextCtrl::builder(&panel)
        .with_value(&strip_html(full.get("description").and_then(Value::as_str).unwrap_or("")).trim().to_string())
        .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly)
        .with_size(Size::new(-1, 80))
        .build();
    if info.get_value().trim().is_empty() {
        info.set_value("No description.");
    }
    let list = ListBox::builder(&panel).build();
    for account in &accounts {
        list.append(&app::account_label_line(account));
    }
    theme::paint(&info);
    theme::paint(&list);
    let view = Button::builder(&panel).with_label("&View Profile").build();
    let open = Button::builder(&panel).with_label("Open in &Browser").build();
    let close = Button::builder(&panel).with_label("&Close").with_id(ID_CANCEL).build();
    theme::paint(&view);
    theme::paint(&open);
    theme::paint(&close);
    sizer.add(&info, 0, SizerFlag::Expand | SizerFlag::All, 10);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 10);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&view, 0, SizerFlag::All, 5);
    let remove = if can_manage {
        let remove = Button::builder(&panel).with_label("&Remove").build();
        theme::paint(&remove);
        buttons.add(&remove, 0, SizerFlag::All, 5);
        Some(remove)
    } else {
        None
    };
    buttons.add(&open, 0, SizerFlag::All, 5);
    buttons.add_stretch_spacer(1);
    buttons.add(&close, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::Expand | SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    let accounts = Rc::new(RefCell::new(accounts));
    let app_view = app.clone();
    let accounts_view = accounts.clone();
    view.on_click(move |_| {
        let Some(index) = list.get_selection() else { return };
        let Some(account) = accounts_view.borrow().get(index as usize).cloned() else { return };
        let full = app_view.borrow().api.account(&id_of(&account)).unwrap_or(account);
        profile_dialog(&app_view, &dialog, &full);
    });
    if let Some(remove) = remove {
        let api = app.borrow().api.clone();
        let accounts_remove = accounts.clone();
        let collection_id = id_of(&full);
        let full_remove = full.clone();
        remove.on_click(move |_| {
            let Some(index) = list.get_selection() else { return };
            let Some(account) = accounts_remove.borrow().get(index as usize).cloned() else { return };
            let Some(item) = full_remove.get("items").and_then(Value::as_array).and_then(|items| items.iter().find(|item| item.get("account_id").and_then(Value::as_str) == Some(id_of(&account).as_str()))) else {
                alert(&dialog, "This account cannot be removed from the collection response.", "Collections");
                return;
            };
            if !confirm(&dialog, "Remove this account from the collection?", "Collections") {
                return;
            }
            match api.call("DELETE", &format!("/api/v1/collections/{collection_id}/items/{}", id_of(item)), Vec::new()) {
                Ok(_) => {
                    accounts_remove.borrow_mut().remove(index as usize);
                    list.delete(index);
                }
                Err(error) => error_box(&dialog, &format!("Error removing account: {error}"), "Collections"),
            }
        });
    }
    let url = full.get("url").and_then(Value::as_str).unwrap_or("").to_string();
    open.on_click(move |_| {
        if !url.is_empty() {
            open_browser(&url);
        }
    });
    dialog.show_modal();
    dialog.destroy();
}

pub fn choose_collection(app: &Rc<RefCell<App>>, collections: &[Value]) {
    let frame = app.borrow().frame;
    let (dialog, panel, sizer) = styled_dialog(&frame, "Collection Search Results", 540, 360);
    let list = ListBox::builder(&panel).build();
    for collection in collections {
        list.append(&collection_label(collection, ""));
    }
    theme::paint(&list);
    if !collections.is_empty() {
        list.set_selection(0, true);
    }
    let open = Button::builder(&panel).with_label("&Open").build();
    let close = Button::builder(&panel).with_label("&Close").with_id(ID_CANCEL).build();
    theme::paint(&open);
    theme::paint(&close);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 10);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&open, 0, SizerFlag::All, 5);
    buttons.add(&close, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    let app_open = app.clone();
    let collections = collections.to_vec();
    open.on_click(move |_| {
        if let Some(index) = list.get_selection() {
            show_collection(&app_open, &collections[index as usize], false);
        }
    });
    dialog.show_modal();
    dialog.destroy();
}

fn collection_form(parent: &impl WxWidget, title: &str, initial: &Value, include_accounts: bool) -> Option<Value> {
    let (dialog, panel, sizer) = styled_dialog(parent, title, 520, if include_accounts { 430 } else { 380 });
    let name = labeled_text(&panel, &sizer, "&Name:", initial.get("name").and_then(Value::as_str).unwrap_or(""));
    let description = labeled_area(&panel, &sizer, "&Description:", &strip_html(initial.get("description").and_then(Value::as_str).unwrap_or("")));
    let language = labeled_text(&panel, &sizer, "&Language code:", initial.get("language").and_then(Value::as_str).unwrap_or(""));
    let tag = labeled_text(&panel, &sizer, "&Topic hashtag:", initial.pointer("/tag/name").and_then(Value::as_str).unwrap_or(""));
    let accounts = if include_accounts {
        Some(labeled_text(&panel, &sizer, "Initial account &IDs, comma separated:", ""))
    } else {
        None
    };
    let sensitive = CheckBox::builder(&panel).with_label("Mark collection as &sensitive").build();
    sensitive.set_value(initial.get("sensitive").and_then(Value::as_bool).unwrap_or(false));
    let discoverable = CheckBox::builder(&panel).with_label("Show collection on profile and in discovery").build();
    discoverable.set_value(initial.get("discoverable").and_then(Value::as_bool).unwrap_or(true));
    theme::paint(&sensitive);
    theme::paint(&discoverable);
    sizer.add(&sensitive, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 10);
    sizer.add(&discoverable, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 10);
    let ok = Button::builder(&panel).with_label("&OK").with_id(ID_OK).build();
    let cancel = Button::builder(&panel).with_label("&Cancel").with_id(ID_CANCEL).build();
    theme::paint(&ok);
    theme::paint(&cancel);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&ok, 0, SizerFlag::All, 5);
    buttons.add(&cancel, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 10);
    panel.set_sizer(sizer, true);
    let dialog_ok = dialog;
    ok.on_click(move |_| dialog_ok.end_modal(ID_OK));
    let accepted = dialog.show_modal() == ID_OK;
    let result = if accepted {
        let mut value = json!({
            "name": name.get_value().trim(),
            "description": description.get_value().trim(),
            "sensitive": sensitive.get_value(),
            "discoverable": discoverable.get_value(),
        });
        let language = language.get_value();
        if !language.trim().is_empty() {
            value["language"] = json!(language.trim());
        }
        let tag = tag.get_value().trim().trim_start_matches('#').to_string();
        if !tag.is_empty() {
            value["tag_name"] = json!(tag);
        }
        if let Some(accounts) = accounts {
            let ids = accounts.get_value().split(',').map(str::trim).filter(|item| !item.is_empty()).map(|item| json!(item)).collect::<Vec<_>>();
            if !ids.is_empty() {
                value["account_ids"] = Value::Array(ids);
            }
        }
        Some(value)
    } else {
        None
    };
    dialog.destroy();
    result
}

fn labeled_text(panel: &Panel, sizer: &BoxSizer, label: &str, value: &str) -> TextCtrl {
    let caption = StaticText::builder(panel).with_label(label).build();
    let input = TextCtrl::builder(panel).with_value(value).build();
    theme::paint(&caption);
    theme::paint(&input);
    sizer.add(&caption, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 10);
    sizer.add(&input, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 10);
    input
}

fn labeled_area(panel: &Panel, sizer: &BoxSizer, label: &str, value: &str) -> TextCtrl {
    let caption = StaticText::builder(panel).with_label(label).build();
    let input = TextCtrl::builder(panel).with_value(value).with_style(TextCtrlStyle::MultiLine).with_size(Size::new(-1, 80)).build();
    theme::paint(&caption);
    theme::paint(&input);
    sizer.add(&caption, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 10);
    sizer.add(&input, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 10);
    input
}

pub fn user_collections(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let Some((status, _)) = app::selected_status(&app.borrow()) else { return };
    let account = source_status(&status).get("account").cloned().unwrap_or(Value::Null);
    let id = id_of(&account);
    let collections = match app.borrow().api.call("GET", &format!("/api/v1/accounts/{id}/collections"), vec![("limit".to_string(), "80".to_string())]) {
        Ok(value) => collections_from_response(&value),
        Err(error) => {
            error_box(&frame, &format!("Error loading collections: {error}"), "Collections");
            return;
        }
    };
    if collections.is_empty() {
        alert(&frame, "This user has no visible collections.", "Collections");
        return;
    }
    choose_collection(app, &collections);
}

pub fn hashtags(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let (dialog, panel, sizer) = styled_dialog(&frame, "Followed Hashtags", 500, 400);
    let list = ListBox::builder(&panel).build();
    let mut tags = app.borrow().api.call("GET", "/api/v1/followed_tags", Vec::new()).map(crate::api::as_array).unwrap_or_default();
    for tag in &tags {
        list.append(&format!("#{}", tag.get("name").and_then(Value::as_str).unwrap_or("")));
    }
    theme::paint(&list);
    let open = Button::builder(&panel).with_label("&Open Timeline").build();
    let follow = Button::builder(&panel).with_label("&Follow New Hashtag").build();
    let unfollow = Button::builder(&panel).with_label("&Unfollow").build();
    let close = Button::builder(&panel).with_label("&Close").with_id(ID_CANCEL).build();
    for button in [&open, &follow, &unfollow, &close] {
        theme::paint(button);
    }
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 10);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    for button in [&open, &follow, &unfollow, &close] {
        buttons.add(button, 0, SizerFlag::All, 3);
    }
    sizer.add_sizer(&buttons, 0, SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    let app_open = app.clone();
    let dialog_open = dialog;
    let tags_open = std::sync::Arc::new(std::sync::Mutex::new(tags.clone()));
    let stored = tags_open.clone();
    let open_selected = Rc::new(move || {
        let Some(index) = list.get_selection() else { return };
        let Some(name) = stored.lock().ok().and_then(|tags| tags.get(index as usize).and_then(|tag| tag.get("name").and_then(Value::as_str).map(|value| value.to_string()))) else { return };
        let key = format!("hashtag:{name}");
        app::ensure_timeline(&app_open, &key, &format!("#{name}"));
        app::begin_load(&app_open, &key);
        dialog_open.end_modal(ID_CANCEL);
    });
    let open_action = open_selected.clone();
    open.on_click(move |_| open_action());
    list.on_item_double_clicked(move |_| open_selected());
    let api = app.borrow().api.clone();
    let stored = tags_open.clone();
    follow.on_click(move |_| {
        let Some(name) = prompt(&dialog, "Hashtag to follow (without #):", "Follow Hashtag") else { return };
        let name = name.trim().trim_start_matches('#').to_string();
        if name.is_empty() { return; }
        match api.call("POST", &format!("/api/v1/tags/{name}/follow"), Vec::new()) {
            Ok(tag) => {
                list.append(&format!("#{}", tag.get("name").and_then(Value::as_str).unwrap_or(&name)));
                if let Ok(mut tags) = stored.lock() { tags.push(tag); }
            }
            Err(error) => error_box(&dialog, &format!("Error: {error}"), "Error"),
        }
    });
    let api = app.borrow().api.clone();
    unfollow.on_click(move |_| {
        let Some(index) = list.get_selection() else { return };
        let Some(name) = tags.get(index as usize).and_then(|tag| tag.get("name").and_then(Value::as_str)).map(|value| value.to_string()) else { return };
        match api.call("POST", &format!("/api/v1/tags/{name}/unfollow"), Vec::new()) {
            Ok(_) => {
                tags.remove(index as usize);
                list.delete(index);
            }
            Err(error) => error_box(&dialog, &format!("Error: {error}"), "Error"),
        }
    });
    dialog.show_modal();
    dialog.destroy();
}

pub fn account_list(app: &Rc<RefCell<App>>, title: &str, kind: &str) {
    let frame = app.borrow().frame;
    let me = id_of(&app.borrow().me);
    let path = match kind {
        "followers" => format!("/api/v1/accounts/{me}/followers"),
        "following" => format!("/api/v1/accounts/{me}/following"),
        "blocks" => "/api/v1/blocks".to_string(),
        "mutes" => "/api/v1/mutes".to_string(),
        _ => return,
    };
    let accounts = match app.borrow().api.account_list(&path) {
        Ok(accounts) => accounts,
        Err(error) => {
            error_box(&frame, &format!("Error: {error}"), "Error");
            return;
        }
    };
    show_accounts(app, title, &accounts);
}

pub fn follow_requests(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let accounts = match app.borrow().api.account_list("/api/v1/follow_requests") {
        Ok(accounts) => accounts,
        Err(error) => {
            error_box(&frame, &format!("Error: {error}"), "Error");
            return;
        }
    };
    if accounts.is_empty() {
        alert(&frame, "No pending follow requests.", "Follow Requests");
        return;
    }
    let (dialog, panel, sizer) = styled_dialog(&frame, "Follow Requests", 450, 300);
    let list = ListBox::builder(&panel).build();
    for account in &accounts {
        list.append(&app::account_label_line(account));
    }
    list.set_selection(0, true);
    theme::paint(&list);
    let accept = Button::builder(&panel).with_label("&Accept").build();
    let reject = Button::builder(&panel).with_label("Re&ject").build();
    let close = Button::builder(&panel).with_label("&Close").with_id(ID_CANCEL).build();
    theme::paint(&accept);
    theme::paint(&reject);
    theme::paint(&close);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 10);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&accept, 0, SizerFlag::All, 5);
    buttons.add(&reject, 0, SizerFlag::All, 5);
    buttons.add(&close, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    let api = app.borrow().api.clone();
    let accounts = Rc::new(RefCell::new(accounts));
    let dialog_accept = dialog;
    let accounts_accept = accounts.clone();
    accept.on_click(move |_| {
        let Some(index) = list.get_selection() else { return };
        let id = accounts_accept.borrow().get(index as usize).map(id_of).unwrap_or_default();
        match api.follow_request(&id, "authorize") {
            Ok(_) => {
                accounts_accept.borrow_mut().remove(index as usize);
                list.delete(index);
                alert(&dialog_accept, "Follow request accepted.", "Accepted");
            }
            Err(error) => error_box(&dialog_accept, &format!("Error: {error}"), "Error"),
        }
    });
    let api = app.borrow().api.clone();
    reject.on_click(move |_| {
        let Some(index) = list.get_selection() else { return };
        let id = accounts.borrow().get(index as usize).map(id_of).unwrap_or_default();
        match api.follow_request(&id, "reject") {
            Ok(_) => {
                accounts.borrow_mut().remove(index as usize);
                list.delete(index);
                alert(&dialog, "Follow request rejected.", "Rejected");
            }
            Err(error) => error_box(&dialog, &format!("Error: {error}"), "Error"),
        }
    });
    dialog.show_modal();
    dialog.destroy();
}

fn show_accounts(app: &Rc<RefCell<App>>, title: &str, accounts: &[Value]) {
    let frame = app.borrow().frame;
    let (dialog, panel, sizer) = styled_dialog(&frame, title, 450, 300);
    let list = ListBox::builder(&panel).build();
    for account in accounts {
        list.append(&app::account_label_line(account));
    }
    if !accounts.is_empty() {
        list.set_selection(0, true);
    }
    theme::paint(&list);
    let view = Button::builder(&panel).with_label("&View").build();
    let close = Button::builder(&panel).with_label("&Close").with_id(ID_CANCEL).build();
    theme::paint(&view);
    theme::paint(&close);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 10);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&view, 0, SizerFlag::All, 5);
    buttons.add(&close, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    let accounts = accounts.to_vec();
    let app_view = app.clone();
    view.on_click(move |_| {
        if let Some(index) = list.get_selection() {
            let account = app_view.borrow().api.account(&id_of(&accounts[index as usize])).unwrap_or_else(|_| accounts[index as usize].clone());
            profile_dialog(&app_view, &dialog, &account);
        }
    });
    dialog.show_modal();
    dialog.destroy();
}

fn choose_account(parent: &impl WxWidget, accounts: &[Value]) -> Option<Value> {
    let (dialog, panel, sizer) = styled_dialog(parent, "Select a User to View", 450, 300);
    let list = ListBox::builder(&panel).build();
    for account in accounts {
        list.append(&app::account_label_line(account));
    }
    list.set_selection(0, true);
    theme::paint(&list);
    let view = Button::builder(&panel).with_label("&View").with_id(ID_OK).build();
    let cancel = Button::builder(&panel).with_label("&Cancel").with_id(ID_CANCEL).build();
    theme::paint(&view);
    theme::paint(&cancel);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 10);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&view, 0, SizerFlag::All, 5);
    buttons.add(&cancel, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 10);
    panel.set_sizer(sizer, true);
    let dialog_view = dialog;
    view.on_click(move |_| dialog_view.end_modal(ID_OK));
    list.on_item_double_clicked(move |_| dialog.end_modal(ID_OK));
    let result = if dialog.show_modal() == ID_OK {
        list.get_selection().map(|index| accounts[index as usize].clone())
    } else {
        None
    };
    dialog.destroy();
    result
}

pub fn profile_from_selection(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let Some((status, _)) = app::selected_status(&app.borrow()) else { return };
    let mut unique = Vec::<Value>::new();
    for account in [status.get("account"), source_status(&status).get("account")].into_iter().flatten() {
        if !unique.iter().any(|item| id_of(item) == id_of(account)) {
            unique.push(account.clone());
        }
    }
    if let Some(mentions) = source_status(&status).get("mentions").and_then(Value::as_array) {
        for mention in mentions {
            if !unique.iter().any(|item| id_of(item) == id_of(mention)) {
                unique.push(mention.clone());
            }
        }
    }
    unique.sort_by_key(|account| display_name(account).to_lowercase());
    let account = if unique.len() == 1 {
        unique.into_iter().next()
    } else {
        choose_account(&frame, &unique)
    };
    let Some(account) = account else { return };
    let full = app.borrow().api.account(&id_of(&account)).unwrap_or(account);
    profile_dialog(app, &frame, &full);
}

fn approval_names(approval: &Value, key: &str) -> String {
    approval
        .get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).filter(|name| !name.is_empty()).collect::<Vec<_>>().join(", "))
        .filter(|names| !names.is_empty())
        .unwrap_or_else(|| "nobody".to_string())
}

fn profile_dialog(app: &Rc<RefCell<App>>, parent: &impl WxWidget, account: &Value) {
    let title = format!("Profile for {} ({})", display_name(account), account.get("acct").and_then(Value::as_str).unwrap_or(""));
    let (dialog, panel, sizer) = styled_dialog(parent, &title, 600, 500);
    let mut info = format!(
        "Display Name: {}\nUsername: {}\nBio: {}\nFollowers: {}\nFriends: {}\nPosts: {}\nCreated: {}\nLast post: {}\nWebsite: {}",
        display_name(account),
        account.get("acct").and_then(Value::as_str).unwrap_or(""),
        strip_html(account.get("note").and_then(Value::as_str).unwrap_or("")),
        account.get("followers_count").and_then(Value::as_i64).unwrap_or(0),
        account.get("following_count").and_then(Value::as_i64).unwrap_or(0),
        account.get("statuses_count").and_then(Value::as_i64).unwrap_or(0),
        account.get("created_at").and_then(Value::as_str).map(long_date).unwrap_or_else(|| "Unknown".to_string()),
        account.get("last_status_at").and_then(Value::as_str).unwrap_or("Unknown"),
        account.get("url").and_then(Value::as_str).unwrap_or("")
    );
    if let Some(text) = account.get("avatar_description").and_then(Value::as_str).filter(|text| !text.is_empty()) {
        info.push_str(&format!("\nAvatar description: {text}"));
    }
    if let Some(text) = account.get("header_description").and_then(Value::as_str).filter(|text| !text.is_empty()) {
        info.push_str(&format!("\nHeader description: {text}"));
    }
    let mut preferences = Vec::new();
    if let Some(shown) = account.get("show_media").and_then(Value::as_bool) {
        preferences.push(if shown { "Media tab shown" } else { "Media tab hidden" });
        if shown && let Some(replies) = account.get("show_media_replies").and_then(Value::as_bool) {
            preferences.push(if replies { "Media replies included" } else { "Media replies hidden" });
        }
    }
    if let Some(shown) = account.get("show_featured").and_then(Value::as_bool) {
        preferences.push(if shown { "Featured tab shown" } else { "Featured tab hidden" });
    }
    if !preferences.is_empty() {
        info.push_str("\nProfile preferences: ");
        info.push_str(&preferences.join(", "));
    }
    if let Some(approval) = account.get("feature_approval").filter(|value| value.as_object().is_some_and(|object| !object.is_empty())) {
        let automatic = approval_names(approval, "automatic");
        let manual = approval_names(approval, "manual");
        info.push_str(&format!("\nCollection approval: automatic for {automatic}; manual review for {manual}"));
        if let Some(current) = approval.get("current_user").and_then(Value::as_str).filter(|text| !text.is_empty()) {
            info.push_str(&format!("; you are {current}"));
        }
    }
    let relationship = if id_of(account) != id_of(&app.borrow().me) {
        app.borrow().api.relationships(&id_of(account)).ok().and_then(|value| value.as_array().and_then(|items| items.first().cloned()))
    } else {
        None
    };
    if let Some(relationship) = &relationship {
        let mut lines = Vec::new();
        if relationship.get("following").and_then(Value::as_bool).unwrap_or(false) { lines.push("You follow this user"); }
        if relationship.get("followed_by").and_then(Value::as_bool).unwrap_or(false) { lines.push("This user follows you"); }
        if relationship.get("blocking").and_then(Value::as_bool).unwrap_or(false) { lines.push("You have blocked this user"); }
        if relationship.get("muting").and_then(Value::as_bool).unwrap_or(false) { lines.push("You have muted this user"); }
        if relationship.get("requested").and_then(Value::as_bool).unwrap_or(false) { lines.push("Follow request pending"); }
        if !lines.is_empty() {
            info.push_str("\n\nRelationship:\n");
            info.push_str(&lines.join("\n"));
        }
    }
    let text = TextCtrl::builder(&panel).with_value(&info).with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly).build();
    theme::paint(&text);
    sizer.add(&text, 1, SizerFlag::All | SizerFlag::Expand, 10);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    if id_of(account) != id_of(&app.borrow().me) {
        let following = relationship.as_ref().and_then(|item| item.get("following")).and_then(Value::as_bool).unwrap_or(false);
        let muting = relationship.as_ref().and_then(|item| item.get("muting")).and_then(Value::as_bool).unwrap_or(false);
        let blocking = relationship.as_ref().and_then(|item| item.get("blocking")).and_then(Value::as_bool).unwrap_or(false);
        let follow = Button::builder(&panel).with_label(if following { "Un&follow" } else { "&Follow" }).build();
        let mute = Button::builder(&panel).with_label(if muting { "Un&mute" } else { "&Mute" }).build();
        let block = Button::builder(&panel).with_label(if blocking { "Un&block" } else { "&Block" }).build();
        theme::paint(&follow);
        theme::paint(&mute);
        theme::paint(&block);
        buttons.add(&follow, 0, SizerFlag::All, 5);
        buttons.add(&mute, 0, SizerFlag::All, 5);
        buttons.add(&block, 0, SizerFlag::All, 5);
        let api = app.borrow().api.clone();
        let account_id = id_of(account);
        let display = display_name(account);
        let dialog_follow = dialog;
        follow.on_click(move |_| {
            let label = follow.get_label();
            if label.contains("Un") {
                if confirm(&dialog_follow, &format!("Unfollow {display}?"), "Confirm") {
                    if let Err(error) = api.account_action(&account_id, "unfollow") {
                        error_box(&dialog_follow, &format!("Error: {error}"), "Follow Error");
                    } else {
                        sounds::play("unfollowsnd");
                        follow.set_label("&Follow");
                    }
                }
            } else if let Err(error) = api.account_action(&account_id, "follow") {
                error_box(&dialog_follow, &format!("Error: {error}"), "Follow Error");
            } else {
                sounds::play("followsnd");
                follow.set_label("Un&follow");
            }
        });
        let api = app.borrow().api.clone();
        let account_id = id_of(account);
        mute.on_click(move |_| {
            let un = mute.get_label().contains("Un");
            let action = if un { "unmute" } else { "mute" };
            if let Err(error) = api.account_action(&account_id, action) {
                error_box(&dialog, &format!("Error: {error}"), "Mute Error");
            } else {
                mute.set_label(if un { "&Mute" } else { "Un&mute" });
            }
        });
        let api = app.borrow().api.clone();
        let account_id = id_of(account);
        let display = display_name(account);
        block.on_click(move |_| {
            let un = block.get_label().contains("Un");
            let question = if un { format!("Unblock {display}?") } else { format!("Block {display}?") };
            if !confirm(&dialog, &question, "Confirm") { return; }
            let action = if un { "unblock" } else { "block" };
            if let Err(error) = api.account_action(&account_id, action) {
                error_box(&dialog, &format!("Error: {error}"), "Block Error");
            } else {
                block.set_label(if un { "&Block" } else { "Un&block" });
            }
        });
    }
    let browse = Button::builder(&panel).with_label("Open in &Browser").build();
    let close = Button::builder(&panel).with_label("&Close").with_id(ID_CANCEL).build();
    theme::paint(&browse);
    theme::paint(&close);
    buttons.add(&browse, 0, SizerFlag::All, 5);
    buttons.add_stretch_spacer(1);
    buttons.add(&close, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::Expand | SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    let url = account.get("url").and_then(Value::as_str).unwrap_or("").to_string();
    browse.on_click(move |_| { if !url.is_empty() { open_browser(&url); } });
    text.set_focus();
    dialog.show_modal();
    dialog.destroy();
}

pub fn edit_profile(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let api = app.borrow().api.clone();
    let (profile, modern) = match api.call("GET", "/api/v1/profile", Vec::new()) {
        Ok(profile) => (profile, true),
        Err(_) => (app.borrow().me.clone(), false),
    };
    let (dialog, panel, sizer) = styled_dialog(&frame, "Edit My Profile", 600, 720);
    let name = labeled_text(&panel, &sizer, "Display &Name:", profile.get("display_name").and_then(Value::as_str).unwrap_or(""));
    let note = if modern { profile.get("note").and_then(Value::as_str).unwrap_or("") } else { profile.pointer("/source/note").and_then(Value::as_str).unwrap_or("") };
    let bio = labeled_area(&panel, &sizer, "&Bio:", note);
    let fields = profile
        .get("fields")
        .and_then(Value::as_array)
        .cloned()
        .filter(|items| !items.is_empty())
        .or_else(|| app.borrow().me.pointer("/source/fields").and_then(Value::as_array).cloned())
        .or_else(|| app.borrow().me.get("fields").and_then(Value::as_array).cloned())
        .unwrap_or_default();
    let max_fields = api
        .instance_v2()
        .ok()
        .and_then(|instance| instance.pointer("/configuration/accounts/max_profile_fields").cloned())
        .and_then(|value| value.as_i64().or_else(|| value.as_str().and_then(|text| text.parse().ok())))
        .filter(|count| *count > 0)
        .map(|count| count as usize)
        .unwrap_or_else(|| fields.len().max(4));
    let mut field_inputs = Vec::new();
    for index in 0..max_fields {
        let row = BoxSizer::builder(Orientation::Horizontal).build();
        let label = StaticText::builder(&panel).with_label(&format!("Label {}:", index + 1)).build();
        let field_name = TextCtrl::builder(&panel).with_value(fields.get(index).and_then(|field| field.get("name")).and_then(Value::as_str).unwrap_or("")).with_size(Size::new(150, -1)).build();
        let value_label = StaticText::builder(&panel).with_label("Value:").build();
        let field_value = TextCtrl::builder(&panel).with_value(&strip_html(fields.get(index).and_then(|field| field.get("value")).and_then(Value::as_str).unwrap_or(""))).build();
        theme::paint(&label);
        theme::paint(&field_name);
        theme::paint(&value_label);
        theme::paint(&field_value);
        row.add(&label, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 3);
        row.add(&field_name, 0, SizerFlag::Right, 8);
        row.add(&value_label, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 3);
        row.add(&field_value, 1, SizerFlag::Expand, 0);
        sizer.add_sizer(&row, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 10);
        field_inputs.push((field_name, field_value));
    }
    let avatar = labeled_text(&panel, &sizer, "Avatar &Description:", profile.get("avatar_description").and_then(Value::as_str).unwrap_or(""));
    let header = labeled_text(&panel, &sizer, "Header Des&cription:", profile.get("header_description").and_then(Value::as_str).unwrap_or(""));
    let domains = profile.get("attribution_domains").and_then(Value::as_array).map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default();
    let attribution = labeled_text(&panel, &sizer, "Attribution &Domains, comma separated:", &domains);
    let locked = flag_box(&panel, &sizer, "&Lock account (require follow approval)", profile.get("locked"));
    let bot = flag_box(&panel, &sizer, "Mark as &bot account", profile.get("bot"));
    let discoverable = flag_box(&panel, &sizer, "Show in profile directory and discovery", profile.get("discoverable"));
    let indexable = flag_box(&panel, &sizer, "Allow public posts to be searchable", profile.get("indexable"));
    let hide_collections = flag_box(&panel, &sizer, "Hide follows and followers", profile.get("hide_collections"));
    let show_media = flag_box(&panel, &sizer, "Show Media tab", profile.get("show_media"));
    let show_media_replies = flag_box(&panel, &sizer, "Include replies in Media tab", profile.get("show_media_replies"));
    let show_featured = flag_box(&panel, &sizer, "Show Featured tab", profile.get("show_featured"));
    let save = Button::builder(&panel).with_label("&Save").build();
    let cancel = Button::builder(&panel).with_label("&Cancel").with_id(ID_CANCEL).build();
    theme::paint(&save);
    theme::paint(&cancel);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&save, 0, SizerFlag::All, 5);
    buttons.add(&cancel, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 10);
    panel.set_sizer(sizer, true);
    let dialog_save = dialog;
    let app_save = app.clone();
    save.on_click(move |_| {
        let mut fields = Vec::new();
        for (name, value) in &field_inputs {
            let name = name.get_value().trim().to_string();
            let value = value.get_value().trim().to_string();
            if !name.is_empty() || !value.is_empty() {
                fields.push(json!({"name": name, "value": value}));
            }
        }
        let profile = json!({
            "display_name": name.get_value().trim(),
            "note": bio.get_value().trim(),
            "locked": locked.get_value(),
            "bot": bot.get_value(),
            "discoverable": discoverable.get_value(),
            "indexable": indexable.get_value(),
            "hide_collections": hide_collections.get_value(),
            "show_media": show_media.get_value(),
            "show_media_replies": show_media_replies.get_value(),
            "show_featured": show_featured.get_value(),
            "avatar_description": avatar.get_value().trim(),
            "header_description": header.get_value().trim(),
            "attribution_domains": attribution.get_value().split(',').map(str::trim).filter(|item| !item.is_empty()).collect::<Vec<_>>(),
            "fields_attributes": fields,
        });
        let result = if modern {
            api.call("PATCH", "/api/v1/profile", flatten_profile(&profile))
        } else {
            api.call("PATCH", "/api/v1/accounts/update_credentials", flatten_profile(&profile))
        };
        match result.and_then(|_| api.me()) {
            Ok(me) => {
                app_save.borrow_mut().me = me;
                alert(&dialog_save, "Profile updated successfully!", "Profile");
                dialog_save.end_modal(ID_OK);
            }
            Err(error) => error_box(&dialog_save, &format!("Error updating profile: {error}"), "Error"),
        }
    });
    dialog.show_modal();
    dialog.destroy();
}

fn flag_box(panel: &Panel, sizer: &BoxSizer, label: &str, value: Option<&Value>) -> CheckBox {
    let checked = value.and_then(Value::as_bool).unwrap_or(false);
    let checkbox = CheckBox::builder(panel).with_label(label).build();
    checkbox.set_value(checked);
    theme::paint(&checkbox);
    sizer.add(&checkbox, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 10);
    checkbox
}

pub fn instance_info(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let api = app.borrow().api.clone();
    let instance = match api.instance() {
        Ok(instance) => instance,
        Err(error) => {
            error_box(&frame, &format!("Error: {error}"), "Error");
            return;
        }
    };
    let v2 = api.instance_v2().unwrap_or(json!({}));
    let title = instance.get("title").and_then(Value::as_str).unwrap_or("Unknown");
    let description = strip_html(instance.get("description").and_then(Value::as_str).or_else(|| instance.get("short_description").and_then(Value::as_str)).unwrap_or(""));
    let contact = instance.get("contact_account").cloned().unwrap_or(Value::Null);
    let admin = display_name(&contact);
    let wrapstodon = v2.get("wrapstodon");
    let mut wrap_text = "Not currently offered".to_string();
    if let Some(year) = wrapstodon.and_then(Value::as_i64).or_else(|| wrapstodon.and_then(Value::as_str).and_then(|value| value.parse().ok())) {
        wrap_text = format!("Offered for {year}");
        if let Ok(state) = api.call("GET", &format!("/api/v1/annual_reports/{year}/state"), Vec::new()) {
            wrap_text.push_str(&format!(" ({})", state.get("state").and_then(Value::as_str).unwrap_or("unknown")));
        }
    }
    let info = format!(
        "Instance: {title}\nURI: {}\nVersion: {}\nMastodon API version: {}\nAdmin: {admin}\n\nUsers: {}\nPosts: {}\nKnown domains: {}\nWrapstodon: {wrap_text}\nThumbnail description: {}\n\nDescription:\n{description}",
        instance.get("uri").and_then(Value::as_str).unwrap_or(""),
        instance.get("version").and_then(Value::as_str).unwrap_or("Unknown"),
        v2.pointer("/api_versions/mastodon").and_then(Value::as_i64).map(|value| value.to_string()).or_else(|| v2.pointer("/api_versions/mastodon").and_then(Value::as_str).map(|value| value.to_string())).unwrap_or_else(|| "Unknown".to_string()),
        instance.pointer("/stats/user_count").map(|value| value.to_string()).unwrap_or_else(|| "?".to_string()),
        instance.pointer("/stats/status_count").map(|value| value.to_string()).unwrap_or_else(|| "?".to_string()),
        instance.pointer("/stats/domain_count").map(|value| value.to_string()).unwrap_or_else(|| "?".to_string()),
        strip_html(v2.pointer("/thumbnail/description").and_then(Value::as_str).unwrap_or("None"))
    );
    let (dialog, panel, sizer) = styled_dialog(&frame, &format!("Instance Info: {title}"), 550, 400);
    let text = TextCtrl::builder(&panel).with_value(&info).with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly).build();
    theme::paint(&text);
    sizer.add(&text, 1, SizerFlag::Expand | SizerFlag::All, 10);
    if let Some(year) = wrapstodon.and_then(Value::as_i64).or_else(|| wrapstodon.and_then(Value::as_str).and_then(|value| value.parse().ok())) {
        let wrap = Button::builder(&panel).with_label("&Wrapstodon").build();
        theme::paint(&wrap);
        sizer.add(&wrap, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 10);
        let dialog_wrap = dialog;
        wrap.on_click(move |_| {
            let Ok(state) = api.call("GET", &format!("/api/v1/annual_reports/{year}/state"), Vec::new()) else {
                error_box(&dialog_wrap, "Error loading Wrapstodon.", "Wrapstodon");
                return;
            };
            match state.get("state").and_then(Value::as_str).unwrap_or("unknown") {
                "eligible" => {
                    if confirm(&dialog_wrap, &format!("Generate your {year} Wrapstodon report?"), "Wrapstodon") {
                        match api.call("POST", &format!("/api/v1/annual_reports/{year}/generate"), Vec::new()) {
                            Ok(_) => alert(&dialog_wrap, "Wrapstodon generation has started.", "Wrapstodon"),
                            Err(error) => error_box(&dialog_wrap, &format!("Error loading Wrapstodon: {error}"), "Wrapstodon"),
                        }
                    }
                }
                "available" => {
                    let share = api.call("GET", &format!("/api/v1/annual_reports/{year}"), Vec::new()).ok().and_then(|report| report.get("annual_reports").and_then(Value::as_array).and_then(|items| items.first()).and_then(|item| item.get("share_url")).and_then(Value::as_str).map(|value| value.to_string())).unwrap_or_default();
                    alert(&dialog_wrap, &format!("Your {year} Wrapstodon report is available.\n{share}"), "Wrapstodon");
                }
                "generating" => alert(&dialog_wrap, "Your Wrapstodon report is still generating.", "Wrapstodon"),
                _ => alert(&dialog_wrap, "Your account is not eligible for Wrapstodon right now.", "Wrapstodon"),
            }
        });
    }
    let close = Button::builder(&panel).with_label("&Close").with_id(ID_CANCEL).build();
    theme::paint(&close);
    sizer.add(&close, 0, SizerFlag::AlignRight | SizerFlag::All, 10);
    panel.set_sizer(sizer, true);
    dialog.show_modal();
    dialog.destroy();
}

pub fn post_details(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let Some((status, _)) = app::selected_status(&app.borrow()) else {
        alert(&frame, "This notification has no associated post.", "No Post");
        return;
    };
    post_details_for(app, &frame, status);
}

const POST_WEBVIEW_SCRIPT: &str = "function addEvent(elem, event, handler) { \
if (elem.addEventListener) { \
elem.addEventListener(event, handler, false); \
} else if (elem.attachEvent) { \
elem.attachEvent('on' + event, handler); \
} \
} \
addEvent(document, 'keydown', function(event) { \
if (event.key === 'Escape' || event.keyCode === 27) { \
window.wx.postMessage('close_dialog'); \
} \
}); \
addEvent(document, 'click', function(event) { \
event = event || window.event; \
var target = event.target || event.srcElement; \
while (target && target.tagName !== 'A') { target = target.parentNode; } \
if (target && target.tagName === 'A' && target.href) { \
if (event.preventDefault) event.preventDefault(); \
else event.returnValue = false; \
window.wx.postMessage('open_link:' + target.href); \
} \
});";

fn post_document(status: &Value) -> String {
    let account = status.get("account").cloned().unwrap_or(Value::Null);
    let name = escape_html(&display_name(&account));
    let acct = escape_html(account.get("acct").and_then(Value::as_str).unwrap_or(""));
    let (background, foreground, border) = if theme::is_dark() {
        ("#282828", "#ffffff", "#888888")
    } else {
        ("#ffffff", "#1b1b1b", "GrayText")
    };
    format!(
        "<html><head><meta charset=\"utf-8\"><title>Post content</title><style>\
body {{ font-family: sans-serif; padding: 10px; line-height: 1.5; background: {background}; color: {foreground}; }}\
a {{ color: {foreground}; }}\
article {{ border: 1px solid {border}; border-radius: 4px; margin: 0 0 12px 0; padding: 10px 12px; }}\
.content-warning {{ border: 1px solid {border}; padding: 6px 8px; }}\
img, video {{ max-width: 100%; height: auto; }}\
p {{ margin: 0; }}\
p + p {{ margin-top: 1.5em; }}\
p, span, div, pre, code {{ white-space: pre-wrap; }}\
</style></head><body><h2>{name} <small>({acct})</small></h2>{}{}</body></html>",
        build_status_body_html(status),
        media_html(status)
    )
}

fn media_html(status: &Value) -> String {
    let Some(media) = status.get("media_attachments").and_then(Value::as_array).filter(|items| !items.is_empty()) else {
        return String::new();
    };
    let mut html = String::from("<section aria-labelledby=\"media-heading\"><h2 id=\"media-heading\">Media</h2><ul>");
    for (index, attachment) in media.iter().enumerate() {
        let kind = match attachment.get("type").and_then(Value::as_str).unwrap_or("") {
            "image" => "Image",
            "gifv" => "GIF",
            "video" => "Video",
            "audio" => "Audio",
            _ => "Attachment",
        };
        let description = attachment
            .get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map_or_else(|| "No description".to_string(), escape_html);
        html.push_str(&format!("<li>{kind} {}: {description}</li>", index + 1));
    }
    html.push_str("</ul></section>");
    html
}

fn post_details_for(app: &Rc<RefCell<App>>, parent: &impl WxWidget, status: Value) {
    let source = source_status(&status);
    let account = source.get("account").cloned().unwrap_or(Value::Null);
    let title = format!("View Post from {} ({}) dialog", display_name(&account), account.get("acct").and_then(Value::as_str).unwrap_or(""));
    let (dialog, panel, sizer) = styled_dialog(parent, &title, 700, 560);
    let content_label = StaticText::builder(&panel).with_label("Post C&ontent").build();
    let content = WebView::builder(&panel).build();
    content.add_script_message_handler("wx");
    let dialog_message = dialog;
    content.on_script_message_received(move |event: WebViewEventData| {
        if let Some(message) = event.get_string() {
            if message == "close_dialog" {
                dialog_message.end_modal(ID_CANCEL);
            } else if let Some(url) = message.strip_prefix("open_link:") {
                let _ = launch_default_browser(url, BrowserLaunchFlags::Default);
            }
        }
    });
    let content_loaded = content;
    content.on_loaded(move |_| {
        content_loaded.run_script(POST_WEBVIEW_SCRIPT);
    });
    content.set_page(&post_document(&source), "");
    let created = source.get("created_at").and_then(Value::as_str).unwrap_or("");
    let mut details = format!(
        "Posted: {}\nFrom: {}\nBoosted {} times\nFavorited {} times.\n{} replies\nPrivacy: {}\nLanguage: {}",
        clock_time(created),
        crate::rows::app_name(&source),
        source.get("reblogs_count").and_then(Value::as_i64).unwrap_or(0),
        source.get("favourites_count").and_then(Value::as_i64).unwrap_or(0),
        source.get("replies_count").and_then(Value::as_i64).unwrap_or(0),
        source.get("visibility").and_then(Value::as_str).unwrap_or("unknown"),
        source.get("language").and_then(Value::as_str).unwrap_or("unknown")
    );
    if let Some(quoted) = get_quoted_status(&source) {
        let author = quoted.get("account").cloned().unwrap_or(Value::Null);
        let handle = author.get("acct").and_then(Value::as_str).unwrap_or("");
        details.push_str(&format!("\n\nQuoting {} (@{handle}):\n{}", display_name(&author), status_plain_text(quoted)));
    }
    if source.pointer("/card/missing_attribution").and_then(Value::as_bool).unwrap_or(false) {
        let card = source.pointer("/card/title").and_then(Value::as_str).or_else(|| source.pointer("/card/url").and_then(Value::as_str)).unwrap_or("link preview");
        details.push_str(&format!("\nAttribution warning: {card} claims to credit you, but its domain is not in your attribution domains."));
    }
    let details_label = StaticText::builder(&panel).with_label("Post &Details").build();
    let details_box = TextCtrl::builder(&panel).with_value(&details).with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly).with_size(Size::new(-1, 120)).build();
    theme::paint(&content_label);
    theme::paint(&details_label);
    theme::paint(&details_box);
    sizer.add(&content_label, 0, SizerFlag::All, 5);
    sizer.add(&content, 1, SizerFlag::All | SizerFlag::Expand, 5);
    sizer.add(&details_label, 0, SizerFlag::All, 5);
    sizer.add(&details_box, 0, SizerFlag::All | SizerFlag::Expand, 5);
    if let Some(poll) = source.get("poll").filter(|value| value.is_object()) {
        add_poll(app, dialog, &panel, &sizer, &id_of(&source), poll);
    }
    let links = links_in_status(&source);
    let reply = Button::builder(&panel).with_label("&Reply").build();
    let boost = Button::builder(&panel).with_label(if source.get("reblogged").and_then(Value::as_bool).unwrap_or(false) { "Un&boost" } else { "&Boost" }).build();
    let favourite = Button::builder(&panel).with_label(if source.get("favourited").and_then(Value::as_bool).unwrap_or(false) { "Un&favourite" } else { "&Favourite" }).build();
    let bookmark = Button::builder(&panel).with_label(if source.get("bookmarked").and_then(Value::as_bool).unwrap_or(false) { "Unboo&kmark" } else { "Boo&kmark" }).build();
    let copy = Button::builder(&panel).with_label("&Copy Text").build();
    let open = Button::builder(&panel).with_label("Open UR&L").build();
    let thread_button = Button::builder(&panel).with_label("View &Thread").build();
    let links_button = Button::builder(&panel).with_label("View Lin&ks").build();
    links_button.enable(!links.is_empty());
    let profile = Button::builder(&panel).with_label(&format!("View &Profile of {}", display_name(&account))).build();
    let close = Button::builder(&panel).with_label("C&lose").with_id(ID_CANCEL).build();
    for button in [&reply, &boost, &favourite, &bookmark, &copy, &open, &thread_button, &links_button, &profile, &close] {
        theme::paint(button);
    }
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    for button in [&reply, &boost, &favourite, &bookmark, &copy, &open, &thread_button, &links_button] {
        buttons.add(button, 0, SizerFlag::All, 5);
    }
    let own = id_of(&account) == id_of(&app.borrow().me);
    let own_buttons = if own {
        let edit = Button::builder(&panel).with_label("&Edit").build();
        let pin = Button::builder(&panel)
            .with_label(if source.get("pinned").and_then(Value::as_bool).unwrap_or(false) { "Un&pin" } else { "&Pin" })
            .build();
        let delete = Button::builder(&panel).with_label("Take &down").build();
        theme::paint(&edit);
        theme::paint(&pin);
        theme::paint(&delete);
        buttons.add(&edit, 0, SizerFlag::All, 5);
        buttons.add(&pin, 0, SizerFlag::All, 5);
        buttons.add(&delete, 0, SizerFlag::All, 5);
        Some((edit, pin, delete))
    } else {
        None
    };
    buttons.add_stretch_spacer(1);
    buttons.add(&profile, 0, SizerFlag::All, 5);
    buttons.add(&close, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::Expand | SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    let app_boost = app.clone();
    boost.on_click(move |_| {
        if app::boost(&app_boost) {
            let boosted = boost.get_label().contains("Un");
            boost.set_label(if boosted { "&Boost" } else { "Un&boost" });
        }
    });
    let app_favourite = app.clone();
    favourite.on_click(move |_| {
        if app::favourite(&app_favourite) {
            let favourited = favourite.get_label().contains("Un");
            favourite.set_label(if favourited { "&Favourite" } else { "Un&favourite" });
        }
    });
    let app_bookmark = app.clone();
    bookmark.on_click(move |_| {
        if app::bookmark(&app_bookmark) {
            let saved = bookmark.get_label().contains("Un");
            bookmark.set_label(if saved { "Boo&kmark" } else { "Unboo&kmark" });
        }
    });
    let plain = status_plain_text(&source);
    copy.on_click(move |_| copy_text(&plain));
    let url = source.get("url").and_then(Value::as_str).or_else(|| source.get("uri").and_then(Value::as_str)).unwrap_or("").to_string();
    open.on_click(move |_| { if !url.is_empty() { open_browser(&url); } });
    let api_thread = app.borrow().api.clone();
    let source_thread = source.clone();
    let dialog_thread = dialog;
    thread_button.on_click(move |_| show_thread(&dialog_thread, &api_thread, &source_thread));
    let dialog_links = dialog;
    let links_for_button = links.clone();
    links_button.on_click(move |_| show_links(&dialog_links, &links_for_button));
    let app_profile = app.clone();
    let account_profile = account.clone();
    profile.on_click(move |_| profile_dialog(&app_profile, &dialog, &account_profile));
    let api = app.borrow().api.clone();
    let status_id = id_of(&source);
    let app_reply = app.clone();
    let source_reply = source.clone();
    reply.on_click(move |_| {
        let me = app_reply.borrow().me.get("acct").and_then(Value::as_str).unwrap_or("").to_string();
        reply_dialog(&dialog, &api, &status_id, &reply_mentions(&source_reply, &me), source_reply.get("visibility").and_then(Value::as_str).unwrap_or("public"));
    });
    if let Some((edit, pin, delete)) = own_buttons {
        let source_edit = source.clone();
        let app_edit = app.clone();
        let dialog_edit = dialog;
        let content_edit = content;
        edit.on_click(move |_| {
            if let Some(updated) = app::edit_status(&app_edit, &dialog_edit, &source_edit) {
                content_edit.set_page(&post_document(&updated), "");
                let mut state = app_edit.borrow_mut();
                app::replace_status(&mut state, updated);
            }
        });
        let app_pin = app.clone();
        let dialog_pin = dialog;
        let status_id = id_of(&source);
        let pinned = std::cell::Cell::new(source.get("pinned").and_then(Value::as_bool).unwrap_or(false));
        pin.on_click(move |_| {
            if app::set_pinned(&app_pin, &dialog_pin, &status_id, pinned.get()) {
                let was_pinned = pinned.get();
                pinned.set(!was_pinned);
                pin.set_label(if was_pinned { "&Pin" } else { "Un&pin" });
            }
        });
        let api = app.borrow().api.clone();
        let app_delete = app.clone();
        let id = id_of(&source);
        delete.on_click(move |_| {
            if !confirm(&dialog, "Are you sure you want to take down this post? It will be removed from Mastodon. This action cannot be undone.", "Confirm Deletion") {
                return;
            }
            if let Err(error) = api.status_action(&id, "") {
                error_box(&dialog, &format!("Error deleting post: {error}"), "Error");
            } else {
                let mut state = app_delete.borrow_mut();
                app::remove_status(&mut state, &id);
                drop(state);
                dialog.end_modal(ID_CANCEL);
            }
        });
    }
    content.set_focus();
    dialog.show_modal();
    dialog.destroy();
}

fn add_poll(app: &Rc<RefCell<App>>, dialog: Dialog, panel: &Panel, sizer: &BoxSizer, status_id: &str, poll: &Value) {
    let options = poll.get("options").and_then(Value::as_array).cloned().unwrap_or_default();
    if options.is_empty() {
        return;
    }
    let voted = poll.get("voted").and_then(Value::as_bool).unwrap_or(false);
    let expired = poll.get("expired").and_then(Value::as_bool).unwrap_or(false);
    let show_results = voted || expired;
    let labels: Vec<String> = options
        .iter()
        .map(|option| {
            let title = option.get("title").and_then(Value::as_str).unwrap_or("");
            if show_results {
                format!("{title} ({} votes)", option.get("votes_count").and_then(Value::as_i64).unwrap_or(0))
            } else {
                title.to_string()
            }
        })
        .collect();
    let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let poll_sizer = StaticBoxSizerBuilder::new_with_label(Orientation::Vertical, panel, "Poll").build();
    if let Some(poll_box) = poll_sizer.get_static_box() {
        theme::paint(&poll_box);
    }
    let radios = RadioBox::builder(panel, &label_refs)
        .with_major_dimension(1)
        .with_style(RadioBoxStyle::SpecifyCols)
        .build();
    if let Some(index) = own_vote_index(poll) {
        radios.set_selection(index);
    }
    let vote = Button::builder(panel).with_label("&Vote").build();
    vote.enable(!voted && !expired);
    let info = StaticText::builder(panel).with_label(&poll_info(poll)).build();
    theme::paint(&radios);
    theme::paint(&vote);
    theme::paint(&info);
    poll_sizer.add(&radios, 0, SizerFlag::Expand | SizerFlag::All, 5);
    poll_sizer.add(&vote, 0, SizerFlag::All, 5);
    poll_sizer.add(&info, 0, SizerFlag::All, 5);
    sizer.add_sizer(&poll_sizer, 0, SizerFlag::Expand | SizerFlag::All, 5);

    let api = app.borrow().api.clone();
    let app_vote = app.clone();
    let poll_id = poll.get("id").and_then(Value::as_str).unwrap_or("").to_string();
    let status_id = status_id.to_string();
    let panel = *panel;
    vote.on_click(move |_| {
        let selection = radios.get_selection();
        if selection < 0 {
            alert(&dialog, "Please select an option to vote.", "No Selection");
            return;
        }
        match api.poll_vote(&poll_id, selection as i64) {
            Ok(updated) => {
                sounds::play("votesnd");
                app::store_poll(&app_vote, &status_id, &updated);
                vote.enable(false);
                info.set_label(&poll_results(&updated));
                panel.layout();
                alert(&dialog, "Your vote has been cast successfully!", "Vote Cast");
            }
            Err(error) => error_box(&dialog, &format!("Failed to cast vote: {error}"), "Error"),
        }
    });
}

fn own_vote_index(poll: &Value) -> Option<i32> {
    let votes = poll.get("own_votes")?.as_array()?;
    let first = votes.first()?;
    first
        .as_i64()
        .or_else(|| first.as_str().and_then(|text| text.parse().ok()))
        .map(|index| index as i32)
}

fn poll_info(poll: &Value) -> String {
    let mut text = format!("Total votes: {}", poll.get("votes_count").and_then(Value::as_i64).unwrap_or(0));
    if poll.get("expired").and_then(Value::as_bool).unwrap_or(false) {
        text.push_str(" - Poll is closed.");
    }
    text
}

fn poll_results(poll: &Value) -> String {
    let mut text = poll_info(poll);
    if let Some(options) = poll.get("options").and_then(Value::as_array) {
        let parts: Vec<String> = options
            .iter()
            .map(|option| {
                format!(
                    "{}: {}",
                    option.get("title").and_then(Value::as_str).unwrap_or(""),
                    option.get("votes_count").and_then(Value::as_i64).unwrap_or(0)
                )
            })
            .collect();
        if !parts.is_empty() {
            text.push(' ');
            text.push_str(&parts.join(", "));
            text.push('.');
        }
    }
    text
}

fn reply_mentions(status: &Value, me: &str) -> String {
    let mut users = String::new();
    for word in status_plain_text(status).split_whitespace() {
        if word.starts_with('@') && word != format!("@{me}") {
            users.push_str(word);
            users.push(' ');
        }
    }
    let author = status.get("account").and_then(|account| account.get("acct")).and_then(Value::as_str).unwrap_or("");
    if !author.is_empty() && author != me {
        format!("@{author} {users}")
    } else {
        users
    }
}

fn reply_dialog(parent: &impl WxWidget, api: &crate::api::Mastodon, status_id: &str, mentions: &str, visibility: &str) {
    let (dialog, panel, sizer) = styled_dialog(parent, "Reply to Post", 500, 300);
    let label = StaticText::builder(&panel).with_label("&Reply").build();
    let initial = {
        let trimmed = mentions.trim();
        if trimmed.is_empty() { String::new() } else { format!("{trimmed} ") }
    };
    let text = TextCtrl::builder(&panel)
        .with_value(&initial)
        .with_style(TextCtrlStyle::MultiLine)
        .with_size(Size::new(480, 100))
        .build();
    text.set_insertion_point_end();
    let privacy_label = StaticText::builder(&panel).with_label("P&rivacy:").build();
    let selected = app::PRIVACY_VALUES.iter().position(|item| *item == visibility).unwrap_or(0) as u32;
    let privacy = Choice::builder(&panel)
        .with_choices(app::PRIVACY_LABELS.iter().map(|item| (*item).to_string()).collect())
        .with_selection(Some(selected))
        .build();
    let send = Button::builder(&panel).with_label("&Post").build();
    let cancel = Button::builder(&panel).with_label("&Cancel").with_id(ID_CANCEL).build();
    theme::paint(&label);
    theme::paint(&privacy_label);
    theme::paint(&text);
    theme::paint(&privacy);
    theme::paint(&send);
    theme::paint(&cancel);
    sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 10);
    sizer.add(&text, 1, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 10);
    sizer.add(&privacy_label, 0, SizerFlag::Left | SizerFlag::Right, 10);
    sizer.add(&privacy, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 10);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&send, 0, SizerFlag::All, 5);
    buttons.add(&cancel, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::Right | SizerFlag::Bottom, 10);
    panel.set_sizer(sizer, true);

    let api = api.clone();
    let status_id = status_id.to_string();
    let dialog_send = dialog;
    let send_reply = Rc::new(move || {
        let body = text.get_value().trim().to_string();
        if body.is_empty() {
            error_box(&dialog_send, "Reply cannot be empty.", "Error");
            return;
        }
        let visibility = app::PRIVACY_VALUES
            .get(privacy.get_selection().unwrap_or(0) as usize)
            .copied()
            .unwrap_or("public");
        match api.status_post(vec![
            ("status".to_string(), body),
            ("in_reply_to_id".to_string(), status_id.clone()),
            ("visibility".to_string(), visibility.to_string()),
        ]) {
            Ok(_) => {
                sounds::play("replysnd");
                dialog_send.end_modal(ID_OK);
            }
            Err(error) => error_box(&dialog_send, &format!("Error sending reply: {error}"), "Error"),
        }
    });
    let send_click = send_reply.clone();
    send.on_click(move |_| send_click());
    dialog.on_char_hook(move |event| {
        let post = matches!(&event, WindowEventData::Keyboard(key) if key.get_key_code() == Some(WXK_RETURN) && (key.control_down() || key.shift_down() || key.alt_down()));
        if post {
            send_reply();
        } else {
            event.skip(true);
        }
    });
    text.set_focus();
    dialog.show_modal();
    dialog.destroy();
}

fn show_thread(parent: &impl WxWidget, api: &crate::api::Mastodon, status: &Value) {
    let id = id_of(status);
    let context = match api.status_context(&id) {
        Ok(context) => context,
        Err(error) => {
            error_box(parent, &format!("Error loading thread: {error}"), "Thread Error");
            return;
        }
    };
    let mut posts = context.get("ancestors").and_then(Value::as_array).cloned().unwrap_or_default();
    posts.push(status.clone());
    if let Some(descendants) = context.get("descendants").and_then(Value::as_array) {
        posts.extend(descendants.clone());
    }
    let (dialog, panel, sizer) = styled_dialog(parent, "Thread View", 600, 500);
    let list = ListBox::builder(&panel).with_size(Size::new(-1, 350)).build();
    for post in &posts {
        list.append(&thread_line(post, &id));
    }
    if let Some(index) = posts.iter().position(|post| id_of(post) == id) {
        list.set_selection(index as u32, true);
    }
    theme::paint(&list);
    let close = Button::builder(&panel).with_label("&Close").with_id(ID_CANCEL).build();
    theme::paint(&close);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 10);
    sizer.add(&close, 0, SizerFlag::AlignRight | SizerFlag::All, 10);
    panel.set_sizer(sizer, true);
    dialog.show_modal();
    dialog.destroy();
}

fn thread_line(post: &Value, current_id: &str) -> String {
    let account = post.get("account").cloned().unwrap_or(Value::Null);
    let raw = post.get("content").and_then(Value::as_str).unwrap_or("");
    let spaced = raw.replace("<br />", " ").replace("<br>", " ").replace("</p>", " ");
    let content = strip_html(&spaced).split_whitespace().collect::<Vec<_>>().join(" ");
    let content: String = content.chars().take(200).collect();
    let prefix = if id_of(post) == current_id { ">>> " } else { "" };
    format!("{prefix}{}: {content}", display_name(&account))
}

fn show_links(parent: &impl WxWidget, links: &[String]) {
    if links.is_empty() { return; }
    let (dialog, panel, sizer) = styled_dialog(parent, "Links in Post", 500, 300);
    let list = ListBox::builder(&panel).build();
    for link in links { list.append(link); }
    list.set_selection(0, true);
    theme::paint(&list);
    let open = Button::builder(&panel).with_label("&Open Link").build();
    let close = Button::builder(&panel).with_label("&Close").with_id(ID_CANCEL).build();
    theme::paint(&open);
    theme::paint(&close);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 5);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&open, 0, SizerFlag::All, 5);
    buttons.add(&close, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    let links = links.to_vec();
    let dialog_open = dialog;
    let open_selected = Rc::new(move || {
        if let Some(index) = list.get_selection() {
            open_browser(&links[index as usize]);
            dialog_open.end_modal(ID_OK);
        } else {
            alert(&dialog_open, "Please select a link to open.", "No Link Selected");
        }
    });
    let open_action = open_selected.clone();
    open.on_click(move |_| open_action());
    list.on_item_double_clicked(move |_| open_selected());
    dialog.show_modal();
    dialog.destroy();
}

pub fn media(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let Some((status, _)) = app::selected_status(&app.borrow()) else { return };
    let source = source_status(&status);
    let attachments = source.get("media_attachments").and_then(Value::as_array).cloned().unwrap_or_default();
    if attachments.is_empty() {
        alert(&frame, "This post has no media attachments.", "No Media");
        return;
    }
    let (dialog, panel, sizer) = styled_dialog(&frame, &format!("Media Attachments ({})", attachments.len()), 600, 450);
    let list = ListBox::builder(&panel).build();
    for attachment in &attachments {
        let kind = attachment.get("type").and_then(Value::as_str).unwrap_or("unknown");
        let description = attachment.get("description").and_then(Value::as_str).unwrap_or("No description");
        list.append(&format!("[{}] {description}", capitalize(kind)));
    }
    let description = TextCtrl::builder(&panel).with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly).with_size(Size::new(-1, 80)).build();
    theme::paint(&list);
    theme::paint(&description);
    let label = StaticText::builder(&panel).with_label("Description:").build();
    theme::paint(&label);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 10);
    sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right, 10);
    sizer.add(&description, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 10);
    let open = Button::builder(&panel).with_label("Open in &Browser").build();
    let copy = Button::builder(&panel).with_label("&Copy URL").build();
    let close = Button::builder(&panel).with_label("C&lose").with_id(ID_CANCEL).build();
    theme::paint(&open);
    theme::paint(&copy);
    theme::paint(&close);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&open, 0, SizerFlag::All, 5);
    buttons.add(&copy, 0, SizerFlag::All, 5);
    buttons.add(&close, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    if !attachments.is_empty() {
        list.set_selection(0, true);
        description.set_value(attachments[0].get("description").and_then(Value::as_str).unwrap_or("No description provided"));
    }
    let attachments_select = attachments.clone();
    list.on_selection_changed(move |_| {
        if let Some(index) = list.get_selection() {
            description.set_value(attachments_select[index as usize].get("description").and_then(Value::as_str).unwrap_or("No description provided"));
        }
    });
    let attachments_open = attachments.clone();
    open.on_click(move |_| {
        if let Some(index) = list.get_selection() {
            let url = attachments_open[index as usize].get("url").and_then(Value::as_str).or_else(|| attachments_open[index as usize].get("remote_url").and_then(Value::as_str)).unwrap_or("");
            if !url.is_empty() { open_browser(url); }
        }
    });
    copy.on_click(move |_| {
        if let Some(index) = list.get_selection() {
            let url = attachments[index as usize].get("url").and_then(Value::as_str).or_else(|| attachments[index as usize].get("remote_url").and_then(Value::as_str)).unwrap_or("");
            copy_text(url);
        }
    });
    dialog.show_modal();
    dialog.destroy();
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub fn who(app: &Rc<RefCell<App>>, action: &str, title: &str, empty: &str, empty_title: &str) {
    let frame = app.borrow().frame;
    let Some((status, _)) = app::selected_status(&app.borrow()) else { return };
    let id = id_of(&source_status(&status));
    match app.borrow().api.call("GET", &format!("/api/v1/statuses/{id}/{action}"), Vec::new()) {
        Ok(value) => {
            let accounts = crate::api::as_array(value);
            if accounts.is_empty() {
                alert(&frame, empty, empty_title);
            } else {
                show_accounts(app, &format!("{title} ({})", accounts.len()), &accounts);
            }
        }
        Err(error) => error_box(&frame, &format!("Error: {error}"), "Error"),
    }
}

pub fn history(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let Some((status, _)) = app::selected_status(&app.borrow()) else { return };
    let id = id_of(&source_status(&status));
    let history = match app.borrow().api.call("GET", &format!("/api/v1/statuses/{id}/history"), Vec::new()) {
        Ok(value) => crate::api::as_array(value),
        Err(error) => {
            error_box(&frame, &format!("Error: {error}"), "Error");
            return;
        }
    };
    if history.len() < 2 {
        alert(&frame, "This post has no edit history.", "Edit History");
        return;
    }
    let (dialog, panel, sizer) = styled_dialog(&frame, &format!("Edit History ({} versions)", history.len()), 600, 400);
    let list = ListBox::builder(&panel).build();
    for (index, version) in history.iter().enumerate() {
        let when = version.get("created_at").and_then(Value::as_str).unwrap_or("");
        list.append(&format!("Version {} - {when}", history.len() - index));
    }
    let content = TextCtrl::builder(&panel).with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly).build();
    theme::paint(&list);
    theme::paint(&content);
    let label = StaticText::builder(&panel).with_label("Content:").build();
    theme::paint(&label);
    sizer.add(&list, 0, SizerFlag::Expand | SizerFlag::All, 10);
    sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right, 10);
    sizer.add(&content, 1, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 10);
    let close = Button::builder(&panel).with_label("&Close").with_id(ID_CANCEL).build();
    theme::paint(&close);
    sizer.add(&close, 0, SizerFlag::AlignRight | SizerFlag::All, 10);
    panel.set_sizer(sizer, true);
    list.set_selection(0, true);
    content.set_value(&history_text(&history[0]));
    list.on_selection_changed(move |_| {
        if let Some(index) = list.get_selection() {
            content.set_value(&history_text(&history[index as usize]));
        }
    });
    dialog.show_modal();
    dialog.destroy();
}

fn history_text(version: &Value) -> String {
    let body = status_plain_text(version);
    if let Some(spoiler) = version.get("spoiler_text").and_then(Value::as_str).filter(|text| !text.is_empty()) {
        format!("CW: {spoiler}\n\n{body}")
    } else {
        body
    }
}

pub fn report(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let Some((status, _)) = app::selected_status(&app.borrow()) else { return };
    let source = source_status(&status);
    let account = source.get("account").cloned().unwrap_or(Value::Null);
    let display = display_name(&account);
    let (dialog, panel, sizer) = styled_dialog(&frame, &format!("Report {display}"), 500, 350);
    let heading = StaticText::builder(&panel).with_label(&format!("Report {display} (@{})", account.get("acct").and_then(Value::as_str).unwrap_or(""))).build();
    let label = StaticText::builder(&panel).with_label("&Reason (optional):").build();
    let reason = TextCtrl::builder(&panel).with_style(TextCtrlStyle::MultiLine).with_size(Size::new(-1, 100)).build();
    let include = CheckBox::builder(&panel).with_label("Include this specific post in the report").build();
    include.set_value(true);
    theme::paint(&heading);
    theme::paint(&label);
    theme::paint(&reason);
    theme::paint(&include);
    sizer.add(&heading, 0, SizerFlag::All, 10);
    sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right, 10);
    sizer.add(&reason, 1, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 10);
    sizer.add(&include, 0, SizerFlag::Left | SizerFlag::Right, 10);
    let submit = Button::builder(&panel).with_label("&Submit Report").build();
    let cancel = Button::builder(&panel).with_label("&Cancel").with_id(ID_CANCEL).build();
    theme::paint(&submit);
    theme::paint(&cancel);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&submit, 0, SizerFlag::All, 5);
    buttons.add(&cancel, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::All, 5);
    panel.set_sizer(sizer, true);
    let api = app.borrow().api.clone();
    let account_id = id_of(&account);
    let status_id = id_of(&source);
    let dialog_submit = dialog;
    submit.on_click(move |_| {
        let mut fields = vec![("account_id".to_string(), account_id.clone()), ("comment".to_string(), reason.get_value().trim().to_string())];
        if include.get_value() {
            fields.push(("status_ids[]".to_string(), status_id.clone()));
        }
        match api.call("POST", "/api/v1/reports", fields) {
            Ok(_) => {
                alert(&dialog_submit, "Report submitted successfully.", "Report");
                dialog_submit.end_modal(ID_OK);
            }
            Err(error) => error_box(&dialog_submit, &format!("Error: {error}"), "Error"),
        }
    });
    dialog.show_modal();
    dialog.destroy();
}
