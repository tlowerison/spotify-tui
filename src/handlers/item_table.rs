use super::{
    super::app::{App, ItemTable, ItemTableContext, RecommendationsContext},
    common_key_events,
};
use crate::event::Key;
use crate::network::IoEvent;
use rand::{thread_rng, Rng};
use rspotify::model::{idtypes::*, PlayableItem};
use serde_json::from_value;

pub fn handler(key: Key, app: &mut App) {
    match key {
        k if common_key_events::left_event(k) => common_key_events::handle_left_event(app),
        k if common_key_events::down_event(k) => {
            let next_index = common_key_events::on_down_press_handler(
                &app.item_table.items,
                Some(app.item_table.selected_index),
            );
            app.item_table.selected_index = next_index;
        }
        k if common_key_events::up_event(k) => {
            let next_index = common_key_events::on_up_press_handler(
                &app.item_table.items,
                Some(app.item_table.selected_index),
            );
            app.item_table.selected_index = next_index;
        }
        k if common_key_events::high_event(k) => {
            let next_index = common_key_events::on_high_press_handler();
            app.item_table.selected_index = next_index;
        }
        k if common_key_events::middle_event(k) => {
            let next_index = common_key_events::on_middle_press_handler(&app.item_table.items);
            app.item_table.selected_index = next_index;
        }
        k if common_key_events::low_event(k) => {
            let next_index = common_key_events::on_low_press_handler(&app.item_table.items);
            app.item_table.selected_index = next_index;
        }
        Key::Enter => {
            on_enter(app);
        }
        // Scroll down
        k if k == app.user_config.keys.next_page => {
            match &app.item_table.context {
                Some(context) => match context {
                    ItemTableContext::MyPlaylists => {
                        if let (Some(playlists), Some(selected_playlist_index)) =
                            (&app.playlists, &app.selected_playlist_index)
                        {
                            if let Some(selected_playlist) =
                                playlists.items.get(selected_playlist_index.to_owned())
                            {
                                if let Some(playlist_tracks) = &app.playlist_items {
                                    if app.playlist_offset + app.large_search_limit
                                        < playlist_tracks.total
                                    {
                                        app.playlist_offset += app.large_search_limit;
                                        let playlist_id = selected_playlist.id.to_owned();
                                        app.dispatch(IoEvent::GetPlaylistItems {
                                            playlist_id,
                                            offset: app.playlist_offset,
                                        });
                                    }
                                }
                            }
                        };
                    }
                    ItemTableContext::RecommendedTracks => {}
                    ItemTableContext::SavedTracks => {
                        app.get_current_user_saved_tracks_next();
                    }
                    ItemTableContext::AlbumSearch => {}
                    ItemTableContext::PlaylistSearch => {}
                    ItemTableContext::MadeForYou => {
                        let (playlists, selected_playlist_index) =
                            (&app.library.made_for_you_playlists, &app.made_for_you_index);

                        if let Some(selected_playlist) = playlists
                            .get_results(Some(0))
                            .unwrap()
                            .items
                            .get(selected_playlist_index.to_owned())
                        {
                            if let Some(playlist_tracks) = &app.made_for_you_tracks {
                                if app.made_for_you_offset + app.large_search_limit
                                    < playlist_tracks.total
                                {
                                    app.made_for_you_offset += app.large_search_limit;
                                    let playlist_id = selected_playlist.id.to_owned();
                                    app.dispatch(IoEvent::GetMadeForYouPlaylistItems {
                                        playlist_id,
                                        offset: app.made_for_you_offset,
                                    });
                                }
                            }
                        }
                    }
                },
                None => {}
            };
        }
        // Scroll up
        k if k == app.user_config.keys.previous_page => {
            match &app.item_table.context {
                Some(context) => match context {
                    ItemTableContext::MyPlaylists => {
                        if let (Some(playlists), Some(selected_playlist_index)) =
                            (&app.playlists, &app.selected_playlist_index)
                        {
                            if app.playlist_offset >= app.large_search_limit {
                                app.playlist_offset -= app.large_search_limit;
                            };
                            if let Some(selected_playlist) =
                                playlists.items.get(selected_playlist_index.to_owned())
                            {
                                let playlist_id = selected_playlist.id.to_owned();
                                app.dispatch(IoEvent::GetPlaylistItems {
                                    playlist_id,
                                    offset: app.playlist_offset,
                                });
                            }
                        };
                    }
                    ItemTableContext::RecommendedTracks => {}
                    ItemTableContext::SavedTracks => {
                        app.get_current_user_saved_tracks_previous();
                    }
                    ItemTableContext::AlbumSearch => {}
                    ItemTableContext::PlaylistSearch => {}
                    ItemTableContext::MadeForYou => {
                        let (playlists, selected_playlist_index) = (
                            &app.library
                                .made_for_you_playlists
                                .get_results(Some(0))
                                .unwrap(),
                            app.made_for_you_index,
                        );
                        if app.made_for_you_offset >= app.large_search_limit {
                            app.made_for_you_offset -= app.large_search_limit;
                        }
                        if let Some(selected_playlist) =
                            playlists.items.get(selected_playlist_index)
                        {
                            let playlist_id = selected_playlist.id.to_owned();
                            app.dispatch(IoEvent::GetMadeForYouPlaylistItems {
                                playlist_id,
                                offset: app.made_for_you_offset,
                            });
                        }
                    }
                },
                None => {}
            };
        }
        Key::Char('s') => handle_save_track_event(app),
        Key::Char('S') => play_random_song(app),
        k if k == app.user_config.keys.jump_to_end => jump_to_end(app),
        k if k == app.user_config.keys.jump_to_start => jump_to_start(app),
        //recommended song radio
        Key::Char('r') => {
            handle_recommended_tracks(app);
        }
        _ if key == app.user_config.keys.add_item_to_queue => on_queue(app),
        _ => {}
    }
}

