use rspotify::prelude::PlayableId;

use super::common_key_events;
use crate::app::{ActiveBlock, App, ArtistBlock, ItemTableContext, RecommendationsContext};
use crate::event::Key;
use crate::network::IoEvent;

fn handle_down_press_on_selected_block(app: &mut App) {
    if let Some(artist) = &mut app.artist {
        match artist.artist_selected_block {
            ArtistBlock::TopTracks => {
                let next_index = common_key_events::on_down_press_handler(
                    &artist.top_tracks,
                    Some(artist.selected_top_track_index),
                );
                artist.selected_top_track_index = next_index;
            }
            ArtistBlock::Albums => {
                let next_index = common_key_events::on_down_press_handler(
                    &artist.albums.items,
                    Some(artist.selected_album_index),
                );
                artist.selected_album_index = next_index;
            }
            ArtistBlock::RelatedArtists => {
                let next_index = common_key_events::on_down_press_handler(
                    &artist.related_artists,
                    Some(artist.selected_related_artist_index),
                );
                artist.selected_related_artist_index = next_index;
            }
            ArtistBlock::Empty => {}
        }
    }
}

fn handle_down_press_on_hovered_block(app: &mut App) {
    if let Some(artist) = &mut app.artist {
        match artist.artist_hovered_block {
            ArtistBlock::TopTracks => {
                artist.artist_hovered_block = ArtistBlock::Albums;
            }
            ArtistBlock::Albums => {
                artist.artist_hovered_block = ArtistBlock::RelatedArtists;
            }
            ArtistBlock::RelatedArtists => {
                artist.artist_hovered_block = ArtistBlock::TopTracks;
            }
            ArtistBlock::Empty => {}
        }
    }
}

fn handle_up_press_on_selected_block(app: &mut App) {
    if let Some(artist) = &mut app.artist {
        match artist.artist_selected_block {
            ArtistBlock::TopTracks => {
                let next_index = common_key_events::on_up_press_handler(
                    &artist.top_tracks,
                    Some(artist.selected_top_track_index),
                );
                artist.selected_top_track_index = next_index;
            }
            ArtistBlock::Albums => {
                let next_index = common_key_events::on_up_press_handler(
                    &artist.albums.items,
                    Some(artist.selected_album_index),
                );
                artist.selected_album_index = next_index;
            }
            ArtistBlock::RelatedArtists => {
                let next_index = common_key_events::on_up_press_handler(
                    &artist.related_artists,
                    Some(artist.selected_related_artist_index),
                );
                artist.selected_related_artist_index = next_index;
            }
            ArtistBlock::Empty => {}
        }
    }
}

fn handle_up_press_on_hovered_block(app: &mut App) {
    if let Some(artist) = &mut app.artist {
        match artist.artist_hovered_block {
            ArtistBlock::TopTracks => {
                artist.artist_hovered_block = ArtistBlock::RelatedArtists;
            }
            ArtistBlock::Albums => {
                artist.artist_hovered_block = ArtistBlock::TopTracks;
            }
            ArtistBlock::RelatedArtists => {
                artist.artist_hovered_block = ArtistBlock::Albums;
            }
            ArtistBlock::Empty => {}
        }
    }
}

fn handle_high_press_on_selected_block(app: &mut App) {
    if let Some(artist) = &mut app.artist {
        match artist.artist_selected_block {
            ArtistBlock::TopTracks => {
                let next_index = common_key_events::on_high_press_handler();
                artist.selected_top_track_index = next_index;
            }
            ArtistBlock::Albums => {
                let next_index = common_key_events::on_high_press_handler();
                artist.selected_album_index = next_index;
            }
            ArtistBlock::RelatedArtists => {
                let next_index = common_key_events::on_high_press_handler();
                artist.selected_related_artist_index = next_index;
            }
            ArtistBlock::Empty => {}
        }
    }
}

