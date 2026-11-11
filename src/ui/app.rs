use super::{alert, confirm, copy_text, dialogs, error_box, open_browser, prompt};
use crate::announce::Announcer;
use crate::api::Mastodon;
use crate::render::{get_quoted_status, status_plain_text};
use crate::rows::{row_from_notification, row_from_status, source_status};
use crate::sounds;
use crate::theme;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::cell::RefCell;
use std::time::Duration;
use wxdragon::event::window_events::KeyboardEvent;
use wxdragon::prelude::*;
use wxdragon::sizers::staticbox_sizer::StaticBoxSizerBuilder;

const HOME: &[&str] = &[
    "home",
    "local",
    "federated",
    "sent",
    "direct_messages",
    "favourites",
    "bookmarks",
    "notifications",
    "mentions",
];

const TITLES: &[(&str, &str)] = &[
    ("home", "Home"),
    ("local", "Local"),
    ("federated", "Federated"),
    ("sent", "Sent"),
    ("direct_messages", "Direct Messages"),
    ("favourites", "Favourites"),
    ("bookmarks", "Bookmarks"),
    ("notifications", "Notifications"),
    ("mentions", "Mentions"),
];

pub const SETTINGS: i32 = 10001;
pub const FILTERS: i32 = 10002;
pub const REFRESH: i32 = 10003;
pub const AVATARS: i32 = 10004;
pub const FIND: i32 = 10005;
pub const SCHEDULED: i32 = 10006;
pub const EXPLORE: i32 = 10007;
pub const LISTS: i32 = 10008;
pub const COLLECTIONS: i32 = 10009;
pub const HASHTAGS: i32 = 10010;
pub const FAVOURITES_TL: i32 = 10011;
pub const BOOKMARKS_TL: i32 = 10012;
pub const FOLLOWERS: i32 = 10013;
pub const FOLLOWING: i32 = 10014;
pub const BLOCKED: i32 = 10015;
pub const MUTED_USERS: i32 = 10016;
pub const REQUESTS: i32 = 10017;
pub const EDIT_PROFILE: i32 = 10018;
pub const INSTANCE: i32 = 10019;
pub const COMPOSE: i32 = 10020;
pub const REPLY: i32 = 10021;
pub const QUOTE: i32 = 10022;
pub const BOOST: i32 = 10023;
pub const FAVOURITE: i32 = 10024;
pub const BOOKMARK: i32 = 10025;
pub const COPY: i32 = 10026;
pub const OPEN_URL: i32 = 10027;
pub const VIEW_POST: i32 = 10028;
pub const THREAD: i32 = 10029;
pub const FOLLOW: i32 = 10030;
pub const BLOCK: i32 = 10031;
pub const MUTE_USER: i32 = 10032;
pub const EDIT: i32 = 10033;
pub const PIN: i32 = 10034;
pub const MUTE_CONVO: i32 = 10035;
pub const REPORT: i32 = 10036;
pub const MEDIA: i32 = 10037;
pub const BOOSTERS: i32 = 10038;
pub const FAVOURITERS: i32 = 10039;
pub const HISTORY: i32 = 10040;
pub const PROFILE: i32 = 10041;
pub const USER_COLLECTIONS: i32 = 10042;
pub const SEARCH: i32 = 10043;
pub const USER_TIMELINE: i32 = 10044;
pub const DM: i32 = 10045;
pub const DISMISS: i32 = 10046;
pub const CLEAR_NOTIFICATIONS: i32 = 10047;
pub const ACCEPT: i32 = 10048;
pub const REJECT: i32 = 10049;
pub const QUOTE_PERMISSIONS: i32 = 10050;
pub const VIEW_QUOTES: i32 = 10051;
pub const REVOKE_QUOTE: i32 = 10052;

pub(crate) const PRIVACY_LABELS: &[&str] = &["Public", "Unlisted", "Followers-only", "Direct"];
pub(crate) const PRIVACY_VALUES: &[&str] = &["public", "unlisted", "private", "direct"];
const LANGUAGES: &[(&str, &str)] = &[
    ("Default", ""),
    ("English", "en"),
    ("Spanish", "es"),
    ("French", "fr"),
    ("German", "de"),
    ("Italian", "it"),
    ("Portuguese", "pt"),
    ("Japanese", "ja"),
    ("Chinese", "zh"),
    ("Korean", "ko"),
    ("Russian", "ru"),
    ("Arabic", "ar"),
    ("Dutch", "nl"),
    ("Polish", "pl"),
    ("Swedish", "sv"),
    ("Turkish", "tr"),
    ("Indonesian", "id"),
    ("Hindi", "hi"),
    ("Thai", "th"),
    ("Vietnamese", "vi"),
];
const POLL_LABELS: &[&str] = &[
    "5 minutes",
    "30 minutes",
    "1 hour",
    "6 hours",
    "12 hours",
    "1 day",
    "3 days",
    "7 days",
];
const POLL_SECONDS: &[i64] = &[300, 1800, 3600, 21600, 43200, 86400, 259200, 604800];
const QUOTE_POLICY_LABELS: &[&str] = &["Default", "Anybody", "Followers", "Just me"];
const QUOTE_POLICY_VALUES: &[Option<&str>] = &[None, Some("public"), Some("followers"), Some("nobody")];

enum UiMsg {
    Timeline { key: String, generation: u64, result: Result<Vec<Value>, String> },
    Stream(Value),
    Avatar { url: String, pixels: Option<Vec<u8>> },
}

pub(crate) struct App {
    pub api: Mastodon,
    pub me: Value,
    pub frame: Frame,
    pub announcer: Announcer,
    pub timelines: HashMap<String, Vec<Value>>,
    pub nodes: Vec<(String, TreeItemId)>,
    pub tree: TreeCtrl,
    pub root: TreeItemId,
    pub posts: ListCtrl,
    pub toot: TextCtrl,
    pub cw_label: StaticText,
    pub cw_input: TextCtrl,
    pub cw_toggle: CheckBox,
    pub poll_toggle: CheckBox,
    pub media_toggle: CheckBox,
    pub schedule_toggle: CheckBox,
    pub privacy: Choice,
    pub language: Choice,
    pub quote_policy: Choice,
    pub supports_quotes: bool,
    pub supports_collections: bool,
    pub post_button: Button,
    pub poll_inputs: Vec<TextCtrl>,
    pub poll_labels: Vec<StaticText>,
    pub duration_label: StaticText,
    pub poll_duration: Choice,
    pub poll_multiple: CheckBox,
    pub poll_box: Option<StaticBox>,
    pub media_list: ListBox,
    pub add_media: Button,
    pub remove_media: Button,
    pub alt_label: StaticText,
    pub alt_input: TextCtrl,
    pub media_box: Option<StaticBox>,
    pub schedule_date: TextCtrl,
    pub schedule_time: TextCtrl,
    pub schedule_date_label: StaticText,
    pub schedule_time_label: StaticText,
    pub media_files: Vec<(String, String)>,
    pub show_avatars: bool,
    pub focus: AtomicI32,
    tx: Sender<UiMsg>,
    rx: Receiver<UiMsg>,
    loads: HashMap<String, u64>,
    next_load: u64,
    row_avatars: Vec<Option<String>>,
    avatar_ids: HashMap<String, i32>,
    pending_avatars: HashSet<String>,
}