fn play_random_song(app: &mut App) {
    if let Some(context) = &app.item_table.context {
        match context {
            ItemTableContext::MyPlaylists => {
                let (play_context_id, track_json) =
                    match (&app.selected_playlist_index, &app.playlists) {
                        (Some(selected_playlist_index), Some(playlists)) => {
                            if let Some(selected_playlist) =
                                playlists.items.get(selected_playlist_index.to_owned())
                            {
                                (
                                    Some(PlayContextId::Playlist(selected_playlist.id)),
                                    selected_playlist.tracks.get("total"),
                                )
                            } else {
                                (None, None)
                            }
                        }
                        _ => (None, None),
                    };

                if let Some(val) = track_json {
                    let num_tracks: usize = from_value(val.clone()).unwrap();
                    app.dispatch(IoEvent::StartContextPlayback {
                        play_context_id,
                        offset: Some(thread_rng().gen_range(0..num_tracks) as u32),
                    });
                }
            }
            ItemTableContext::RecommendedTracks => {}
            ItemTableContext::SavedTracks => {
                if let Some(saved_tracks) = &app.library.saved_tracks.get_results(None) {
                    let playable_ids = saved_tracks
                        .items
                        .iter()
                        .filter_map(|item| item.track.id)
                        .map(PlayableId::Track)
                        .collect::<Vec<_>>();
                    let rand_idx = thread_rng().gen_range(0..playable_ids.len());
                    app.dispatch(IoEvent::StartPlayablesPlayback {
                        playable_ids,
                        offset: Some(rand_idx as u32),
                    })
                }
            }
            ItemTableContext::AlbumSearch => {}
            ItemTableContext::PlaylistSearch => {
                let (play_context_id, playlist_track_json) = match (
                    &app.search_results.selected_playlists_index,
                    &app.search_results.playlists,
                ) {
                    (Some(selected_playlist_index), Some(playlist_result)) => {
                        if let Some(selected_playlist) = playlist_result
                            .items
                            .get(selected_playlist_index.to_owned())
                        {
                            (
                                Some(PlayContextId::Playlist(selected_playlist.id)),
                                selected_playlist.tracks.get("total"),
                            )
                        } else {
                            (None, None)
                        }
                    }
                    _ => (None, None),
                };
                if let Some(val) = playlist_track_json {
                    let num_tracks: usize = from_value(val.clone()).unwrap();
                    app.dispatch(IoEvent::StartContextPlayback {
                        play_context_id,
                        offset: Some(thread_rng().gen_range(0..num_tracks) as u32),
                    })
                }
            }
            ItemTableContext::MadeForYou => {
                if let Some(playlist) = &app
                    .library
                    .made_for_you_playlists
                    .get_results(Some(0))
                    .and_then(|playlist| playlist.items.get(app.made_for_you_index))
                {
                    if let Some(num_tracks) = &playlist
                        .tracks
                        .get("total")
                        .and_then(|total| -> Option<usize> { from_value(total.clone()).ok() })
                    {
                        let play_context_id = PlayContextId::Playlist(playlist.id);
                        app.dispatch(IoEvent::StartContextPlayback {
                            play_context_id,
                            offset: Some(thread_rng().gen_range(0..*num_tracks)),
                        })
                    };
                };
            }
        }
    };
}

