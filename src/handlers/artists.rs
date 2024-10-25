use rspotify::prelude::PlayContextId;

use super::common_key_events;
use crate::{
    app::{ActiveBlock, App, RecommendationsContext, RouteId},
    event::Key,
    network::IoEvent,
};

pub fn handler(key: Key, app: &mut App) {
    match key {
        k if common_key_events::left_event(k) => common_key_events::handle_left_event(app),
        k if common_key_events::down_event(k) => {
            if let Some(artists) = &mut app.library.saved_artists.get_results(None) {
                let next_index = common_key_events::on_down_press_handler(
                    &artists.items,
                    Some(app.artists_list_index),
                );
                app.artists_list_index = next_index;
            }
        }
        k if common_key_events::up_event(k) => {
            if let Some(artists) = &mut app.library.saved_artists.get_results(None) {
                let next_index = common_key_events::on_up_press_handler(
                    &artists.items,
                    Some(app.artists_list_index),
                );
                app.artists_list_index = next_index;
            }
        }
        k if common_key_events::high_event(k) => {
            if let Some(_artists) = &mut app.library.saved_artists.get_results(None) {
                let next_index = common_key_events::on_high_press_handler();
                app.artists_list_index = next_index;
            }
        }
        k if common_key_events::middle_event(k) => {
            if let Some(artists) = &mut app.library.saved_artists.get_results(None) {
                let next_index = common_key_events::on_middle_press_handler(&artists.items);
                app.artists_list_index = next_index;
            }
        }
        k if common_key_events::low_event(k) => {
            if let Some(artists) = &mut app.library.saved_artists.get_results(None) {
                let next_index = common_key_events::on_low_press_handler(&artists.items);
                app.artists_list_index = next_index;
            }
        }
        Key::Enter => {
            let artists = app.artists.to_owned();
            if !artists.is_empty() {
                let artist = &artists[app.artists_list_index];
                app.get_artist(artist.id.clone(), artist.name.clone());
                app.push_navigation_stack(RouteId::Artist, ActiveBlock::ArtistBlock);
            }
        }
        Key::Char('D') => app.user_unfollow_artists(ActiveBlock::AlbumList),
        Key::Char('e') => {
            let artists = app.artists.to_owned();
            let artist = artists.get(app.artists_list_index);
            if let Some(artist) = artist {
                app.dispatch(IoEvent::StartContextPlayback {
                    play_context_id: PlayContextId::Artist(artist.id.clone()),
                    offset: None,
                });
            }
        }
        Key::Char('r') => {
            let artists = app.artists.to_owned();
            let artist = artists.get(app.artists_list_index);
            if let Some(artist) = artist {
                let artist_name = artist.name.clone();

                app.recommendations_context = Some(RecommendationsContext::Artist);
                app.recommendations_seed = artist_name;
                app.get_recommendations_for_seed(Some(vec![artist.id.clone()]), None, None);
            }
        }
        k if k == app.user_config.keys.next_page => app.get_current_user_saved_artists_next(),
        k if k == app.user_config.keys.previous_page => {
            app.get_current_user_saved_artists_previous()
        }
        _ => {}
    }
}