pub fn open(api: Mastodon, me: Value) {
    let version = crate::api::api_version(&api.instance_v2().unwrap_or(Value::Null));
    let supports_quotes = version.is_some_and(|version| version >= crate::api::QUOTE_POSTS_API_VERSION);
    let supports_collections = version.is_some_and(|version| version >= crate::api::COLLECTIONS_API_VERSION);
    let frame = Frame::builder()
        .with_title("Thrive Mastodon Client")
        .with_size(Size::new(1100, 700))
        .build();
    let panel = Panel::builder(&frame).build();
    theme::paint(&frame);
    theme::paint(&panel);
    let announcer = Announcer::new(&panel);

    let (tx, rx) = mpsc::channel();
    let tree = TreeCtrl::builder(&panel)
        .with_style(TreeCtrlStyle::HasButtons | TreeCtrlStyle::HideRoot)
        .build();
    let root = tree.add_root("Timelines", None, None).expect("timeline root");
    let mut nodes = Vec::new();
    for key in HOME {
        let title = TITLES.iter().find(|(name, _)| name == key).map(|(_, title)| *title).unwrap_or(key);
        if let Some(item) = tree.append_item_with_data(&root, title, (*key).to_string(), None, None) {
            nodes.push(((*key).to_string(), item));
        }
    }
    theme::paint(&tree);

    let posts = ListCtrl::builder(&panel)
        .with_style(ListCtrlStyle::Report | ListCtrlStyle::SingleSel)
        .build();
    posts.insert_column(0, "Author", ListColumnFormat::Left, 180);
    posts.insert_column(1, "Content", ListColumnFormat::Left, 640);
    posts.insert_column(2, "Time", ListColumnFormat::Left, 140);
    posts.insert_column(3, "Client", ListColumnFormat::Left, 140);
    theme::paint(&posts);

    let toot_label = StaticText::builder(&panel).with_label("&Create New Post").build();
    let toot = TextCtrl::builder(&panel)
        .with_style(TextCtrlStyle::MultiLine)
        .with_size(Size::new(-1, 80))
        .build();
    let cw_label = StaticText::builder(&panel).with_label("Content w&arning:").build();
    let cw_input = TextCtrl::builder(&panel).with_size(Size::new(400, -1)).build();
    cw_label.show(false);
    cw_input.show(false);
    let cw_toggle = CheckBox::builder(&panel).with_label("C&W").build();
    let poll_toggle = CheckBox::builder(&panel).with_label("&Poll").build();
    let media_toggle = CheckBox::builder(&panel).with_label("M&edia").build();
    let schedule_toggle = CheckBox::builder(&panel).with_label("Sc&hedule").build();
    let privacy_label = StaticText::builder(&panel).with_label("P&rivacy:").build();
    let privacy = Choice::builder(&panel)
        .with_choices(PRIVACY_LABELS.iter().map(|item| (*item).to_string()).collect())
        .with_selection(Some(0))
        .build();
    let language_label = StaticText::builder(&panel).with_label("&Lang:").build();
    let language = Choice::builder(&panel)
        .with_choices(LANGUAGES.iter().map(|(name, _)| (*name).to_string()).collect())
        .with_selection(Some(0))
        .build();
    let quote_policy_label = StaticText::builder(&panel).with_label("&Quotes:").build();
    let quote_policy = Choice::builder(&panel)
        .with_choices(QUOTE_POLICY_LABELS.iter().map(|item| (*item).to_string()).collect())
        .with_selection(Some(0))
        .build();
    let post_button = Button::builder(&panel).with_label("&Post").build();
    let exit_button = Button::builder(&panel).with_label("E&xit").build();

    let poll_sizer = StaticBoxSizerBuilder::new_with_label(Orientation::Vertical, &panel, "Poll Options").build();
    let poll_box = poll_sizer.get_static_box();
    let mut poll_inputs = Vec::new();
    let mut poll_labels = Vec::new();
    for index in 1..=4 {
        let label = StaticText::builder(&panel).with_label(&format!("Option {index}:")).build();
        let input = TextCtrl::builder(&panel).build();
        poll_sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 5);
        poll_sizer.add(&input, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 5);
        poll_labels.push(label);
        poll_inputs.push(input);
    }
    let duration_label = StaticText::builder(&panel).with_label("Duration:").build();
    let poll_duration = Choice::builder(&panel)
        .with_choices(POLL_LABELS.iter().map(|item| (*item).to_string()).collect())
        .with_selection(Some(5))
        .build();
    let poll_multiple = CheckBox::builder(&panel).with_label("Allow multiple choices").build();
    poll_sizer.add(&duration_label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 5);
    poll_sizer.add(&poll_duration, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 5);
    poll_sizer.add(&poll_multiple, 0, SizerFlag::All, 5);
    for label in &poll_labels {
        label.show(false);
        theme::paint(label);
    }
    for input in &poll_inputs {
        input.show(false);
        theme::paint(input);
    }
    duration_label.show(false);
    poll_duration.show(false);
    poll_multiple.show(false);
    if let Some(poll_box) = poll_box {
        poll_box.show(false);
        theme::paint(&poll_box);
    }

    let media_sizer = StaticBoxSizerBuilder::new_with_label(Orientation::Vertical, &panel, "Media Attachments").build();
    let media_box = media_sizer.get_static_box();
    let media_list = ListBox::builder(&panel).with_size(Size::new(-1, 60)).build();
    let add_media = Button::builder(&panel).with_label("Add &Media...").build();
    let remove_media = Button::builder(&panel).with_label("Re&move Media").build();
    let alt_label = StaticText::builder(&panel).with_label("Alt te&xt for selected:").build();
    let alt_input = TextCtrl::builder(&panel).with_size(Size::new(-1, 30)).build();
    let media_buttons = BoxSizer::builder(Orientation::Horizontal).build();
    media_buttons.add(&add_media, 0, SizerFlag::All, 3);
    media_buttons.add(&remove_media, 0, SizerFlag::All, 3);
    media_sizer.add(&media_list, 0, SizerFlag::Expand | SizerFlag::All, 5);
    media_sizer.add_sizer(&media_buttons, 0, SizerFlag::All, 2);
    media_sizer.add(&alt_label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 5);
    media_sizer.add(&alt_input, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 5);
    media_list.show(false);
    add_media.show(false);
    remove_media.show(false);
    alt_label.show(false);
    alt_input.show(false);
    add_media.enable(false);
    if let Some(media_box) = media_box {
        media_box.show(false);
        theme::paint(&media_box);
    }

    let schedule_date_label = StaticText::builder(&panel).with_label("Date (YYYY-MM-DD):").build();
    let schedule_date = TextCtrl::builder(&panel).with_size(Size::new(120, -1)).build();
    let schedule_time_label = StaticText::builder(&panel).with_label("Time (HH:MM):").build();
    let schedule_time = TextCtrl::builder(&panel).with_size(Size::new(80, -1)).build();
    for widget in [&schedule_date_label, &schedule_time_label] {
        widget.show(false);
        theme::paint(widget);
    }
    schedule_date.show(false);
    schedule_time.show(false);

    let posts_label = StaticText::builder(&panel).with_label("Timelines &List").build();
    for widget in [
        &toot_label,
        &cw_label,
        &privacy_label,
        &language_label,
        &quote_policy_label,
        &posts_label,
        &duration_label,
        &alt_label,
    ] {
        theme::paint(widget);
    }
    theme::paint(&toot);
    theme::paint(&cw_input);
    theme::paint(&cw_toggle);
    theme::paint(&poll_toggle);
    theme::paint(&media_toggle);
    theme::paint(&schedule_toggle);
    theme::paint(&privacy);
    theme::paint(&language);
    theme::paint(&quote_policy);
    theme::paint(&post_button);
    theme::paint(&exit_button);
    theme::paint(&poll_duration);
    theme::paint(&poll_multiple);
    theme::paint(&media_list);
    theme::paint(&add_media);
    theme::paint(&remove_media);
    theme::paint(&alt_input);
    theme::paint(&schedule_date);
    theme::paint(&schedule_time);

    install_menus(&frame, supports_collections);
    let vbox = BoxSizer::builder(Orientation::Vertical).build();
    vbox.add(&toot_label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 5);
    vbox.add(&toot, 0, SizerFlag::All | SizerFlag::Expand, 5);
    vbox.add(&cw_label, 0, SizerFlag::Left | SizerFlag::Right, 5);
    vbox.add(&cw_input, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 5);
    let controls = BoxSizer::builder(Orientation::Horizontal).build();
    for toggle in [&cw_toggle, &poll_toggle, &media_toggle, &schedule_toggle] {
        controls.add(toggle, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 5);
    }
    controls.add_spacer(10);
    controls.add(&privacy_label, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 3);
    controls.add(&privacy, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 8);
    controls.add(&language_label, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 3);
    controls.add(&language, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 8);
    if supports_quotes {
        controls.add(&quote_policy_label, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 3);
        controls.add(&quote_policy, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 8);
    } else {
        quote_policy_label.show(false);
        quote_policy.show(false);
    }
    controls.add_stretch_spacer(1);
    controls.add(&post_button, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 5);
    controls.add(&exit_button, 0, SizerFlag::AlignCenterVertical, 0);
    vbox.add_sizer(&controls, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 5);
    vbox.add_sizer(&poll_sizer, 0, SizerFlag::Expand | SizerFlag::All, 5);
    vbox.add_sizer(&media_sizer, 0, SizerFlag::Expand | SizerFlag::All, 5);
    let schedule = BoxSizer::builder(Orientation::Horizontal).build();
    schedule.add(&schedule_date_label, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 5);
    schedule.add(&schedule_date, 0, SizerFlag::Right, 10);
    schedule.add(&schedule_time_label, 0, SizerFlag::AlignCenterVertical | SizerFlag::Right, 5);
    schedule.add(&schedule_time, 0, SizerFlag::AlignLeft, 0);
    vbox.add_sizer(&schedule, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 5);
    vbox.add(&posts_label, 0, SizerFlag::All | SizerFlag::Expand, 5);
    let columns = BoxSizer::builder(Orientation::Horizontal).build();
    columns.add(&tree, 0, SizerFlag::Expand | SizerFlag::All, 5);
    columns.add(&posts, 1, SizerFlag::Expand | SizerFlag::All, 5);
    vbox.add_sizer(&columns, 1, SizerFlag::Expand, 0);
    panel.set_sizer(vbox, true);
    let frame_sizer = BoxSizer::builder(Orientation::Vertical).build();
    frame_sizer.add(&panel, 1, SizerFlag::Expand, 0);
    frame.set_sizer(frame_sizer, true);

    let app = Rc::new(RefCell::new(App {
        api,
        me,
        frame,
        announcer,
        timelines: HOME.iter().map(|key| ((*key).to_string(), Vec::new())).collect(),
        nodes,
        tree,
        root,
        posts,
        toot,
        cw_label,
        cw_input,
        cw_toggle,
        poll_toggle,
        media_toggle,
        schedule_toggle,
        privacy,
        language,
        quote_policy,
        supports_quotes,
        supports_collections,
        post_button,
        poll_inputs,
        poll_labels,
        duration_label,
        poll_duration,
        poll_multiple,
        poll_box,
        media_list,
        add_media,
        remove_media,
        alt_label,
        alt_input,
        media_box,
        schedule_date,
        schedule_time,
        schedule_date_label,
        schedule_time_label,
        media_files: Vec::new(),
        show_avatars: false,
        focus: AtomicI32::new(0),
        tx,
        rx,
        loads: HashMap::new(),
        next_load: 1,
        row_avatars: Vec::new(),
        avatar_ids: HashMap::new(),
        pending_avatars: HashSet::new(),
    }));

    wire(&app, exit_button);
    if let Some((_, home)) = app.borrow().nodes.iter().find(|(key, _)| key == "home") {
        let home = home.clone();
        app.borrow().tree.select_item(&home);
    }
    for key in HOME {
        begin_load(&app, key);
    }
    spawn_stream(&app);
    let timer = Timer::new(&frame);
    let ticking = app.clone();
    timer.on_tick(move |_| drain(&ticking));
    timer.start(200, false);
    std::mem::forget(timer);
    frame.show(true);
    frame.layout();
    panel.layout();
    frame.centre();
    reveal_list(&app.borrow().posts);
    app.borrow().toot.set_focus();
}

fn install_menus(frame: &Frame, supports_collections: bool) {
    let client = Menu::builder().with_title("Client").build();
    client.append(SETTINGS, "&Settings...\tAlt-S", "Open Settings", ItemKind::Normal);
    client.append(FILTERS, "Notification &Filters...", "Choose which notifications are accepted, filtered, or dropped", ItemKind::Normal);
    let view = Menu::builder().with_title("View").build();
    view.append(REFRESH, "&Refresh\tF5", "Reload current timeline", ItemKind::Normal);
    view.append_separator();
    view.append(AVATARS, "Show Profile Pictures", "Toggle display of profile pictures", ItemKind::Check);
    view.append_separator();
    view.append(FIND, "&Find in Timeline...", "Search within the current timeline", ItemKind::Normal);
    view.append_separator();
    view.append(SCHEDULED, "&Scheduled Posts...", "View and manage scheduled posts", ItemKind::Normal);
    view.append_separator();
    view.append(EXPLORE, "E&xplore/Discover...\tCtrl+T", "Browse trending posts, hashtags, and links", ItemKind::Normal);
    view.append(LISTS, "&Lists...", "Manage and view lists", ItemKind::Normal);
    view.append(COLLECTIONS, "&Collections...", "Manage and view account collections", ItemKind::Normal);
    view.enable_item(COLLECTIONS, supports_collections);
    view.append(HASHTAGS, "Followed &Hashtags...", "View and manage followed hashtags", ItemKind::Normal);
    view.append_separator();
    view.append(FAVOURITES_TL, "Fav&ourites\tCtrl+Alt+K", "View your favourited posts", ItemKind::Normal);
    view.append(BOOKMARKS_TL, "Boo&kmarks\tCtrl+Alt+B", "View your bookmarked posts", ItemKind::Normal);
    view.append_separator();
    view.append(FOLLOWERS, "My &Followers\tCtrl+[", "View your followers", ItemKind::Normal);
    view.append(FOLLOWING, "My F&ollowing\tCtrl+]", "View who you follow", ItemKind::Normal);
    view.append(BLOCKED, "&Blocked Users", "View blocked users", ItemKind::Normal);
    view.append(MUTED_USERS, "M&uted Users", "View muted users", ItemKind::Normal);
    view.append(REQUESTS, "Follow &Requests", "View pending follow requests", ItemKind::Normal);
    view.append_separator();
    view.append(EDIT_PROFILE, "Edit &My Profile...", "Edit your display name, bio, and avatar", ItemKind::Normal);
    view.append_separator();
    view.append(INSTANCE, "&Instance Info...\tCtrl+I", "View information about the Mastodon instance", ItemKind::Normal);
    let actions = Menu::builder().with_title("Actions").build();
    actions.append(COMPOSE, "Compose &New Post\tCtrl+N", "", ItemKind::Normal);
    actions.append(REPLY, "&Reply\tCtrl+R", "", ItemKind::Normal);
    actions.append(QUOTE, "&Quote\tCtrl+Q", "", ItemKind::Normal);
    actions.append(BOOST, "&Boost\tCtrl+Shift+R", "", ItemKind::Normal);
    actions.append(FAVOURITE, "&Favourite\tCtrl+F", "", ItemKind::Normal);
    actions.append(BOOKMARK, "Boo&kmark\tAlt+B", "", ItemKind::Normal);
    actions.append_separator();
    actions.append(COPY, "&Copy Post Text\tCtrl+C", "", ItemKind::Normal);
    actions.append(OPEN_URL, "&Open Post on Web\tAlt+W", "", ItemKind::Normal);
    actions.append(VIEW_POST, "&View Post Details\tAlt+V", "", ItemKind::Normal);
    actions.append(THREAD, "Get &Thread/Conversation\tCtrl+G", "", ItemKind::Normal);
    actions.append_separator();
    actions.append(FOLLOW, "Fo&llow User\tCtrl+L", "", ItemKind::Normal);
    actions.append(BLOCK, "B&lock User\tCtrl+B", "", ItemKind::Normal);
    actions.append(MUTE_USER, "&Mute/Unmute User", "", ItemKind::Normal);
    actions.append_separator();
    actions.append(EDIT, "&Edit Post\tCtrl+E", "", ItemKind::Normal);
    actions.append(PIN, "&Pin/Unpin Post", "", ItemKind::Normal);
    actions.append_separator();
    actions.append(MUTE_CONVO, "Mute Con&versation", "", ItemKind::Normal);
    actions.append(REPORT, "&Report Post/User...", "", ItemKind::Normal);
    actions.append_separator();
    actions.append(MEDIA, "View M&edia Attachments", "", ItemKind::Normal);
    actions.append(BOOSTERS, "View &Who Boosted", "", ItemKind::Normal);
    actions.append(FAVOURITERS, "View Who Fav&ourited", "", ItemKind::Normal);
    actions.append(HISTORY, "View Edit &History", "", ItemKind::Normal);
    actions.append_separator();
    actions.append(PROFILE, "View &User Profile\tCtrl+Shift+U", "", ItemKind::Normal);
    actions.append(USER_COLLECTIONS, "View User Co&llections", "", ItemKind::Normal);
    actions.enable_item(USER_COLLECTIONS, supports_collections);
    actions.append(SEARCH, "&Search\tCtrl+/", "", ItemKind::Normal);
    actions.append(USER_TIMELINE, "Open User Time&line\tCtrl+U", "", ItemKind::Normal);
    actions.append(DM, "Send &Direct Message\tCtrl+D", "", ItemKind::Normal);
    let bar = MenuBar::builder()
        .append(client, "&Client")
        .append(view, "&View")
        .append(actions, "&Actions")
        .build();
    frame.set_menu_bar(bar);
}

