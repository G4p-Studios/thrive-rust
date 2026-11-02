use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};
use std::fs::File;
use std::io::BufReader;
use std::sync::Mutex;

const FILES: &[(&str, &str)] = &[
    ("tootsnd", "send_toot.wav"),
    ("replysnd", "send_reply.wav"),
    ("boostsnd", "send_boost.wav"),
    ("favsnd", "favorite.wav"),
    ("unfavsnd", "unfavorite.wav"),
    ("newtootsnd", "new_toot.wav"),
    ("dmsnd", "new_dm.wav"),
    ("mentionsnd", "new_mention.wav"),
    ("imagesnd", "image.wav"),
    ("mediasnd", "media.wav"),
    ("select_mentionsnd", "mention.wav"),
    ("pollsnd", "poll.wav"),
    ("votesnd", "vote.wav"),
    ("notificationsnd", "new_notification.wav"),
    ("errorsnd", "error.wav"),
    ("maxsnd", "max.wav"),
    ("searchsnd", "new_search.wav"),
    ("open_timelinesnd", "open_timeline.wav"),
    ("send_dmsnd", "send_dm.wav"),
    ("followsnd", "follow.wav"),
    ("unfollowsnd", "unfollow.wav"),
    ("boundarysnd", "boundary.wav"),
    ("close_timelinesnd", "close_timeline.wav"),
    ("favoritessnd", "favorites.wav"),
    ("search_updatedsnd", "search_updated.wav"),
    ("usersnd", "user.wav"),
];

static OUTPUT: Mutex<Option<MixerDeviceSink>> = Mutex::new(None);

fn output() -> Option<std::sync::MutexGuard<'static, Option<MixerDeviceSink>>> {
    let mut guard = OUTPUT.lock().ok()?;
    if guard.is_none()
        && let Ok(mut sink) = DeviceSinkBuilder::open_default_sink()
    {
        sink.log_on_drop(false);
        *guard = Some(sink);
    }
    if guard.is_some() { Some(guard) } else { None }
}

pub fn play(name: &str) {
    let Some((_, file_name)) = FILES.iter().find(|(key, _)| *key == name) else {
        return;
    };
    let Some(path) = crate::config::sound_file(&crate::config::sound_pack(), file_name) else {
        return;
    };
    let Ok(file) = File::open(path) else {
        return;
    };
    let Ok(decoder) = Decoder::new(BufReader::new(file)) else {
        return;
    };
    let Some(guard) = output() else {
        return;
    };
    let Some(sink) = guard.as_ref() else {
        return;
    };
    let player = Player::connect_new(sink.mixer());
    player.append(decoder);
    player.detach();
}