fn handle_save_track_event(app: &mut App) {
    let selected_index = app.item_table.selected_index;
    let items = &app.item_table.items;
    if let Some(item) = items.get(selected_index) {
        if let Some(id) = item.id() {
            let track_id = match id {
                PlayableId::Track(id) => id,
                _ => return,
            };
            app.dispatch(IoEvent::ToggleSaveTrack { track_id });
        };
    };
}

fn handle_recommended_tracks(app: &mut App) {
    let selected_index = app.item_table.selected_index;
    let items = &app.item_table.items;
    if let Some(item) = items.get(selected_index).cloned() {
        let track = match item {
            PlayableItem::Track(track) => track,
            _ => return,
        };
        let track_id = match track.id.clone() {
            Some(id) => id,
            None => return,
        };
        app.recommendations_context = Some(RecommendationsContext::Song);
        app.recommendations_seed = track.name.clone();
        app.get_recommendations_for_seed(None, Some(vec![track_id]), Some(track));
    };
}

fn jump_to_end(app: &mut App) {
    match &app.item_table.context {
        Some(context) => match context {
            ItemTableContext::MyPlaylists => {
                if let (Some(playlists), Some(selected_playlist_index)) =
                    (&app.playlists, &app.selected_playlist_index)
                {
                    if let Some(selected_playlist) =
                        playlists.items.get(selected_playlist_index.to_owned())
                    {
                        let total_tracks = selected_playlist
                            .tracks
                            .get("total")
                            .and_then(|total| total.as_u64())
                            .expect("playlist.tracks object should have a total field")
                            as u32;

                        if app.large_search_limit < total_tracks {
                            app.playlist_offset =
                                total_tracks - (total_tracks % app.large_search_limit);
                            let playlist_id = selected_playlist.id;
                            app.dispatch(IoEvent::GetPlaylistItems {
                                playlist_id,
                                offset: app.playlist_offset,
                            });
                        }
                    }
                }
            }
            ItemTableContext::RecommendedTracks => {}
            ItemTableContext::SavedTracks => {}
            ItemTableContext::AlbumSearch => {}
            ItemTableContext::PlaylistSearch => {}
            ItemTableContext::MadeForYou => {}
        },
        None => {}
    }
}

fn on_enter(app: &mut App) {
    let ItemTable {
        context,
        selected_index,
        items,
    } = &app.item_table;
    match &context {
        Some(context) => match context {
            ItemTableContext::MyPlaylists => {
                if let Some(_track) = items.get(*selected_index) {
                    let play_context_id = match (&app.active_playlist_index, &app.playlists) {
                        (Some(active_playlist_index), Some(playlists)) => playlists
                            .items
                            .get(active_playlist_index.to_owned())
                            .map(|selected_playlist| PlayContextId::Playlist(selected_playlist.id)),
                        _ => None,
                    };
                    if let Some(play_context_id) = play_context_id {
                        app.dispatch(IoEvent::StartContextPlayback {
                            play_context_id,
                            offset: Some(
                                app.item_table.selected_index as u32 + app.playlist_offset,
                            ),
                        });
                    }
                };
            }
            ItemTableContext::RecommendedTracks => {
                app.dispatch(IoEvent::StartPlayablesPlayback {
                    playable_ids: app
                        .recommended_tracks
                        .iter()
                        .filter_map(|x| x.id.clone())
                        .map(PlayableId::Track)
                        .collect::<Vec<_>>(),
                    offset: Some(app.item_table.selected_index as u32),
                });
            }
            ItemTableContext::SavedTracks => {
                if let Some(saved_tracks) = &app.library.saved_tracks.get_results(None) {
                    let playable_ids = saved_tracks
                        .items
                        .iter()
                        .filter_map(|item| item.track.id.clone())
                        .map(PlayableId::Track)
                        .collect::<Vec<_>>();

                    app.dispatch(IoEvent::StartPlayablesPlayback {
                        playable_ids,
                        offset: Some(app.item_table.selected_index as u32),
                    });
                };
            }
            ItemTableContext::AlbumSearch => {}
            ItemTableContext::PlaylistSearch => {
                let ItemTable {
                    selected_index,
                    items,
                    ..
                } = &app.item_table;
                if let Some(_track) = items.get(*selected_index) {
                    let play_context_id = match (
                        &app.search_results.selected_playlists_index,
                        &app.search_results.playlists,
                    ) {
                        (Some(selected_playlist_index), Some(playlist_result)) => playlist_result
                            .items
                            .get(selected_playlist_index.to_owned())
                            .map(|selected_playlist| PlayContextId::Playlist(selected_playlist.id)),
                        _ => None,
                    };
                    if let Some(play_context_id) = play_context_id {
                        app.dispatch(IoEvent::StartContextPlayback {
                            play_context_id,
                            offset: Some(app.item_table.selected_index as u32),
                        });
                    }
                };
            }
            ItemTableContext::MadeForYou => {
                if items.get(*selected_index).is_some() {
                    let play_context_id = PlayContextId::Playlist(
                        app.library
                            .made_for_you_playlists
                            .get_results(Some(0))
                            .unwrap()
                            .items
                            .get(app.made_for_you_index)
                            .unwrap()
                            .id,
                    );

                    app.dispatch(IoEvent::StartContextPlayback {
                        play_context_id,
                        offset: Some(
                            app.item_table.selected_index as u32 + app.made_for_you_offset,
                        ),
                    });
                }
            }
        },
        None => {}
    };
}