fn handle_middle_press_on_selected_block(app: &mut App) {
    if let Some(artist) = &mut app.artist {
        match artist.artist_selected_block {
            ArtistBlock::TopTracks => {
                let next_index = common_key_events::on_middle_press_handler(&artist.top_tracks);
                artist.selected_top_track_index = next_index;
            }
            ArtistBlock::Albums => {
                let next_index = common_key_events::on_middle_press_handler(&artist.albums.items);
                artist.selected_album_index = next_index;
            }
            ArtistBlock::RelatedArtists => {
                let next_index =
                    common_key_events::on_middle_press_handler(&artist.related_artists);
                artist.selected_related_artist_index = next_index;
            }
            ArtistBlock::Empty => {}
        }
    }
}

fn handle_low_press_on_selected_block(app: &mut App) {
    if let Some(artist) = &mut app.artist {
        match artist.artist_selected_block {
            ArtistBlock::TopTracks => {
                let next_index = common_key_events::on_low_press_handler(&artist.top_tracks);
                artist.selected_top_track_index = next_index;
            }
            ArtistBlock::Albums => {
                let next_index = common_key_events::on_low_press_handler(&artist.albums.items);
                artist.selected_album_index = next_index;
            }
            ArtistBlock::RelatedArtists => {
                let next_index = common_key_events::on_low_press_handler(&artist.related_artists);
                artist.selected_related_artist_index = next_index;
            }
            ArtistBlock::Empty => {}
        }
    }
}

fn handle_recommend_event_on_selected_block(app: &mut App) {
    //recommendations.
    if let Some(artist) = &mut app.artist.clone() {
        match artist.artist_selected_block {
            ArtistBlock::TopTracks => {
                let selected_index = artist.selected_top_track_index;
                if let Some(track) = artist.top_tracks.get(selected_index) {
                    let track_ids = track.id.clone().map(|id| vec![id]);
                    app.recommendations_context = Some(RecommendationsContext::Song);
                    app.recommendations_seed = track.name.clone();
                    app.get_recommendations_for_seed(None, track_ids, Some(track.clone()));
                }
            }
            ArtistBlock::RelatedArtists => {
                let selected_index = artist.selected_related_artist_index;
                let artist_id = artist.related_artists[selected_index].id.clone();
                let artist_name = &artist.related_artists[selected_index].name;

                app.recommendations_context = Some(RecommendationsContext::Artist);
                app.recommendations_seed = artist_name.clone();
                app.get_recommendations_for_seed(Some(vec![artist_id]), None, None);
            }
            _ => {}
        }
    }
}

fn handle_enter_event_on_selected_block(app: &mut App) {
    if let Some(artist) = &mut app.artist.clone() {
        match artist.artist_selected_block {
            ArtistBlock::TopTracks => {
                let selected_index = artist.selected_top_track_index;
                let playable_ids = artist
                    .top_tracks
                    .iter()
                    .filter_map(|track| track.id.clone())
                    .map(PlayableId::Track)
                    .collect();
                app.dispatch(IoEvent::StartPlayablesPlayback {
                    playable_ids,
                    offset: Some(selected_index as u32),
                });
            }
            ArtistBlock::Albums => {
                if let Some(selected_album) = artist
                    .albums
                    .items
                    .get(artist.selected_album_index)
                    .cloned()
                {
                    app.item_table.context = Some(ItemTableContext::AlbumSearch);
                    app.dispatch(IoEvent::GetAlbumTracks {
                        album: Box::new(selected_album),
                    });
                }
            }
            ArtistBlock::RelatedArtists => {
                let selected_index = artist.selected_related_artist_index;
                let artist_id = artist.related_artists[selected_index].id.clone();
                let artist_name = artist.related_artists[selected_index].name.clone();
                app.get_artist(artist_id, artist_name);
            }
            ArtistBlock::Empty => {}
        }
    }
}