fn wire(app: &Rc<RefCell<App>>, exit_button: Button) {
    let frame = app.borrow().frame;
    let commands = app.clone();
    frame.on_menu_selected(move |event| run(&commands, event.get_id()));
    let commands = app.clone();
    frame.on_char_hook(move |event| {
        if let WindowEventData::Keyboard(event) = event {
            on_key(&commands, &event);
        }
    });
    let exit_app = app.clone();
    exit_button.on_click(move |_| exit_app.borrow().frame.close(false));

    let watched = app.clone();
    watch_text(&watched, app.borrow().toot);
    watch_text(&watched, app.borrow().cw_input);
    watch_text(&watched, app.borrow().alt_input);
    watch_text(&watched, app.borrow().schedule_date);
    watch_text(&watched, app.borrow().schedule_time);
    for input in &app.borrow().poll_inputs {
        watch_text(&watched, *input);
    }
    let posts_focus = app.clone();
    app.borrow().posts.on_key_down(move |event| {
        posts_focus.borrow().focus.store(2, Ordering::Relaxed);
        if event.get_key_code() == Some(WXK_DELETE) {
            delete_selected(&posts_focus);
        }
    });

    let toggled = app.clone();
    app.borrow().cw_toggle.on_toggled(move |_| toggle_cw(&toggled));
    let toggled = app.clone();
    app.borrow().poll_toggle.on_toggled(move |_| toggle_poll(&toggled));
    let toggled = app.clone();
    app.borrow().media_toggle.on_toggled(move |_| toggle_media(&toggled));
    let toggled = app.clone();
    app.borrow().schedule_toggle.on_toggled(move |_| toggle_schedule(&toggled));
    let posting = app.clone();
    app.borrow().post_button.on_click(move |_| post(&posting));
    let adding = app.clone();
    app.borrow().add_media.on_click(move |_| add_media(&adding));
    let removing = app.clone();
    app.borrow().remove_media.on_click(move |_| remove_media(&removing));
    let selecting = app.clone();
    app.borrow().media_list.on_selection_changed(move |_| show_alt(&selecting));
    let editing = app.clone();
    app.borrow().alt_input.on_text_updated(move |_| store_alt(&editing));

    let tree_app = app.clone();
    app.borrow().tree.on_selection_changed(move |_| show_cached(&tree_app));
    let selected = app.clone();
    app.borrow().posts.on_item_selected(move |_| on_row_selected(&selected));
    let activated = app.clone();
    app.borrow().posts.on_item_activated(move |_| dialogs::post_details(&activated));
    let menu_app = app.clone();
    app.borrow().posts.on_item_right_click(move |_| show_context_menu(&menu_app));
}

fn watch_text(app: &Rc<RefCell<App>>, text: TextCtrl) {
    let entered = app.clone();
    text.on_set_focus(move |_| entered.borrow().focus.store(1, Ordering::Relaxed));
    let left = app.clone();
    text.on_kill_focus(move |_| left.borrow().focus.store(0, Ordering::Relaxed));
}

fn run(app: &Rc<RefCell<App>>, id: i32) {
    match id {
        SETTINGS => dialogs::settings(app),
        FILTERS => dialogs::notification_policy(app),
        REFRESH => refresh(app),
        AVATARS => toggle_avatars(app),
        FIND => dialogs::find(app),
        SCHEDULED => dialogs::scheduled(app),
        EXPLORE => dialogs::explore(app),
        LISTS => dialogs::lists(app),
        COLLECTIONS => dialogs::collections(app),
        HASHTAGS => dialogs::hashtags(app),
        FAVOURITES_TL => {
            sounds::play("favoritessnd");
            select_key(app, "favourites");
        }
        BOOKMARKS_TL => select_key(app, "bookmarks"),
        FOLLOWERS => dialogs::account_list(app, "My Followers", "followers"),
        FOLLOWING => dialogs::account_list(app, "My Following", "following"),
        BLOCKED => dialogs::account_list(app, "Blocked Users", "blocks"),
        MUTED_USERS => dialogs::account_list(app, "Muted Users", "mutes"),
        REQUESTS => dialogs::follow_requests(app),
        EDIT_PROFILE => dialogs::edit_profile(app),
        INSTANCE => dialogs::instance_info(app),
        COMPOSE => app.borrow().toot.set_focus(),
        REPLY => reply(app),
        QUOTE => quote(app),
        BOOST => {
            boost(app);
        }
        FAVOURITE => {
            favourite(app);
        }
        BOOKMARK => {
            bookmark(app);
        }
        COPY => copy_post(app),
        OPEN_URL => open_post(app),
        VIEW_POST => dialogs::post_details(app),
        THREAD => thread(app),
        FOLLOW => relationship_toggle(app, "following", "follow", "unfollow", "Follow"),
        BLOCK => relationship_confirm(app, "blocking", "block", "unblock", "Block"),
        MUTE_USER => relationship_toggle(app, "muting", "mute", "unmute", "Mute"),
        EDIT => edit_post(app),
        PIN => pin_post(app),
        MUTE_CONVO => mute_conversation(app),
        REPORT => dialogs::report(app),
        MEDIA => dialogs::media(app),
        BOOSTERS => dialogs::who(app, "reblogged_by", "Boosted by", "No one has boosted this post yet.", "Boosters"),
        FAVOURITERS => dialogs::who(app, "favourited_by", "Favourited by", "No one has favourited this post yet.", "Favouriters"),
        HISTORY => dialogs::history(app),
        PROFILE => dialogs::profile_from_selection(app),
        USER_COLLECTIONS => dialogs::user_collections(app),
        SEARCH => search(app),
        USER_TIMELINE => user_timeline(app),
        DM => direct_message(app),
        DISMISS => dismiss_notification(app),
        CLEAR_NOTIFICATIONS => clear_notifications(app),
        ACCEPT => follow_request_action(app, "authorize", "Accepted"),
        REJECT => follow_request_action(app, "reject", "Rejected"),
        QUOTE_PERMISSIONS => quote_permissions(app),
        VIEW_QUOTES => dialogs::quotes(app),
        REVOKE_QUOTE => revoke_quote(app),
        _ => {}
    }
}

fn on_key(app: &Rc<RefCell<App>>, event: &KeyboardEvent) {
    let code = event.get_key_code().unwrap_or(0);
    let ctrl = event.control_down();
    let shift = event.shift_down();
    let alt = event.alt_down();
    let typing = app.borrow().focus.load(Ordering::Relaxed) == 1;
    // Menu items already carry these shortcuts. Handling them here as well
    // ran the action twice, so the sound and the announcement both doubled.
    if code == WXK_RETURN && typing && (ctrl || shift || alt) {
        post(app);
        return;
    }
    if !typing && ctrl && shift && !alt {
        let letter = if (b'a' as i32..=b'z' as i32).contains(&code) { code - 32 } else { code };
        if letter == b'F' as i32 {
            favourite(app);
            return;
        }
        if letter == b'L' as i32 {
            relationship_toggle(app, "following", "follow", "unfollow", "Follow");
            return;
        }
        if letter == b'B' as i32 {
            relationship_confirm(app, "blocking", "block", "unblock", "Block");
            return;
        }
    }
    if !typing && alt && !ctrl && !shift {
        let letter = if (b'a' as i32..=b'z' as i32).contains(&code) { code - 32 } else { code };
        if letter == b'L' as i32 {
            select_key(app, "local");
            return;
        }
        if letter == b'P' as i32 {
            select_key(app, "federated");
            return;
        }
        if code == WXK_PAGEUP {
            load_older(app);
            return;
        }
    }
    if !typing && alt && shift && !ctrl && (code == b'B' as i32 || code == b'b' as i32) {
        bookmark(app);
        return;
    }
    if !typing && ctrl && !shift && !alt && code == WXK_NUMPAD_DIVIDE {
        search(app);
        return;
    }
    event.event.skip(true);
}

fn toggle_cw(app: &Rc<RefCell<App>>) {
    let show = app.borrow().cw_toggle.is_checked();
    let state = app.borrow();
    state.cw_input.show(show);
    state.cw_label.show(show);
    state.frame.layout();
}

fn toggle_poll(app: &Rc<RefCell<App>>) {
    let show = app.borrow().poll_toggle.is_checked();
    let state = app.borrow();
    for input in &state.poll_inputs {
        input.show(show);
    }
    for label in &state.poll_labels {
        label.show(show);
    }
    state.duration_label.show(show);
    state.poll_duration.show(show);
    state.poll_multiple.show(show);
    if let Some(poll_box) = state.poll_box {
        poll_box.show(show);
    }
    state.frame.layout();
}

fn toggle_media(app: &Rc<RefCell<App>>) {
    let state = app.borrow();
    let show = state.media_toggle.is_checked();
    state.add_media.show(show);
    state.add_media.enable(show);
    if let Some(media_box) = state.media_box {
        media_box.show(show);
    }
    let has_files = show && !state.media_files.is_empty();
    state.media_list.show(has_files);
    state.remove_media.show(has_files);
    state.alt_label.show(has_files);
    state.alt_input.show(has_files);
    state.frame.layout();
}

fn toggle_schedule(app: &Rc<RefCell<App>>) {
    let show = app.borrow().schedule_toggle.is_checked();
    let state = app.borrow();
    state.schedule_date.show(show);
    state.schedule_time.show(show);
    state.schedule_date_label.show(show);
    state.schedule_time_label.show(show);
    state.frame.layout();
}