fn on_queue(app: &mut App) {
    let ItemTable {
        context,
        selected_index,
        items,
    } = &app.item_table;
    match &context {
        Some(context) => match context {
            ItemTableContext::MyPlaylists => {
                if let Some(playable_id) = items
                    .get(*selected_index)
                    .and_then(|playable_item| playable_item.clone().id())
                {
                    app.dispatch(IoEvent::AddItemToQueue { playable_id });
                };
            }
            ItemTableContext::RecommendedTracks => {
                if let Some(playable_id) = app
                    .recommended_tracks
                    .get(app.item_table.selected_index)
                    .and_then(|track| track.id.clone().map(PlayableId::Track))
                {
                    app.dispatch(IoEvent::AddItemToQueue { playable_id });
                }
            }
            ItemTableContext::SavedTracks => {
                if let Some(page) = app.library.saved_tracks.get_results(None) {
                    if let Some(playable_id) = page
                        .items
                        .get(app.item_table.selected_index)
                        .and_then(|saved_track| saved_track.track.id.clone().map(PlayableId::Track))
                    {
                        app.dispatch(IoEvent::AddItemToQueue { playable_id });
                    }
                }
            }
            ItemTableContext::AlbumSearch => {}
            ItemTableContext::PlaylistSearch => {
                let ItemTable {
                    selected_index,
                    items,
                    ..
                } = &app.item_table;
                if let Some(playable_id) = items
                    .get(*selected_index)
                    .and_then(|playable_item| playable_item.clone().id())
                {
                    app.dispatch(IoEvent::AddItemToQueue { playable_id });
                };
            }
            ItemTableContext::MadeForYou => {
                if let Some(playable_id) = items
                    .get(*selected_index)
                    .and_then(|playable_item| playable_item.clone().id())
                {
                    app.dispatch(IoEvent::AddItemToQueue { playable_id });
                }
            }
        },
        None => {}
    };
}

fn jump_to_start(app: &mut App) {
    match &app.item_table.context {
        Some(context) => match context {
            ItemTableContext::MyPlaylists => {
                if let (Some(playlists), Some(selected_playlist_index)) =
                    (&app.playlists, &app.selected_playlist_index)
                {
                    if let Some(selected_playlist) =
                        playlists.items.get(selected_playlist_index.to_owned())
                    {
                        app.playlist_offset = 0;
                        let playlist_id = selected_playlist.id.to_owned();
                        app.dispatch(IoEvent::GetPlaylistItems {
                            playlist_id,
                            offset: app.playlist_offset,
                        });
                    }
                }
            }
            ItemTableContext::RecommendedTracks => {}
            ItemTableContext::SavedTracks => {}
            ItemTableContext::AlbumSearch => {}
            ItemTableContext::PlaylistSearch => {}
            ItemTableContext::MadeForYou => {}
        },
        None => {}
    }
}