fn handle_enter_event_on_hovered_block(app: &mut App) {
    if let Some(artist) = &mut app.artist {
        match artist.artist_hovered_block {
            ArtistBlock::TopTracks => artist.artist_selected_block = ArtistBlock::TopTracks,
            ArtistBlock::Albums => artist.artist_selected_block = ArtistBlock::Albums,
            ArtistBlock::RelatedArtists => {
                artist.artist_selected_block = ArtistBlock::RelatedArtists
            }
            ArtistBlock::Empty => {}
        }
    }
}

pub fn handler(key: Key, app: &mut App) {
    let Some(artist) = &mut app.artist else {
        return;
    };

    match key {
        Key::Esc => {
            artist.artist_selected_block = ArtistBlock::Empty;
        }
        k if common_key_events::down_event(k) => {
            if artist.artist_selected_block != ArtistBlock::Empty {
                handle_down_press_on_selected_block(app);
            } else {
                handle_down_press_on_hovered_block(app);
            }
        }
        k if common_key_events::up_event(k) => {
            if artist.artist_selected_block != ArtistBlock::Empty {
                handle_up_press_on_selected_block(app);
            } else {
                handle_up_press_on_hovered_block(app);
            }
        }
        k if common_key_events::left_event(k) => {
            artist.artist_selected_block = ArtistBlock::Empty;
            match artist.artist_hovered_block {
                ArtistBlock::TopTracks => common_key_events::handle_left_event(app),
                ArtistBlock::Albums => {
                    artist.artist_hovered_block = ArtistBlock::TopTracks;
                }
                ArtistBlock::RelatedArtists => {
                    artist.artist_hovered_block = ArtistBlock::Albums;
                }
                ArtistBlock::Empty => {}
            }
        }
        k if common_key_events::right_event(k) => {
            artist.artist_selected_block = ArtistBlock::Empty;
            handle_down_press_on_hovered_block(app);
        }
        k if common_key_events::high_event(k) => {
            if artist.artist_selected_block != ArtistBlock::Empty {
                handle_high_press_on_selected_block(app);
            }
        }
        k if common_key_events::middle_event(k) => {
            if artist.artist_selected_block != ArtistBlock::Empty {
                handle_middle_press_on_selected_block(app);
            }
        }
        k if common_key_events::low_event(k) => {
            if artist.artist_selected_block != ArtistBlock::Empty {
                handle_low_press_on_selected_block(app);
            }
        }
        Key::Enter => {
            if artist.artist_selected_block != ArtistBlock::Empty {
                handle_enter_event_on_selected_block(app);
            } else {
                handle_enter_event_on_hovered_block(app);
            }
        }
        Key::Char('r') => {
            if artist.artist_selected_block != ArtistBlock::Empty {
                handle_recommend_event_on_selected_block(app);
            }
        }
        Key::Char('w') => match artist.artist_selected_block {
            ArtistBlock::Albums => app.current_user_saved_album_add(ActiveBlock::ArtistBlock),
            ArtistBlock::RelatedArtists => app.user_follow_artists(ActiveBlock::ArtistBlock),
            _ => (),
        },
        Key::Char('D') => match artist.artist_selected_block {
            ArtistBlock::Albums => app.current_user_saved_album_delete(ActiveBlock::ArtistBlock),
            ArtistBlock::RelatedArtists => app.user_unfollow_artists(ActiveBlock::ArtistBlock),
            _ => (),
        },
        _ if key == app.user_config.keys.add_item_to_queue => match artist.artist_selected_block {
            ArtistBlock::TopTracks => {
                if let Some(track_id) = artist
                    .top_tracks
                    .get(artist.selected_top_track_index)
                    .and_then(|track| track.id.as_ref())
                {
                    app.dispatch(IoEvent::AddItemToQueue {
                        playable_id: track_id.clone().into(),
                    });
                }
            }
            _ => (),
        },
        _ => {}
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ActiveBlock;

    #[test]
    fn on_esc() {
        let mut app = App::default();

        handler(Key::Esc, &mut app);

        let current_route = app.get_current_route();
        assert_eq!(current_route.active_block, ActiveBlock::Empty);
    }
}