fn post(app: &Rc<RefCell<App>>) {
    let (text, spoiler, visibility, language, quote_policy, poll, files, scheduled) = {
        let state = app.borrow();
        let text = state.toot.get_value().trim().to_string();
        let spoiler = if state.cw_toggle.is_checked() {
            let value = state.cw_input.get_value();
            let value = value.trim().to_string();
            if value.is_empty() { None } else { Some(value) }
        } else {
            None
        };
        let visibility = PRIVACY_VALUES[state.privacy.get_selection().unwrap_or(0) as usize].to_string();
        let language_index = state.language.get_selection().unwrap_or(0) as usize;
        let language = LANGUAGES.get(language_index).map(|(_, code)| (*code).to_string()).filter(|code| !code.is_empty());
        let quote_policy = if state.supports_quotes {
            QUOTE_POLICY_VALUES.get(state.quote_policy.get_selection().unwrap_or(0) as usize).copied().flatten()
        } else {
            None
        };
        let poll = if state.poll_toggle.is_checked() {
            let options = state
                .poll_inputs
                .iter()
                .map(|input| input.get_value().trim().to_string())
                .filter(|option| !option.is_empty())
                .collect::<Vec<_>>();
            if options.len() < 2 {
                drop(state);
                error_box(&app.borrow().frame, "A poll must have at least two options.", "Poll Error");
                return;
            }
            Some((
                options,
                POLL_SECONDS[state.poll_duration.get_selection().unwrap_or(5) as usize],
                state.poll_multiple.is_checked(),
            ))
        } else {
            None
        };
        let files = state.media_files.clone();
        let scheduled = if state.schedule_toggle.is_checked() {
            Some((state.schedule_date.get_value(), state.schedule_time.get_value()))
        } else {
            None
        };
        (text, spoiler, visibility, language, quote_policy, poll, files, scheduled)
    };
    if text.is_empty() && poll.is_none() && files.is_empty() {
        sounds::play("errorsnd");
        error_box(&app.borrow().frame, "Cannot post empty status.", "Error");
        return;
    }
    let scheduled_at = if let Some((date, time)) = scheduled {
        let date = date.trim().to_string();
        let time = time.trim().to_string();
        if date.is_empty() || time.is_empty() {
            error_box(&app.borrow().frame, "Please enter both date (YYYY-MM-DD) and time (HH:MM) for scheduling.", "Schedule Error");
            return;
        }
        if !valid_schedule(&date, &time) {
            error_box(&app.borrow().frame, "Invalid date/time format. Use YYYY-MM-DD and HH:MM.", "Schedule Error");
            return;
        }
        Some(format!("{date}T{time}:00Z"))
    } else {
        None
    };
    let frame = app.borrow().frame;
    let api = app.borrow().api.clone();
    let mut media_ids = Vec::new();
    for (path, alt) in &files {
        match api.upload_media(std::path::Path::new(path), alt) {
            Ok(media) => {
                if let Some(id) = media.get("id").and_then(Value::as_str) {
                    media_ids.push(id.to_string());
                }
            }
            Err(error) => {
                sounds::play("errorsnd");
                error_box(&frame, &format!("Error: {error}"), "Post Error");
                return;
            }
        }
    }
    let mut fields = vec![("status".to_string(), text), ("visibility".to_string(), visibility)];
    if let Some(spoiler) = spoiler {
        fields.push(("spoiler_text".to_string(), spoiler));
    }
    if let Some(language) = language {
        fields.push(("language".to_string(), language));
    }
    if let Some(policy) = quote_policy {
        fields.push(("quote_approval_policy".to_string(), policy.to_string()));
    }
    if let Some(scheduled_at) = scheduled_at.clone() {
        fields.push(("scheduled_at".to_string(), scheduled_at));
    }
    if let Some((options, seconds, multiple)) = poll {
        for option in options {
            fields.push(("poll[options][]".to_string(), option));
        }
        fields.push(("poll[expires_in]".to_string(), seconds.to_string()));
        fields.push(("poll[multiple]".to_string(), multiple.to_string()));
    }
    for id in media_ids {
        fields.push(("media_ids[]".to_string(), id));
    }
    if let Err(error) = api.status_post(fields) {
        sounds::play("errorsnd");
        error_box(&frame, &format!("Error: {error}"), "Post Error");
        return;
    }
    if let Some(when) = scheduled_at {
        alert(&frame, &format!("Post scheduled for {when} UTC."), "Scheduled");
    } else {
        sounds::play("tootsnd");
        app.borrow().announcer.say("Post sent");
    }
    let state = app.borrow();
    state.toot.set_value("");
    state.cw_input.set_value("");
    state.cw_toggle.set_value(false);
    state.media_list.clear();
    state.alt_input.set_value("");
    state.media_toggle.set_value(false);
    state.schedule_toggle.set_value(false);
    state.schedule_date.set_value("");
    state.schedule_time.set_value("");
    state.poll_toggle.set_value(false);
    for input in &state.poll_inputs {
        input.set_value("");
    }
    state.poll_duration.set_selection(5);
    state.poll_multiple.set_value(false);
    drop(state);
    toggle_cw(app);
    toggle_media(app);
    toggle_schedule(app);
    toggle_poll(app);
    app.borrow_mut().media_files.clear();
}

fn valid_schedule(date: &str, time: &str) -> bool {
    let mut dates = date.split('-');
    let year = dates.next().and_then(|value| value.parse::<i32>().ok());
    let month = dates.next().and_then(|value| value.parse::<u32>().ok());
    let day = dates.next().and_then(|value| value.parse::<u32>().ok());
    let mut times = time.split(':');
    let hour = times.next().and_then(|value| value.parse::<u32>().ok());
    let minute = times.next().and_then(|value| value.parse::<u32>().ok());
    matches!(
        (year, month, day, hour, minute, dates.next(), times.next()),
        (Some(year), Some(1..=12), Some(1..=31), Some(0..=23), Some(0..=59), None, None) if year > 2000
    )
}

fn add_media(app: &Rc<RefCell<App>>) {
    if app.borrow().media_files.len() >= 4 {
        alert(&app.borrow().frame, "Maximum 4 media attachments allowed.", "Limit Reached");
        return;
    }
    let frame = app.borrow().frame;
    let dialog = FileDialog::builder(&frame)
        .with_message("Choose media file")
        .with_wildcard("Media files (*.png;*.jpg;*.jpeg;*.gif;*.webp;*.mp4;*.webm;*.mp3;*.ogg;*.wav)|*.png;*.jpg;*.jpeg;*.gif;*.webp;*.mp4;*.webm;*.mp3;*.ogg;*.wav|All files (*.*)|*.*")
        .with_style(FileDialogStyle::Open)
        .build();
    if dialog.show_modal() == ID_OK
        && let Some(path) = dialog.get_path()
    {
        let name = std::path::Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&path)
            .to_string();
        let mut state = app.borrow_mut();
        state.media_files.push((path, String::new()));
        state.media_list.append(&name);
        let index = state.media_list.get_count().saturating_sub(1);
        state.media_list.set_selection(index, true);
        drop(state);
        toggle_media(app);
    }
}

fn remove_media(app: &Rc<RefCell<App>>) {
    let Some(index) = app.borrow().media_list.get_selection() else {
        return;
    };
    let mut state = app.borrow_mut();
    if (index as usize) < state.media_files.len() {
        state.media_files.remove(index as usize);
        state.media_list.delete(index);
        state.alt_input.set_value("");
    }
    drop(state);
    toggle_media(app);
}

fn show_alt(app: &Rc<RefCell<App>>) {
    let state = app.borrow();
    let Some(index) = state.media_list.get_selection() else {
        return;
    };
    let text = state.media_files.get(index as usize).map(|(_, alt)| alt.clone()).unwrap_or_default();
    state.alt_input.set_value(&text);
}

fn store_alt(app: &Rc<RefCell<App>>) {
    let state = app.borrow();
    let Some(index) = state.media_list.get_selection() else {
        return;
    };
    let text = state.alt_input.get_value();
    drop(state);
    if let Some(file) = app.borrow_mut().media_files.get_mut(index as usize) {
        file.1 = text;
    }
}

pub(crate) fn begin_load(app: &Rc<RefCell<App>>, key: &str) {
    let generation = {
        let mut state = app.borrow_mut();
        let generation = state.next_load;
        state.next_load += 1;
        state.loads.insert(key.to_string(), generation);
        if selected_key(&state).as_deref() == Some(key) {
            state.posts.delete_all_items();
            state.row_avatars.clear();
        }
        generation
    };
    let api = app.borrow().api.clone();
    let tx = app.borrow().tx.clone();
    let key = key.to_string();
    std::thread::spawn(move || {
        let result = api.load_timeline(&key, None);
        let _ = tx.send(UiMsg::Timeline { key, generation, result });
    });
}

fn load_older(app: &Rc<RefCell<App>>) {
    let (key, max_id, api, tx) = {
        let state = app.borrow();
        let Some(key) = selected_key(&state) else {
            return;
        };
        let Some(last) = state.timelines.get(&key).and_then(|items| items.last()) else {
            return;
        };
        let id = last.get("id").and_then(Value::as_str).unwrap_or("").to_string();
        if id.is_empty() {
            return;
        }
        (key, id, state.api.clone(), state.tx.clone())
    };
    std::thread::spawn(move || {
        let result = api.load_timeline(&key, Some(&max_id)).map(|items| (true, items));
        let _ = tx.send(UiMsg::Timeline {
            key,
            generation: 0,
            result: result.map(|(_, items)| items),
        });
    });
}

fn refresh(app: &Rc<RefCell<App>>) {
    if let Some(key) = selected_key(&app.borrow()) {
        begin_load(app, &key);
    }
}

fn toggle_avatars(app: &Rc<RefCell<App>>) {
    let enabled = !app.borrow().show_avatars;
    {
        let mut state = app.borrow_mut();
        state.show_avatars = enabled;
        if enabled {
            ensure_avatar_list(&state.posts);
        }
        if let Some(bar) = state.frame.get_menu_bar() {
            bar.check_item(AVATARS, enabled);
        }
    }
    show_cached(app);
}

fn show_cached(app: &Rc<RefCell<App>>) {
    refill(&mut app.borrow_mut());
}

fn refill(state: &mut App) {
    state.posts.delete_all_items();
    state.row_avatars.clear();
    let Some(key) = selected_key(state) else {
        return;
    };
    let items = state.timelines.get(&key).cloned().unwrap_or_default();
    let notifications = key == "notifications";
    for item in items {
        let (row, avatar) = if notifications {
            row_from_notification(&item, state.show_avatars)
        } else {
            match row_from_status(&item, state.show_avatars) {
                Some(row) => row,
                None => continue,
            }
        };
        append_row(state, &row, avatar);
    }
    reveal_list(&state.posts);
}

fn append_row(state: &mut App, row: &[String], avatar: Option<String>) {
    let image = avatar_image(state, avatar.as_deref());
    let index = state.posts.insert_item(
        state.posts.get_item_count() as i64,
        row.first().map(String::as_str).unwrap_or(""),
        image,
    );
    for (column, text) in row.iter().enumerate().skip(1) {
        state.posts.set_item_text_by_column(index as i64, column as i32, text);
    }
    paint_row(&state.posts, index as i64);
    if let Some(url) = avatar.clone() {
        if let Some(image) = state.avatar_ids.get(&url).copied() {
            state.posts.set_item_image(index as i64, image);
        } else {
            queue_avatar(state, url);
        }
    }
    state.row_avatars.push(avatar);
}

fn avatar_image(state: &App, url: Option<&str>) -> Option<i32> {
    let url = url?;
    Some(state.avatar_ids.get(url).copied().unwrap_or(0))
}

fn ensure_avatar_list(posts: &ListCtrl) {
    if posts.get_image_list(image_list_type::SMALL).is_some() {
        return;
    }
    let images = ImageList::new(48, 48, true, 8);
    let mut pixels = vec![0u8; 48 * 48 * 4];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    if let Some(bitmap) = Bitmap::from_rgba(&pixels, 48, 48) {
        images.add_bitmap(&bitmap);
    }
    posts.set_image_list(images, image_list_type::SMALL);
}

fn queue_avatar(state: &mut App, url: String) {
    if state.avatar_ids.contains_key(&url) || !state.pending_avatars.insert(url.clone()) {
        return;
    }
    let tx = state.tx.clone();
    std::thread::spawn(move || {
        let pixels = fetch_avatar(&url);
        let _ = tx.send(UiMsg::Avatar { url, pixels });
    });
}

fn fetch_avatar(url: &str) -> Option<Vec<u8>> {
    let client = reqwest::blocking::Client::builder().user_agent("Thrive").build().ok()?;
    let bytes = client.get(url).send().ok()?.bytes().ok()?;
    let image = image::load_from_memory(&bytes).ok()?;
    Some(image.resize_exact(48, 48, image::imageops::FilterType::Triangle).to_rgba8().into_raw())
}

fn paint_row(posts: &ListCtrl, index: i64) {
    if let Some((background, foreground)) = theme::list_colours() {
        posts.set_item_background_colour(index, &background);
        posts.set_item_text_colour(index, &foreground);
    }
}

fn reveal_list(posts: &ListCtrl) {
    let width = posts.get_column_width(0);
    if width > 0 {
        posts.set_column_width(0, width);
    }
    posts.refresh(false, None);
}

fn insert_row(state: &mut App, row: &[String], avatar: Option<String>) {
    let image = avatar_image(state, avatar.as_deref());
    let index = state.posts.insert_item(0, row.first().map(String::as_str).unwrap_or(""), image);
    for (column, text) in row.iter().enumerate().skip(1) {
        state.posts.set_item_text_by_column(index as i64, column as i32, text);
    }
    paint_row(&state.posts, index as i64);
    if let Some(url) = avatar.clone()
        && !state.avatar_ids.contains_key(&url)
    {
        queue_avatar(state, url);
    }
    state.row_avatars.insert(0, avatar);
    reveal_list(&state.posts);
}

