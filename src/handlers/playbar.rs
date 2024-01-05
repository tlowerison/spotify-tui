use super::{
    super::app::{ActiveBlock, App},
    common_key_events,
};
use crate::event::Key;
use crate::network::IoEvent;
use anyhow::anyhow;
use rspotify::model::{context::CurrentPlaybackContext, PlayableItem};

pub fn handler(key: Key, app: &mut App) {
    match key {
        k if common_key_events::up_event(k) => {
            app.set_current_route_state(Some(ActiveBlock::Empty), Some(ActiveBlock::MyPlaylists));
        }
        Key::Char('s') => {
            if let Some(CurrentPlaybackContext {
                item: Some(item), ..
            }) = app.current_playback_context.to_owned()
            {
                match item {
                    PlayableItem::Track(track) => {
                        if let Some(track_id) = track.id {
                            app.dispatch(IoEvent::ToggleSaveTrack { track_id });
                        }
                    }
                    PlayableItem::Episode(episode) => {
                        app.handle_error(anyhow!("cannot save episodes right now"));
                    }
                };
            };
        }
        _ => {}
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_left_press() {
        let mut app = App::default();
        app.set_current_route_state(Some(ActiveBlock::PlayBar), Some(ActiveBlock::PlayBar));

        handler(Key::Up, &mut app);
        let current_route = app.get_current_route();
        assert_eq!(current_route.active_block, ActiveBlock::Empty);
        assert_eq!(current_route.hovered_block, ActiveBlock::MyPlaylists);
    }
}