fn drain(app: &Rc<RefCell<App>>) {
    let mut messages = Vec::new();
    while let Ok(message) = app.borrow().rx.try_recv() {
        messages.push(message);
    }
    for message in messages {
        match message {
            UiMsg::Timeline { key, generation, result } => apply_timeline(app, key, generation, result),
            UiMsg::Stream(value) => apply_stream(app, value),
            UiMsg::Avatar { url, pixels } => apply_avatar(app, url, pixels),
        }
    }
}

fn apply_timeline(app: &Rc<RefCell<App>>, key: String, generation: u64, result: Result<Vec<Value>, String>) {
    let mut state = app.borrow_mut();
    let older = generation == 0;
    if !older && state.loads.get(&key).copied() != Some(generation) {
        return;
    }
    match result {
        Ok(items) => {
            if older {
                state.timelines.entry(key.clone()).or_default().extend(items.clone());
            } else {
                let previous = state.timelines.get(&key).map(Vec::len).unwrap_or(0);
                if key.starts_with("search:") && items.len() > previous {
                    sounds::play("search_updatedsnd");
                }
                state.timelines.insert(key.clone(), items.clone());
            }
            if selected_key(&state).as_deref() == Some(key.as_str()) {
                if older {
                    let notifications = key == "notifications";
                    for item in items {
                        let (row, avatar) = if notifications {
                            row_from_notification(&item, state.show_avatars)
                        } else if let Some(row) = row_from_status(&item, state.show_avatars) {
                            row
                        } else {
                            continue;
                        };
                        append_row(&mut state, &row, avatar);
                    }
                    reveal_list(&state.posts);
                } else {
                    refill(&mut state);
                }
            }
        }
        Err(error) => {
            let frame = state.frame;
            drop(state);
            error_box(&frame, &format!("Failed to load timeline: {error}"), "Error");
        }
    }
}

fn apply_stream(app: &Rc<RefCell<App>>, value: Value) {
    let event = value.get("event").and_then(Value::as_str).unwrap_or("");
    let payload = value.get("payload").and_then(Value::as_str).unwrap_or("");
    let Ok(parsed) = serde_json::from_str::<Value>(payload) else {
        return;
    };
    let mut state = app.borrow_mut();
    match event {
        "update" => add_status(&mut state, parsed),
        "notification" => add_notification(&mut state, parsed),
        "status.update" => replace_status(&mut state, parsed),
        "delete" => {
            let id = parsed.as_str().unwrap_or("").to_string();
            remove_status(&mut state, &id);
        }
        _ => {}
    }
    reveal_list(&state.posts);
}

fn add_status(state: &mut App, status: Value) {
    let own = id_of(&state.me) == status.get("account").map(id_of).unwrap_or_default();
    if own {
        sounds::play("usersnd");
        push_front(state, "sent", status.clone());
    } else if status.get("visibility").and_then(Value::as_str) == Some("direct") {
        sounds::play("dmsnd");
    } else {
        sounds::play("newtootsnd");
    }
    push_front(state, "home", status.clone());
    if let Some(account_id) = status.get("account").map(id_of) {
        let key = format!("user:{account_id}");
        if state.nodes.iter().any(|(name, _)| name == &key) {
            push_front(state, &key, status);
        }
    }
}

fn add_notification(state: &mut App, notification: Value) {
    match notification.get("type").and_then(Value::as_str).unwrap_or("") {
        "favourite" | "reblog" | "follow" | "follow_request" | "status" | "quote" | "quoted_update" | "added_to_collection" | "collection_update" => {
            sounds::play("notificationsnd");
        }
        "mention" => {
            sounds::play("mentionsnd");
        }
        _ => {}
    }
    let selected = selected_key(state);
    state.timelines.entry("notifications".to_string()).or_default().insert(0, notification.clone());
    if selected.as_deref() == Some("notifications") {
        let (row, avatar) = row_from_notification(&notification, state.show_avatars);
        insert_row(state, &row, avatar);
    }
    if notification.get("type").and_then(Value::as_str) == Some("mention")
        && let Some(status) = notification.get("status").filter(|value| !value.is_null()).cloned()
    {
        push_front(state, "mentions", status);
    }
}

pub(crate) fn replace_status(state: &mut App, status: Value) {
    let id = id_of(&status);
    for key in ["home", "local", "federated", "sent", "mentions", "direct_messages", "favourites", "bookmarks"] {
        let Some(items) = state.timelines.get_mut(key) else {
            continue;
        };
        if let Some(slot) = items.iter_mut().find(|item| id_of(item) == id) {
            *slot = status.clone();
        }
    }
    if let Some(key) = selected_key(state)
        && key != "notifications"
        && let Some(index) = state.timelines.get(&key).and_then(|items| items.iter().position(|item| id_of(item) == id))
        && let Some((row, avatar)) = row_from_status(&status, state.show_avatars)
    {
        for (column, text) in row.iter().enumerate() {
            state.posts.set_item_text_by_column(index as i64, column as i32, text);
        }
        if let Some(avatar) = avatar {
            state.row_avatars.get_mut(index).map(|slot| *slot = Some(avatar.clone()));
            queue_avatar(state, avatar);
        }
    }
}

pub(crate) fn remove_status(state: &mut App, id: &str) {
    for key in ["home", "local", "federated", "sent", "mentions", "direct_messages", "favourites", "bookmarks"] {
        let Some(items) = state.timelines.get_mut(key) else {
            continue;
        };
        if let Some(index) = items.iter().position(|item| id_of(item) == id) {
            items.remove(index);
            if selected_key(state).as_deref() == Some(key) {
                state.posts.delete_item(index as i64);
                if index < state.row_avatars.len() {
                    state.row_avatars.remove(index);
                }
            }
        }
    }
}

fn push_front(state: &mut App, key: &str, status: Value) {
    let selected = selected_key(state).as_deref() == Some(key);
    state.timelines.entry(key.to_string()).or_default().insert(0, status.clone());
    if selected && let Some((row, avatar)) = row_from_status(&status, state.show_avatars) {
        insert_row(state, &row, avatar);
    }
}

fn apply_avatar(app: &Rc<RefCell<App>>, url: String, pixels: Option<Vec<u8>>) {
    let mut state = app.borrow_mut();
    state.pending_avatars.remove(&url);
    let Some(pixels) = pixels else {
        return;
    };
    let Some(bitmap) = Bitmap::from_rgba(&pixels, 48, 48) else {
        return;
    };
    ensure_avatar_list(&state.posts);
    let Some(images) = state.posts.get_image_list(image_list_type::SMALL) else {
        return;
    };
    let index = images.add_bitmap(&bitmap);
    if index < 0 {
        return;
    }
    state.avatar_ids.insert(url.clone(), index);
    for (row, avatar) in state.row_avatars.iter().enumerate() {
        if avatar.as_deref() == Some(url.as_str()) {
            state.posts.set_item_image(row as i64, index);
        }
    }
    reveal_list(&state.posts);
}

fn spawn_stream(app: &Rc<RefCell<App>>) {
    let url = app.borrow().api.stream_url();
    let tx = app.borrow().tx.clone();
    std::thread::spawn(move || {
        loop {
            if let Ok((mut socket, _)) = tungstenite::connect(&url) {
                while let Ok(message) = socket.read() {
                    match message {
                        tungstenite::Message::Text(text) => {
                            if let Ok(value) = serde_json::from_str::<Value>(text.as_str()) {
                                let _ = tx.send(UiMsg::Stream(value));
                            }
                        }
                        tungstenite::Message::Ping(payload) => {
                            let _ = socket.send(tungstenite::Message::Pong(payload));
                        }
                        tungstenite::Message::Close(_) => break,
                        _ => {}
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    });
}

pub(crate) fn selected_key(state: &App) -> Option<String> {
    let item = state.tree.get_selection()?;
    let data = state.tree.get_custom_data_direct(&item)?;
    data.downcast_ref::<String>().cloned()
}

pub(crate) fn selected_status(state: &App) -> Option<(Value, usize)> {
    let index = state.posts.get_first_selected_item();
    if index < 0 {
        return None;
    }
    let key = selected_key(state)?;
    let item = state.timelines.get(&key)?.get(index as usize)?.clone();
    let status = if key == "notifications" {
        item.get("status").filter(|value| !value.is_null()).cloned()?
    } else {
        item
    };
    Some((status, index as usize))
}

pub(crate) fn select_key(app: &Rc<RefCell<App>>, key: &str) {
    let item = app.borrow().nodes.iter().find(|(name, _)| name == key).map(|(_, item)| item.clone());
    if let Some(item) = item {
        app.borrow().tree.select_item(&item);
    }
}

pub(crate) fn ensure_timeline(app: &Rc<RefCell<App>>, key: &str, title: &str) {
    let exists = app.borrow().nodes.iter().any(|(name, _)| name == key);
    if !exists {
        let mut state = app.borrow_mut();
        if let Some(item) = state.tree.append_item_with_data(&state.root, title, key.to_string(), None, None) {
            state.nodes.push((key.to_string(), item));
            state.timelines.entry(key.to_string()).or_default();
            sounds::play("open_timelinesnd");
        }
    }
    select_key(app, key);
}

fn on_row_selected(app: &Rc<RefCell<App>>) {
    let state = app.borrow();
    let Some((status, _)) = selected_status(&state) else {
        return;
    };
    let source = source_status(&status);
    if source.get("poll").is_some_and(|value| !value.is_null()) {
        sounds::play("pollsnd");
    } else if source
        .get("mentions")
        .and_then(Value::as_array)
        .is_some_and(|mentions| mentions.iter().any(|mention| id_of(mention) == id_of(&state.me)))
    {
        sounds::play("select_mentionsnd");
    } else if let Some(attachments) = source.get("media_attachments").and_then(Value::as_array) {
        let types = attachments.iter().filter_map(|item| item.get("type").and_then(Value::as_str)).collect::<Vec<_>>();
        if types.iter().any(|kind| *kind == "video" || *kind == "gifv" || *kind == "audio") {
            sounds::play("mediasnd");
        } else if types.iter().any(|kind| *kind == "image") {
            sounds::play("imagesnd");
        }
    }
}

fn reply(app: &Rc<RefCell<App>>) {
    let Some((status, _)) = selected_status(&app.borrow()) else {
        return;
    };
    let source = source_status(&status);
    let me = app.borrow().me.get("acct").and_then(Value::as_str).unwrap_or("").to_string();
    let author = source.get("account").and_then(|account| account.get("acct")).and_then(Value::as_str).unwrap_or("");
    let mut mentions = Vec::new();
    if author != me {
        mentions.push(format!("@{author}"));
    }
    if let Some(list) = source.get("mentions").and_then(Value::as_array) {
        for mention in list {
            let acct = mention.get("acct").and_then(Value::as_str).unwrap_or("");
            if !acct.is_empty() && acct != me && acct != author {
                mentions.push(format!("@{acct}"));
            }
        }
    }
    compose_dialog(app, "Reply to Post", "&Reply", &format!("{} ", mentions.join(" ")), &status, true);
}

fn quote(app: &Rc<RefCell<App>>) {
    let Some((status, _)) = selected_status(&app.borrow()) else {
        return;
    };
    let source = source_status(&status);
    if source.get("visibility").and_then(Value::as_str) == Some("direct") {
        sounds::play("errorsnd");
        alert(&app.borrow().frame, "Private mentions cannot be quoted.", "Quote Not Allowed");
        return;
    }
    if let Some(current) = source.get("quote_approval").and_then(|value| value.get("current_user")).and_then(Value::as_str)
        && (current == "denied" || current == "unknown")
    {
        sounds::play("errorsnd");
        let followers = ["/quote_approval/automatic", "/quote_approval/manual"].iter().any(|path| {
            source
                .pointer(path)
                .and_then(Value::as_array)
                .is_some_and(|audiences| audiences.iter().any(|audience| audience.as_str() == Some("followers")))
        });
        let message = if followers { "Only followers can quote this post." } else { "You are not allowed to quote this post." };
        alert(&app.borrow().frame, message, "Quote Not Allowed");
        return;
    }
    compose_dialog(app, "Quote Post", "&Quote", "", &status, false);
}

fn compose_dialog(app: &Rc<RefCell<App>>, title: &str, label: &str, initial: &str, status: &Value, reply: bool) {
    let frame = app.borrow().frame;
    let (dialog, panel, sizer) = super::styled_dialog(&frame, title, 500, 300);
    let caption = StaticText::builder(&panel).with_label(label).build();
    let text = TextCtrl::builder(&panel)
        .with_value(initial)
        .with_style(TextCtrlStyle::MultiLine)
        .with_size(Size::new(480, 100))
        .build();
    text.set_insertion_point_end();
    let privacy_label = StaticText::builder(&panel).with_label("P&rivacy:").build();
    let quoted = source_status(status);
    let visibility = if reply { status } else { &quoted }.get("visibility").and_then(Value::as_str).unwrap_or("public");
    // A followers-only post may only be quoted privately.
    let (privacy_labels, privacy_values): (&[&str], &'static [&'static str]) = if !reply && visibility == "private" {
        (&["Followers-only", "Direct"], &["private", "direct"])
    } else {
        (PRIVACY_LABELS, PRIVACY_VALUES)
    };
    let selected = privacy_values.iter().position(|item| *item == visibility).unwrap_or(0) as u32;
    let privacy = Choice::builder(&panel)
        .with_choices(privacy_labels.iter().map(|item| (*item).to_string()).collect())
        .with_selection(Some(selected))
        .build();
    let manual = !reply && quoted.pointer("/quote_approval/current_user").and_then(Value::as_str) == Some("manual");
    let send = Button::builder(&panel).with_label(if manual { "&Request to Quote" } else { "&Post" }).build();
    let cancel = Button::builder(&panel).with_label("&Cancel").with_id(ID_CANCEL).build();
    theme::paint(&caption);
    theme::paint(&privacy_label);
    theme::paint(&text);
    theme::paint(&privacy);
    theme::paint(&send);
    theme::paint(&cancel);
    sizer.add(&caption, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 10);
    sizer.add(&text, 1, SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom | SizerFlag::Expand, 10);
    sizer.add(&privacy_label, 0, SizerFlag::Left | SizerFlag::Right, 10);
    sizer.add(&privacy, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom | SizerFlag::Expand, 10);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&send, 0, SizerFlag::All, 5);
    buttons.add(&cancel, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::Right | SizerFlag::Bottom, 10);
    panel.set_sizer(sizer, true);
    let api = app.borrow().api.clone();
    let announcer = app.borrow().announcer;
    let native_quote = app.borrow().supports_quotes || quoted.get("quote_approval").is_some_and(|value| !value.is_null());
    let author = quoted.pointer("/account/acct").and_then(Value::as_str).unwrap_or("").to_string();
    let status_id = id_of(&quoted);
    let status_url = quoted
        .get("url")
        .and_then(Value::as_str)
        .or_else(|| quoted.get("uri").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    let dialog_send = dialog;
    send.on_click(move |_| {
        let body = text.get_value().trim().to_string();
        if body.is_empty() {
            error_box(&dialog_send, if reply { "Reply cannot be empty." } else { "Quote text cannot be empty." }, "Error");
            return;
        }
        let visibility = privacy_values[privacy.get_selection().unwrap_or(0) as usize].to_string();
        if !reply && visibility == "direct" && !author.is_empty() {
            let mention = format!("@{author}");
            if !body.split_whitespace().any(|word| word == mention) {
                error_box(&dialog_send, &format!("A private quote must explicitly mention {mention}."), "Quote Not Allowed");
                return;
            }
        }
        let result = if reply {
            api.status_post(vec![
                ("status".to_string(), body),
                ("visibility".to_string(), visibility),
                ("in_reply_to_id".to_string(), status_id.clone()),
            ])
        } else if native_quote {
            api.status_post(vec![
                ("status".to_string(), body),
                ("visibility".to_string(), visibility),
                ("quoted_status_id".to_string(), status_id.clone()),
            ])
        } else {
            api.status_post(vec![
                ("status".to_string(), format!("{body}\n\n{status_url}")),
                ("visibility".to_string(), visibility),
            ])
        };
        match result {
            Ok(_) => {
                if reply {
                    sounds::play("replysnd");
                    announcer.say("Reply sent");
                } else {
                    sounds::play("tootsnd");
                    announcer.say("Quote posted");
                }
                dialog_send.end_modal(ID_OK);
            }
            Err(error) => error_box(&dialog_send, &format!("Error sending: {error}"), "Error"),
        }
    });
    dialog.show_modal();
    dialog.destroy();
}

pub(crate) fn boost(app: &Rc<RefCell<App>>) -> bool {
    let Some((mut status, index)) = selected_status(&app.borrow()) else {
        return false;
    };
    let id = id_of(&status);
    let api = app.borrow().api.clone();
    let frame = app.borrow().frame;
    let undo = status.get("reblogged").and_then(Value::as_bool).unwrap_or(false);
    let result = if undo {
        api.status_action(&id, "unreblog")
    } else {
        api.status_action(&id, "reblog")
    };
    match result {
        Ok(_) => {
            if !undo {
                sounds::play("boostsnd");
                app.borrow().announcer.say("Boosted");
            } else {
                app.borrow().announcer.say("Unboosted");
            }
            if let Some(flag) = status.get_mut("reblogged") {
                *flag = json!(!undo);
            }
            refresh_row(app, index, &status);
            true
        }
        Err(error) => {
            error_box(&frame, &format!("Error: {error}"), "Boost Error");
            false
        }
    }
}

pub(crate) fn favourite(app: &Rc<RefCell<App>>) -> bool {
    let Some((mut status, index)) = selected_status(&app.borrow()) else {
        return false;
    };
    let id = id_of(&status);
    let source = source_status(&status);
    let api = app.borrow().api.clone();
    let frame = app.borrow().frame;
    let undo = status.get("favourited").and_then(Value::as_bool).unwrap_or(false);
    let result = if undo {
        api.status_action(&id, "unfavourite")
    } else {
        api.status_action(&id, "favourite")
    };
    if let Err(error) = result {
        error_box(&frame, &format!("Error: {error}"), "Favourite Error");
        return false;
    }
    if undo {
        sounds::play("unfavsnd");
        app.borrow().announcer.say("Unfavourited");
        remove_from_timeline(app, "favourites", &id_of(&source));
    } else {
        sounds::play("favsnd");
        app.borrow().announcer.say("Favourited");
        let mut state = app.borrow_mut();
        let selected = selected_key(&state).as_deref() == Some("favourites");
        state.timelines.entry("favourites".to_string()).or_default().insert(0, source.clone());
        if selected && let Some((row, avatar)) = row_from_status(&source, state.show_avatars) {
            insert_row(&mut state, &row, avatar);
        }
    }
    if let Some(flag) = status.get_mut("favourited") {
        *flag = json!(!undo);
    }
    refresh_row(app, index, &status);
    true
}

pub(crate) fn bookmark(app: &Rc<RefCell<App>>) -> bool {
    let Some((mut status, index)) = selected_status(&app.borrow()) else {
        return false;
    };
    let source = source_status(&status);
    let id = id_of(&source);
    let undo = source.get("bookmarked").and_then(Value::as_bool).unwrap_or(false);
    let frame = app.borrow().frame;
    let result = if undo {
        app.borrow().api.status_action(&id, "unbookmark")
    } else {
        app.borrow().api.status_action(&id, "bookmark")
    };
    match result {
        Ok(_) => {
            alert(&frame, if undo { "Post unbookmarked." } else { "Post bookmarked." }, "Bookmark");
            if let Some(reblog) = status.get_mut("reblog").filter(|value| value.is_object()) {
                reblog["bookmarked"] = json!(!undo);
            } else {
                status["bookmarked"] = json!(!undo);
            }
            refresh_row(app, index, &status);
            true
        }
        Err(error) => {
            error_box(&frame, &format!("Error: {error}"), "Bookmark Error");
            false
        }
    }
}

fn copy_post(app: &Rc<RefCell<App>>) {
    let Some((status, _)) = selected_status(&app.borrow()) else {
        return;
    };
    copy_text(&status_plain_text(&source_status(&status)));
}

fn open_post(app: &Rc<RefCell<App>>) {
    let Some((status, _)) = selected_status(&app.borrow()) else {
        return;
    };
    let source = source_status(&status);
    let url = source.get("url").and_then(Value::as_str).or_else(|| source.get("uri").and_then(Value::as_str));
    match url {
        Some(url) => open_browser(url),
        None => error_box(&app.borrow().frame, "No URL available for this post.", "Error"),
    }
}

fn thread(app: &Rc<RefCell<App>>) {
    let Some((status, _)) = selected_status(&app.borrow()) else {
        return;
    };
    let source = source_status(&status);
    let id = id_of(&source);
    let frame = app.borrow().frame;
    match app.borrow().api.status_context(&id) {
        Ok(context) => {
            let mut thread = context.get("ancestors").and_then(Value::as_array).cloned().unwrap_or_default();
            thread.push(source.clone());
            if let Some(descendants) = context.get("descendants").and_then(Value::as_array) {
                thread.extend(descendants.clone());
            }
            let key = format!("thread:{id}");
            let author = source.get("account").map(crate::rows::display_name).unwrap_or_default();
            {
                let mut state = app.borrow_mut();
                state.timelines.insert(key.clone(), thread);
            }
            ensure_timeline(app, &key, &format!("Thread by {author}"));
        }
        Err(error) => error_box(&frame, &format!("Error loading thread: {error}"), "Thread Error"),
    }
}

fn relationship_toggle(app: &Rc<RefCell<App>>, flag: &str, on: &str, off: &str, title: &str) {
    let Some(account) = selected_account(app) else {
        return;
    };
    let frame = app.borrow().frame;
    let api = app.borrow().api.clone();
    let id = id_of(&account);
    let display = crate::rows::display_name(&account);
    let relationship = match api.relationships(&id) {
        Ok(value) => value.as_array().and_then(|items| items.first()).cloned().unwrap_or(json!({})),
        Err(error) => {
            sounds::play("errorsnd");
            error_box(&frame, &format!("Error: {error}"), &format!("{title} Error"));
            return;
        }
    };
    let active = relationship.get(flag).and_then(Value::as_bool).unwrap_or(false);
    if active && flag == "following" && !confirm(&frame, &format!("Unfollow {display}?"), "Confirm") {
        return;
    }
    let action = if active { off } else { on };
    if let Err(error) = api.account_action(&id, action) {
        sounds::play("errorsnd");
        error_box(&frame, &format!("Error: {error}"), &format!("{title} Error"));
        return;
    }
    if flag == "following" {
        sounds::play(if active { "unfollowsnd" } else { "followsnd" });
    }
    let message = if active {
        format!("Unfollowed {display}.")
    } else if flag == "following" {
        format!("Now following {display}.")
    } else if flag == "muting" {
        format!("Muted {display}.")
    } else {
        format!("{title} updated for {display}.")
    };
    if active && flag == "muting" {
        alert(&frame, &format!("Unmuted {display}."), title);
    } else {
        alert(&frame, &message, title);
    }
}

fn relationship_confirm(app: &Rc<RefCell<App>>, flag: &str, on: &str, off: &str, title: &str) {
    let Some(account) = selected_account(app) else {
        return;
    };
    let frame = app.borrow().frame;
    let api = app.borrow().api.clone();
    let id = id_of(&account);
    let display = crate::rows::display_name(&account);
    let relationship = match api.relationships(&id) {
        Ok(value) => value.as_array().and_then(|items| items.first()).cloned().unwrap_or(json!({})),
        Err(error) => {
            sounds::play("errorsnd");
            error_box(&frame, &format!("Error: {error}"), &format!("{title} Error"));
            return;
        }
    };
    let active = relationship.get(flag).and_then(Value::as_bool).unwrap_or(false);
    let question = if active {
        format!("Unblock {display}?")
    } else {
        format!("Block {display}? You won't see their posts.")
    };
    if !confirm(&frame, &question, "Confirm") {
        return;
    }
    let action = if active { off } else { on };
    match api.account_action(&id, action) {
        Ok(_) => alert(&frame, &format!("{} {display}.", if active { "Unblocked" } else { "Blocked" }), title),
        Err(error) => {
            sounds::play("errorsnd");
            error_box(&frame, &format!("Error: {error}"), &format!("{title} Error"));
        }
    }
}

fn selected_account(app: &Rc<RefCell<App>>) -> Option<Value> {
    let (status, _) = selected_status(&app.borrow())?;
    source_status(&status).get("account").cloned()
}

fn edit_post(app: &Rc<RefCell<App>>) {
    let Some((status, _)) = selected_status(&app.borrow()) else {
        return;
    };
    let source = source_status(&status);
    if id_of(source.get("account").unwrap_or(&Value::Null)) != id_of(&app.borrow().me) {
        error_box(&app.borrow().frame, "You can only edit your own posts.", "Edit Error");
        return;
    }
    let frame = app.borrow().frame;
    if let Some(updated) = edit_status(app, &frame, &source) {
        let mut state = app.borrow_mut();
        replace_status(&mut state, updated);
    }
}

pub(crate) fn edit_status(app: &Rc<RefCell<App>>, parent: &impl WxWidget, source: &Value) -> Option<Value> {
    let (dialog, panel, sizer) = super::styled_dialog(parent, "Edit Post", 500, 300);
    let label = StaticText::builder(&panel).with_label("&Edit Post").build();
    let text = TextCtrl::builder(&panel)
        .with_value(&status_plain_text(&source))
        .with_style(TextCtrlStyle::MultiLine)
        .with_size(Size::new(480, 150))
        .build();
    text.set_insertion_point_end();
    let spoiler_label = StaticText::builder(&panel).with_label("Content &Warning:").build();
    let spoiler = TextCtrl::builder(&panel)
        .with_value(source.get("spoiler_text").and_then(Value::as_str).unwrap_or(""))
        .build();
    let save = Button::builder(&panel).with_label("&Save").build();
    let cancel = Button::builder(&panel).with_label("&Cancel").with_id(ID_CANCEL).build();
    theme::paint(&label);
    theme::paint(&text);
    theme::paint(&spoiler_label);
    theme::paint(&spoiler);
    theme::paint(&save);
    theme::paint(&cancel);
    sizer.add(&label, 0, SizerFlag::All, 10);
    sizer.add(&text, 1, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 10);
    sizer.add(&spoiler_label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 10);
    sizer.add(&spoiler, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 10);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&save, 0, SizerFlag::All, 5);
    buttons.add(&cancel, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::Right | SizerFlag::Bottom, 10);
    panel.set_sizer(sizer, true);
    let api = app.borrow().api.clone();
    let id = id_of(source);
    let dialog_save = dialog;
    let updated = Rc::new(RefCell::new(None));
    let updated_save = updated.clone();
    save.on_click(move |_| {
        let body = text.get_value().trim().to_string();
        if body.is_empty() {
            error_box(&dialog_save, "Post cannot be empty.", "Error");
            return;
        }
        let warning = spoiler.get_value();
        let warning = warning.trim();
        match api.status_update(&id, &body, if warning.is_empty() { None } else { Some(warning) }) {
            Ok(status) => {
                *updated_save.borrow_mut() = Some(status);
                dialog_save.end_modal(ID_OK);
            }
            Err(error) => error_box(&dialog_save, &format!("Error editing post: {error}"), "Error"),
        }
    });
    dialog.show_modal();
    dialog.destroy();
    updated.borrow().clone()
}

fn pin_post(app: &Rc<RefCell<App>>) {
    let Some((status, _)) = selected_status(&app.borrow()) else {
        return;
    };
    let source = source_status(&status);
    let frame = app.borrow().frame;
    if id_of(source.get("account").unwrap_or(&Value::Null)) != id_of(&app.borrow().me) {
        error_box(&frame, "You can only pin your own posts.", "Pin Error");
        return;
    }
    let pinned = source.get("pinned").and_then(Value::as_bool).unwrap_or(false);
    let _ = set_pinned(app, &frame, &id_of(&source), pinned);
}

pub(crate) fn set_pinned(app: &Rc<RefCell<App>>, parent: &impl WxWidget, id: &str, pinned: bool) -> bool {
    let result = if pinned {
        app.borrow().api.status_action(id, "unpin")
    } else {
        app.borrow().api.status_action(id, "pin")
    };
    match result {
        Ok(updated) => {
            let mut state = app.borrow_mut();
            replace_status(&mut state, updated);
            drop(state);
            alert(parent, if pinned { "Post unpinned." } else { "Post pinned." }, "Pin");
            true
        }
        Err(error) => {
            error_box(parent, &format!("Error: {error}"), "Pin Error");
            false
        }
    }
}

fn mute_conversation(app: &Rc<RefCell<App>>) {
    let Some((status, _)) = selected_status(&app.borrow()) else {
        return;
    };
    let source = source_status(&status);
    let muted = source.get("muted").and_then(Value::as_bool).unwrap_or(false);
    let id = id_of(&source);
    let frame = app.borrow().frame;
    let result = if muted {
        app.borrow().api.status_action(&id, "unmute")
    } else {
        app.borrow().api.status_action(&id, "mute")
    };
    match result {
        Ok(_) => alert(
            &frame,
            if muted {
                "Conversation unmuted."
            } else {
                "Conversation muted. You will no longer receive notifications from this thread."
            },
            "Mute",
        ),
        Err(error) => error_box(&frame, &format!("Error: {error}"), "Mute Error"),
    }
}

fn search(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    let Some(query) = prompt(&frame, "Enter search query:", "Search") else {
        return;
    };
    let query = query.trim().to_string();
    if query.is_empty() {
        return;
    }
    if query.starts_with("http://") || query.starts_with("https://") {
        if let Ok(results) = app.borrow().api.search_v2(&query, &[("resolve", "true".to_string())]) {
            let collections = crate::api::collections_from_response(results.get("collections").unwrap_or(&Value::Null));
            if collections.len() == 1 {
                dialogs::show_collection(app, &collections[0], false);
                return;
            }
            if collections.len() > 1 {
                dialogs::choose_collection(app, &collections);
                return;
            }
        }
    }
    sounds::play("searchsnd");
    let key = format!("search:{query}");
    ensure_timeline(app, &key, &format!("Search: {query}"));
    begin_load(app, &key);
}

fn user_timeline(app: &Rc<RefCell<App>>) {
    let Some(account) = selected_account(app) else {
        return;
    };
    let id = id_of(&account);
    let name = crate::rows::display_name(&account);
    let acct = account.get("acct").and_then(Value::as_str).unwrap_or("");
    let key = format!("user:{id}");
    ensure_timeline(app, &key, &format!("{name} (@{acct})"));
    begin_load(app, &key);
}

fn direct_message(app: &Rc<RefCell<App>>) {
    let Some(account) = selected_account(app) else {
        return;
    };
    let display = crate::rows::display_name(&account);
    let acct = account.get("acct").and_then(Value::as_str).unwrap_or("");
    let frame = app.borrow().frame;
    let (dialog, panel, sizer) = super::styled_dialog(&frame, &format!("Direct Message to {display}"), 500, 300);
    let label = StaticText::builder(&panel)
        .with_label(&format!("Send a direct &message to {display} (@{acct}):"))
        .build();
    let text = TextCtrl::builder(&panel)
        .with_value(&format!("@{acct} "))
        .with_style(TextCtrlStyle::MultiLine)
        .build();
    text.set_insertion_point_end();
    let send = Button::builder(&panel).with_label("&Send").build();
    let cancel = Button::builder(&panel).with_label("&Cancel").with_id(ID_CANCEL).build();
    theme::paint(&label);
    theme::paint(&text);
    theme::paint(&send);
    theme::paint(&cancel);
    sizer.add(&label, 0, SizerFlag::All, 10);
    sizer.add(&text, 1, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 10);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    buttons.add(&send, 0, SizerFlag::All, 5);
    buttons.add(&cancel, 0, SizerFlag::All, 5);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight | SizerFlag::Right | SizerFlag::Bottom, 10);
    panel.set_sizer(sizer, true);
    let api = app.borrow().api.clone();
    let announcer = app.borrow().announcer;
    let dialog_send = dialog;
    send.on_click(move |_| {
        let body = text.get_value().trim().to_string();
        if body.is_empty() {
            error_box(&dialog_send, "Message cannot be empty.", "Error");
            return;
        }
        match api.status_post(vec![("status".to_string(), body), ("visibility".to_string(), "direct".to_string())]) {
            Ok(_) => {
                if !sounds::play("send_dmsnd") && !sounds::play("dmsnd") {
                    announcer.say("Direct message sent");
                }
                dialog_send.end_modal(ID_OK);
            }
            Err(error) => error_box(&dialog_send, &format!("Error: {error}"), "Error"),
        }
    });
    dialog.show_modal();
    dialog.destroy();
}

fn delete_selected(app: &Rc<RefCell<App>>) {
    let Some((status, _)) = selected_status(&app.borrow()) else {
        return;
    };
    if id_of(status.get("account").unwrap_or(&Value::Null)) != id_of(&app.borrow().me) {
        return;
    }
    let reblog = status.get("reblog").is_some_and(|value| !value.is_null());
    let frame = app.borrow().frame;
    let question = if reblog { "Are you sure you want to unboost this post?" } else { "Are you sure you want to delete this post?" };
    let title = if reblog { "Confirm Unboost" } else { "Confirm Deletion" };
    if !confirm(&frame, question, title) {
        return;
    }
    let id = id_of(&status);
    let result = if reblog {
        app.borrow().api.status_action(&id, "unreblog")
    } else {
        app.borrow().api.status_action(&id, "")
    };
    if let Err(error) = result {
        error_box(&frame, &format!("Error: {error}"), "Error");
    } else if !reblog {
        let mut state = app.borrow_mut();
        remove_status(&mut state, &id);
    }
}

fn dismiss_notification(app: &Rc<RefCell<App>>) {
    let state = app.borrow();
    let index = state.posts.get_first_selected_item();
    if index < 0 {
        return;
    }
    let Some(key) = selected_key(&state) else {
        return;
    };
    if key != "notifications" && key != "mentions" {
        return;
    }
    let Some(notification) = state.timelines.get(&key).and_then(|items| items.get(index as usize)).cloned() else {
        return;
    };
    let id = id_of(&notification);
    drop(state);
    if let Err(error) = app.borrow().api.dismiss_notification(&id) {
        error_box(&app.borrow().frame, &format!("Error: {error}"), "Error");
    }
}

fn clear_notifications(app: &Rc<RefCell<App>>) {
    let frame = app.borrow().frame;
    if !confirm(&frame, "Clear all notifications? This cannot be undone.", "Confirm") {
        return;
    }
    if let Err(error) = app.borrow().api.clear_notifications() {
        error_box(&frame, &format!("Error: {error}"), "Error");
        return;
    }
    let mut state = app.borrow_mut();
    state.timelines.insert("notifications".to_string(), Vec::new());
    if selected_key(&state).as_deref() == Some("notifications") {
        refill(&mut state);
    }
}

fn follow_request_action(app: &Rc<RefCell<App>>, action: &str, title: &str) {
    let account = {
        let state = app.borrow();
        let index = state.posts.get_first_selected_item();
        if index < 0 {
            None
        } else {
            selected_key(&state).and_then(|key| {
                state
                    .timelines
                    .get(&key)
                    .and_then(|items| items.get(index as usize))
                    .and_then(|item| notification_account(&key, item))
            })
        }
    };
    let Some(account) = account else {
        return;
    };
    let frame = app.borrow().frame;
    let name = crate::rows::display_name(&account);
    match app.borrow().api.follow_request(&id_of(&account), action) {
        Ok(_) => alert(&frame, &format!("{title} follow request from {name}."), "Follow Request"),
        Err(error) => error_box(&frame, &format!("Error: {error}"), "Error"),
    }
}

pub(crate) fn store_poll(app: &Rc<RefCell<App>>, status_id: &str, poll: &Value) {
    let mut state = app.borrow_mut();
    let current = selected_key(&state);
    let show_avatars = state.show_avatars;
    let keys: Vec<String> = state.timelines.keys().cloned().collect();
    for key in keys {
        let Some(items) = state.timelines.get_mut(&key) else {
            continue;
        };
        let mut rows = Vec::new();
        for (index, item) in items.iter_mut().enumerate() {
            if id_of(item) != status_id && id_of(&source_status(item)) != status_id {
                continue;
            }
            if let Some(reblog) = item.get_mut("reblog").filter(|value| value.is_object()) {
                reblog["poll"] = poll.clone();
            } else {
                item["poll"] = poll.clone();
            }
            if current.as_deref() == Some(key.as_str())
                && let Some((row, _)) = crate::rows::row_from_status(item, show_avatars)
            {
                rows.push((index, row));
            }
        }
        for (index, row) in rows {
            for (column, text) in row.iter().enumerate() {
                state.posts.set_item_text_by_column(index as i64, column as i32, text);
            }
        }
    }
}

fn refresh_row(app: &Rc<RefCell<App>>, index: usize, status: &Value) {
    let mut state = app.borrow_mut();
    let Some(key) = selected_key(&state) else {
        return;
    };
    if key == "notifications" {
        let show_avatars = state.show_avatars;
        let Some(notification) = state.timelines.get_mut(&key).and_then(|items| items.get_mut(index)) else {
            return;
        };
        notification["status"] = status.clone();
        let (row, _) = row_from_notification(notification, show_avatars);
        for (column, text) in row.iter().enumerate() {
            state.posts.set_item_text_by_column(index as i64, column as i32, text);
        }
        return;
    }
    if let Some(slot) = state.timelines.get_mut(&key).and_then(|items| items.get_mut(index)) {
        *slot = status.clone();
    }
    if let Some((row, _)) = row_from_status(status, state.show_avatars) {
        for (column, text) in row.iter().enumerate() {
            state.posts.set_item_text_by_column(index as i64, column as i32, text);
        }
    }
}

fn remove_from_timeline(app: &Rc<RefCell<App>>, key: &str, id: &str) {
    let mut state = app.borrow_mut();
    let Some(items) = state.timelines.get_mut(key) else {
        return;
    };
    if let Some(index) = items.iter().position(|item| id_of(item) == id || id_of(&source_status(item)) == id) {
        items.remove(index);
        if selected_key(&state).as_deref() == Some(key) {
            state.posts.delete_item(index as i64);
            if index < state.row_avatars.len() {
                state.row_avatars.remove(index);
            }
        }
    }
}

fn notification_account(key: &str, item: &Value) -> Option<Value> {
    if key == "notifications"
        && let Some(account) = item.get("account").filter(|value| value.is_object())
    {
        return Some(account.clone());
    }
    let status = if key == "notifications" {
        item.get("status").filter(|value| !value.is_null())?
    } else {
        item
    };
    source_status(status).get("account").cloned()
}

fn show_context_menu(app: &Rc<RefCell<App>>) {
    let index = app.borrow().posts.get_first_selected_item();
    if index < 0 {
        return;
    }
    let Some(key) = selected_key(&app.borrow()) else {
        return;
    };
    let Some(item) = app.borrow().timelines.get(&key).and_then(|items| items.get(index as usize)).cloned() else {
        return;
    };
    let status = if key == "notifications" {
        item.get("status").filter(|value| value.is_object()).cloned()
    } else {
        Some(item.clone())
    };
    let mut menu = Menu::builder().build();
    let Some(status) = status else {
        if item.get("type").and_then(Value::as_str) == Some("follow_request") {
            menu.append(ACCEPT, "&Accept Follow Request", "", ItemKind::Normal);
            menu.append(REJECT, "Re&ject Follow Request", "", ItemKind::Normal);
            menu.append_separator();
            menu.append(DISMISS, "D&ismiss Notification", "", ItemKind::Normal);
            menu.append(CLEAR_NOTIFICATIONS, "C&lear All Notifications", "", ItemKind::Normal);
            app.borrow().posts.popup_menu(&mut menu, None);
        }
        return;
    };
    let source = source_status(&status);
    menu.append(REPLY, "&Reply\tCtrl+R", "", ItemKind::Normal);
    menu.append(QUOTE, "&Quote\tCtrl+Q", "", ItemKind::Normal);
    let boost_label = if status.get("reblogged").and_then(Value::as_bool).unwrap_or(false) { "Un&boost\tCtrl+Shift+R" } else { "&Boost\tCtrl+Shift+R" };
    menu.append(BOOST, boost_label, "", ItemKind::Normal);
    let fav_label = if status.get("favourited").and_then(Value::as_bool).unwrap_or(false) { "Un&favourite\tCtrl+F" } else { "&Favourite\tCtrl+F" };
    menu.append(FAVOURITE, fav_label, "", ItemKind::Normal);
    let bookmark_label = if source.get("bookmarked").and_then(Value::as_bool).unwrap_or(false) { "Un&bookmark\tAlt+B" } else { "Boo&kmark\tAlt+B" };
    menu.append(BOOKMARK, bookmark_label, "", ItemKind::Normal);
    menu.append_separator();
    menu.append(COPY, "&Copy Post Text\tCtrl+C", "", ItemKind::Normal);
    menu.append(OPEN_URL, "&Open Post on Web\tAlt+W", "", ItemKind::Normal);
    menu.append(VIEW_POST, "&View Post Details\tAlt+V", "", ItemKind::Normal);
    menu.append(THREAD, "Get &Thread/Conversation\tCtrl+G", "", ItemKind::Normal);
    menu.append_separator();
    menu.append(FOLLOW, "Fo&llow User\tCtrl+L", "", ItemKind::Normal);
    menu.append(BLOCK, "B&lock User\tCtrl+B", "", ItemKind::Normal);
    menu.append(MUTE_USER, "&Mute/Unmute User", "", ItemKind::Normal);
    if id_of(source.get("account").unwrap_or(&Value::Null)) == id_of(&app.borrow().me) {
        menu.append_separator();
        menu.append(EDIT, "&Edit Post\tCtrl+E", "", ItemKind::Normal);
        let pin_label = if source.get("pinned").and_then(Value::as_bool).unwrap_or(false) { "Un&pin Post" } else { "&Pin Post" };
        menu.append(PIN, pin_label, "", ItemKind::Normal);
        let quotable = matches!(source.get("visibility").and_then(Value::as_str), Some("public" | "unlisted"));
        if app.borrow().supports_quotes && quotable {
            menu.append(QUOTE_PERMISSIONS, "Quote &Permissions...", "", ItemKind::Normal);
        }
    }
    menu.append_separator();
    if app.borrow().supports_quotes && source.get("quotes_count").and_then(Value::as_i64).unwrap_or(0) > 0 {
        menu.append(VIEW_QUOTES, "View Q&uotes", "", ItemKind::Normal);
    }
    menu.append(PROFILE, "View &User Profile\tCtrl+Shift+U", "", ItemKind::Normal);
    menu.append(USER_COLLECTIONS, "View User Co&llections", "", ItemKind::Normal);
    menu.enable_item(USER_COLLECTIONS, app.borrow().supports_collections);
    menu.append(USER_TIMELINE, "Open User Time&line\tCtrl+U", "", ItemKind::Normal);
    menu.append_separator();
    menu.append(MEDIA, "View M&edia Attachments", "", ItemKind::Normal);
    menu.append(BOOSTERS, "View &Who Boosted", "", ItemKind::Normal);
    menu.append(FAVOURITERS, "View Who Fav&ourited", "", ItemKind::Normal);
    menu.append(HISTORY, "View Edit &History", "", ItemKind::Normal);
    menu.append_separator();
    menu.append(MUTE_CONVO, "Mute Con&versation", "", ItemKind::Normal);
    menu.append(REPORT, "&Report Post/User...", "", ItemKind::Normal);
    menu.append(DM, "Send &Direct Message\tCtrl+D", "", ItemKind::Normal);
    if key == "notifications" || key == "mentions" {
        menu.append_separator();
        menu.append(DISMISS, "D&ismiss Notification", "", ItemKind::Normal);
        menu.append(CLEAR_NOTIFICATIONS, "C&lear All Notifications", "", ItemKind::Normal);
        if item.get("type").and_then(Value::as_str) == Some("follow_request") {
            menu.append(ACCEPT, "&Accept Follow Request", "", ItemKind::Normal);
            menu.append(REJECT, "Re&ject Follow Request", "", ItemKind::Normal);
        }
        if item.get("type").and_then(Value::as_str) == Some("quote") && get_quoted_status(&status).is_some() {
            menu.append(REVOKE_QUOTE, "Re&voke Quote of My Post", "", ItemKind::Normal);
        }
    }
    app.borrow().posts.popup_menu(&mut menu, None);
}

fn quote_permissions(app: &Rc<RefCell<App>>) {
    let Some((mut status, index)) = selected_status(&app.borrow()) else {
        return;
    };
    let source = source_status(&status);
    if id_of(source.get("account").unwrap_or(&Value::Null)) != id_of(&app.borrow().me) {
        return;
    }
    const POLICIES: [&str; 3] = ["public", "followers", "nobody"];
    let automatic = source.pointer("/quote_approval/automatic").and_then(Value::as_array).cloned().unwrap_or_default();
    let allows = |policy: &str| automatic.iter().any(|audience| audience.as_str() == Some(policy));
    let current = if allows("public") { 0 } else if allows("followers") { 1 } else { 2 };
    let frame = app.borrow().frame;
    let choice = SingleChoiceDialog::builder(&frame, "Who may quote this post?", "Quote Permissions", &["Anybody", "Followers", "Just me"]).build();
    choice.set_selection(current);
    if choice.show_modal() != ID_OK {
        return;
    }
    let Some(policy) = POLICIES.get(choice.get_selection() as usize) else {
        return;
    };
    let path = format!("/api/v1/statuses/{}/interaction_policy", id_of(&source));
    let result = app.borrow().api.call("PUT", &path, vec![("quote_approval_policy".to_string(), (*policy).to_string())]);
    match result {
        Ok(updated) => {
            if !id_of(&updated).is_empty() {
                if status.get("reblog").is_some_and(Value::is_object) {
                    status["reblog"] = updated;
                } else {
                    status = updated;
                }
                refresh_row(app, index, &status);
            }
            alert(&frame, "Quote permissions updated.", "Quote Permissions");
        }
        Err(error) => error_box(&frame, &format!("Error updating quote permissions: {error}"), "Quote Permissions"),
    }
}

fn revoke_quote(app: &Rc<RefCell<App>>) {
    let (index, quoting, quoted) = {
        let state = app.borrow();
        let index = state.posts.get_first_selected_item();
        if index < 0 || selected_key(&state).as_deref() != Some("notifications") {
            return;
        }
        let Some(quoting) = state
            .timelines
            .get("notifications")
            .and_then(|items| items.get(index as usize))
            .and_then(|item| item.get("status"))
            .filter(|value| value.is_object())
            .cloned()
        else {
            return;
        };
        let Some(quoted) = get_quoted_status(&quoting).cloned() else {
            return;
        };
        if id_of(quoted.get("account").unwrap_or(&Value::Null)) != id_of(&state.me) {
            return;
        }
        (index as usize, quoting, quoted)
    };
    let frame = app.borrow().frame;
    if !confirm(&frame, "Revoke this post's permission to quote you?", "Revoke Quote") {
        return;
    }
    let path = format!("/api/v1/statuses/{}/quotes/{}/revoke", id_of(&quoted), id_of(&quoting));
    let result = app.borrow().api.call("POST", &path, Vec::new());
    match result {
        Ok(updated) => {
            if !id_of(&updated).is_empty() {
                refresh_row(app, index, &updated);
            }
            alert(&frame, "Quote authorization revoked.", "Revoke Quote");
        }
        Err(error) => error_box(&frame, &format!("Error revoking quote: {error}"), "Revoke Quote"),
    }
}

pub(crate) fn id_of(value: &Value) -> String {
    match value.get("id") {
        Some(Value::String(id)) => id.clone(),
        Some(Value::Number(id)) => id.to_string(),
        _ => String::new(),
    }
}

pub(crate) fn account_label_line(account: &Value) -> String {
    format!("{} (@{})", crate::rows::display_name(account), account.get("acct").and_then(Value::as_str).unwrap_or(""))
}
