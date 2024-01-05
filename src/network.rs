use crate::app::{
    ActiveBlock, AlbumTableContext, App, Artist, ArtistBlock, EpisodeTableContext,
    ItemTableContext, RouteId, ScrollableResultPages, SelectedAlbum, SelectedFullAlbum,
    SelectedFullShow, SelectedShow,
};
use crate::config::ClientConfig;
use crate::util::{fmt_id, fmt_ids, fmt_opt_ids};
use anyhow::anyhow;
use chrono::Duration;
use derivative::Derivative;
use futures_util::{future::try_join_all, try_join};
use rspotify::model::{
    album::SimplifiedAlbum,
    artist::FullArtist,
    enums::{AdditionalType, Country, RepeatState, SearchType},
    idtypes::*,
    page::Page,
    playlist::{PlaylistItem, SimplifiedPlaylist},
    recommend::Recommendations,
    search::SearchResult,
    show::SimplifiedShow,
    track::FullTrack,
    DevicePayload, Market, Offset, PlayableItem,
};
use rspotify::{clients::*, AuthCodePkceSpotify as Spotify, Credentials, OAuth, Token};
use std::{
    sync::Arc,
    time::{Instant, SystemTime},
};
use tokio::sync::Mutex;

#[derive(Derivative)]
#[derivative(Debug)]
pub enum IoEvent<'a> {
    AddItemToQueue {
        #[derivative(Debug(format_with = "fmt_id"))]
        playable_id: PlayableId<'a>,
    },
    ChangeVolume {
        volume: u8,
    },
    CurrentUserSavedAlbumAdd {
        #[derivative(Debug(format_with = "fmt_id"))]
        album_id: AlbumId<'a>,
    },
    CurrentUserSavedAlbumDelete {
        #[derivative(Debug(format_with = "fmt_id"))]
        album_id: AlbumId<'a>,
    },
    CurrentUserSavedAlbumsContains {
        #[derivative(Debug(format_with = "fmt_ids"))]
        album_ids: Vec<AlbumId<'a>>,
    },
    CurrentUserSavedShowAdd {
        #[derivative(Debug(format_with = "fmt_id"))]
        show_id: ShowId<'a>,
    },
    CurrentUserSavedShowDelete {
        #[derivative(Debug(format_with = "fmt_id"))]
        show_id: ShowId<'a>,
    },
    CurrentUserSavedShowsContains {
        #[derivative(Debug(format_with = "fmt_ids"))]
        show_ids: Vec<ShowId<'a>>,
    },
    CurrentUserSavedTracksContains {
        #[derivative(Debug(format_with = "fmt_ids"))]
        track_ids: Vec<TrackId<'a>>,
    },
    GetAlbum {
        #[derivative(Debug(format_with = "fmt_id"))]
        album_id: AlbumId<'a>,
    },
    GetAlbumForTrack {
        #[derivative(Debug(format_with = "fmt_id"))]
        track_id: TrackId<'a>,
    },
    GetAlbumTracks {
        album: Box<SimplifiedAlbum>,
    },
    GetArtist {
        #[derivative(Debug(format_with = "fmt_id"))]
        artist_id: ArtistId<'a>,
        input_artist_name: String,
        country: Option<Country>,
    },
    GetTrackAnalysis {
        #[derivative(Debug(format_with = "fmt_id"))]
        track_id: TrackId<'a>,
    },
    GetCurrentPlayback,
    GetCurrentSavedTracks {
        offset: Option<u32>,
    },
    GetCurrentShowEpisodes {
        #[derivative(Debug(format_with = "fmt_id"))]
        show_id: ShowId<'a>,
        offset: Option<u32>,
    },
    GetCurrentUserSavedAlbums {
        offset: Option<u32>,
    },
    GetCurrentUserSavedShows {
        offset: Option<u32>,
    },
    GetDevices,
    GetFollowedArtists {
        after: Option<ArtistId<'a>>,
    },
    GetMadeForYouPlaylistItems {
        #[derivative(Debug(format_with = "fmt_id"))]
        playlist_id: PlaylistId<'a>,
        offset: u32,
    },
    GetPlaylists,
    GetPlaylistItems {
        #[derivative(Debug(format_with = "fmt_id"))]
        playlist_id: PlaylistId<'a>,
        offset: u32,
    },
    GetRecentlyPlayed,
    GetRecommendationsForSeed {
        #[derivative(Debug(format_with = "fmt_opt_ids"))]
        seed_artist_ids: Option<Vec<ArtistId<'a>>>,
        #[derivative(Debug(format_with = "fmt_opt_ids"))]
        seed_track_ids: Option<Vec<TrackId<'a>>>,
        first_track: Box<Option<FullTrack>>,
        country: Option<Country>,
    },
    GetRecommendationsForTrackId {
        #[derivative(Debug(format_with = "fmt_id"))]
        track_id: TrackId<'a>,
        country: Option<Country>,
    },
    GetSearchResults {
        search_term: String,
        country: Option<Country>,
    },
    GetShow {
        #[derivative(Debug(format_with = "fmt_id"))]
        show_id: ShowId<'a>,
    },
    GetShowEpisodes {
        show: Box<SimplifiedShow>,
    },
    GetUser,
    MadeForYouSearchAndAdd {
        search_term: String,
        country: Option<Country>,
    },
    NextTrack,
    PausePlayback,
    PreviousTrack,
    RefreshAuthentication,
    Repeat {
        state: RepeatState,
    },
    ResumePlayback,
    Seek {
        position_ms: u32,
    },
    SetArtistsToTable {
        artists: Vec<FullArtist>,
    },
    SetTracksToTable {
        tracks: Vec<FullTrack>,
    },
    StartContextPlayback {
        #[derivative(Debug(format_with = "fmt_id"))]
        play_context_id: PlayContextId<'a>,
        offset: Option<u32>,
    },
    StartPlayablesPlayback {
        #[derivative(Debug(format_with = "fmt_ids"))]
        playable_ids: Vec<PlayableId<'a>>,
        offset: Option<u32>,
    },
    ToggleSaveEpisode {
        #[derivative(Debug(format_with = "fmt_id"))]
        episode_id: EpisodeId<'a>,
    },
    ToggleSaveTrack {
        #[derivative(Debug(format_with = "fmt_id"))]
        track_id: TrackId<'a>,
    },
    ToggleShuffle,
    TransferPlaybackToDevice {
        device_id: String,
    },
    UpdateSearchLimits {
        large_search_limit: u32,
        small_search_limit: u32,
    },
    UserUnfollowArtists {
        #[derivative(Debug(format_with = "fmt_ids"))]
        artist_ids: Vec<ArtistId<'a>>,
    },
    UserFollowArtists {
        #[derivative(Debug(format_with = "fmt_ids"))]
        artist_ids: Vec<ArtistId<'a>>,
    },
    UserFollowPlaylist {
        #[derivative(Debug(format_with = "fmt_id"))]
        playlist_id: PlaylistId<'a>,
        is_public: Option<bool>,
    },
    UserUnfollowPlaylist {
        #[derivative(Debug(format_with = "fmt_id"))]
        playlist_id: PlaylistId<'a>,
    },
    UserArtistFollowCheck {
        #[derivative(Debug(format_with = "fmt_ids"))]
        artist_ids: Vec<ArtistId<'a>>,
    },
}

pub fn get_spotify(token: Token) -> (Spotify, SystemTime) {
    let token_expiry: SystemTime = {
        if let Some(expires_at) = token.expires_at {
            // Set 10 seconds early
            (expires_at - Duration::seconds(10)).into()
        } else {
            SystemTime::now()
        }
    };

    let client_credential = Credentials::default().token_info(token_info).build();

    let spotify = Spotify::default()
        .client_credentials_manager(client_credential)
        .build();

    (spotify, token_expiry)
}

#[derive(Clone)]
pub struct Network<'a> {
    pub spotify: Spotify,
    pub client_config: ClientConfig,
    pub app: &'a Arc<Mutex<App>>,
    oauth: OAuth,
    large_search_limit: u32,
    small_search_limit: u32,
}

macro_rules! handle_error {
    ($self:ident, $res:expr) => {
        match $res {
            Ok(ok) => ok,
            Err(err) => {
                $self.handle_error(anyhow!(err)).await;
                return;
            }
        }
    };
}

impl<'a> Network<'a> {
    pub fn new(
        oauth: OAuth,
        spotify: Spotify,
        client_config: ClientConfig,
        app: &'a Arc<Mutex<App>>,
    ) -> Self {
        Network {
            oauth,
            spotify,
            large_search_limit: 20,
            small_search_limit: 4,
            client_config,
            app,
        }
    }

    #[allow(clippy::cognitive_complexity)]
    pub async fn handle_network_event(&mut self, io_event: IoEvent<'_>) {
        match io_event {
            IoEvent::RefreshAuthentication => self.refresh_authentication().await,
            IoEvent::GetPlaylists => self.get_current_user_playlists().await,
            IoEvent::GetUser => self.get_user().await,
            IoEvent::GetDevices => self.get_devices().await,
            IoEvent::GetCurrentPlayback => self.get_current_playback().await,
            IoEvent::SetTracksToTable { tracks } => {
                self.set_items_to_table(tracks.into_iter().map(PlayableItem::Track).collect())
                    .await
            }
            IoEvent::GetSearchResults {
                search_term,
                country,
            } => self.get_search_results(search_term, country).await,
            IoEvent::GetMadeForYouPlaylistItems {
                playlist_id,
                offset,
            } => {
                self.get_made_for_you_playlist_items(playlist_id, offset)
                    .await
            }
            IoEvent::GetPlaylistItems {
                playlist_id,
                offset,
            } => self.get_playlist_items(playlist_id, offset).await,
            IoEvent::GetCurrentSavedTracks { offset } => {
                self.get_current_user_saved_tracks(offset).await
            }
            IoEvent::StartContextPlayback {
                play_context_id,
                offset,
            } => self.start_context_playback(play_context_id, offset).await,
            IoEvent::StartPlayablesPlayback {
                playable_ids,
                offset,
            } => self.start_playables_playback(playable_ids, offset).await,
            IoEvent::UpdateSearchLimits {
                large_search_limit,
                small_search_limit,
            } => {
                self.large_search_limit = large_search_limit;
                self.small_search_limit = small_search_limit;
            }
            IoEvent::Seek { position_ms } => self.seek(position_ms).await,
            IoEvent::NextTrack => self.next_track().await,
            IoEvent::PreviousTrack => self.previous_track().await,
            IoEvent::Repeat { state } => self.repeat(state).await,
            IoEvent::PausePlayback => self.pause_playback().await,
            IoEvent::ChangeVolume { volume } => self.change_volume(volume).await,
            IoEvent::GetArtist {
                artist_id,
                input_artist_name,
                country,
            } => self.get_artist(artist_id, input_artist_name, country).await,
            IoEvent::GetAlbumTracks { album } => self.get_album_tracks(album).await,
            IoEvent::GetRecommendationsForSeed {
                seed_artist_ids,
                seed_track_ids,
                first_track,
                country,
            } => {
                self.get_recommendations_for_seed(
                    seed_artist_ids,
                    seed_track_ids,
                    first_track,
                    country,
                )
                .await
            }
            IoEvent::GetCurrentUserSavedAlbums { offset } => {
                self.get_current_user_saved_albums(offset).await
            }
            IoEvent::CurrentUserSavedAlbumsContains { album_ids } => {
                self.current_user_saved_albums_contains(album_ids).await
            }
            IoEvent::CurrentUserSavedAlbumDelete { album_id } => {
                self.current_user_saved_album_delete(album_id).await
            }
            IoEvent::CurrentUserSavedAlbumAdd { album_id } => {
                self.current_user_saved_album_add(album_id).await
            }
            IoEvent::UserUnfollowArtists { artist_ids } => {
                self.user_unfollow_artists(artist_ids).await
            }
            IoEvent::UserFollowArtists { artist_ids } => self.user_follow_artists(artist_ids).await,
            IoEvent::UserFollowPlaylist {
                playlist_id,
                is_public,
            } => self.user_follow_playlist(playlist_id, is_public).await,
            IoEvent::UserUnfollowPlaylist { playlist_id } => {
                self.user_unfollow_playlist(playlist_id).await
            }
            IoEvent::MadeForYouSearchAndAdd {
                search_term,
                country,
            } => self.made_for_you_search_and_add(search_term, country).await,
            IoEvent::GetTrackAnalysis { track_id } => self.get_track_analysis(track_id).await,
            IoEvent::ToggleSaveEpisode { episode_id } => self.toggle_save_episode(episode_id).await,
            IoEvent::ToggleSaveTrack { track_id } => self.toggle_save_track(track_id).await,
            IoEvent::GetRecommendationsForTrackId { track_id, country } => {
                self.get_recommendations_for_track_id(track_id, country)
                    .await
            }
            IoEvent::GetRecentlyPlayed => self.get_recently_played().await,
            IoEvent::GetFollowedArtists { after } => self.get_followed_artists(after).await,
            IoEvent::SetArtistsToTable { artists } => self.set_artists_to_table(artists).await,
            IoEvent::UserArtistFollowCheck { artist_ids } => {
                self.user_artist_check_follow(artist_ids).await
            }
            IoEvent::GetAlbum { album_id } => self.get_album(album_id).await,
            IoEvent::TransferPlaybackToDevice { device_id } => {
                self.transfert_playback_to_device(device_id).await
            }
            IoEvent::GetAlbumForTrack { track_id } => self.get_album_for_track(track_id).await,
            IoEvent::ToggleShuffle => self.toggle_shuffle().await,
            IoEvent::CurrentUserSavedTracksContains { track_ids } => {
                self.current_user_saved_tracks_contains(track_ids).await
            }
            IoEvent::GetCurrentUserSavedShows { offset } => {
                self.get_current_user_saved_shows(offset).await
            }
            IoEvent::CurrentUserSavedShowsContains { show_ids } => {
                self.current_user_saved_shows_contains(show_ids).await
            }
            IoEvent::CurrentUserSavedShowDelete { show_id } => {
                self.current_user_saved_shows_delete(show_id).await
            }
            IoEvent::CurrentUserSavedShowAdd { show_id } => {
                self.current_user_saved_shows_add(show_id).await
            }
            IoEvent::GetShowEpisodes { show } => self.get_show_episodes(show).await,
            IoEvent::GetShow { show_id } => self.get_show(show_id).await,
            IoEvent::GetCurrentShowEpisodes { show_id, offset } => {
                self.get_current_show_episodes(show_id, offset).await
            }
            IoEvent::AddItemToQueue { playable_id } => self.add_item_to_queue(playable_id).await,
            IoEvent::ResumePlayback => self.resume_playback().await,
        };

        let mut app = self.app.lock().await;
        app.is_loading = false;
    }

    async fn handle_error(&mut self, e: anyhow::Error) {
        let mut app = self.app.lock().await;
        app.handle_error(e);
    }

    async fn add_item_to_queue(&mut self, playable_id: PlayableId<'_>) {
        handle_error!(
            self,
            self.spotify
                .add_item_to_queue(playable_id, self.client_config.device_id.as_deref())
                .await
        );
    }

    async fn get_user(&mut self) {
        let user = handle_error!(self, self.spotify.current_user().await);
        let mut app = self.app.lock().await;
        app.user = Some(user);
    }

    async fn get_devices(&mut self) {
        let devices = handle_error!(self, self.spotify.device().await);
        let mut app = self.app.lock().await;
        app.push_navigation_stack(RouteId::SelectedDevice, ActiveBlock::SelectDevice);
        if !devices.is_empty() {
            app.devices = Some(DevicePayload { devices });
            // Select the first device in the list
            app.selected_device_index = Some(0);
        }
    }

    async fn get_current_playback(&mut self) {
        let context = handle_error!(
            self,
            self.spotify
                .current_playback(
                    None,
                    Some(vec![&AdditionalType::Episode, &AdditionalType::Track]),
                )
                .await
        );

        let mut app = self.app.lock().await;
        app.instant_since_last_current_playback_poll = Instant::now();

        if let Some(context) = context {
            app.current_playback_context = Some(context.clone());
            if let Some(item) = context.item {
                match item {
                    PlayableItem::Track(track) => {
                        if let Some(track_id) = track.id {
                            app.dispatch(IoEvent::CurrentUserSavedTracksContains(vec![
                                track_id.to_string()
                            ]));
                        };
                    }
                    PlayableItem::Episode(episode) => {
                        app.dispatch(IoEvent::CurrentUserSavedShowsContains(vec![episode
                            .id
                            .to_string()]));
                    }
                }
            }
        }

        let mut app = self.app.lock().await;
        app.seek_ms.take();
        app.is_fetching_current_playback = false;
    }

    async fn current_user_saved_tracks_contains(&mut self, track_ids: Vec<TrackId<'_>>) {
        let is_saved_vec = handle_error!(
            self,
            self.spotify
                .current_user_saved_tracks_contains(track_ids.clone())
                .await
        );

        let mut app = self.app.lock().await;
        for (i, track_id) in track_ids.into_iter().map(TrackId::into_static).enumerate() {
            if let Some(is_liked) = is_saved_vec.get(i) {
                if *is_liked {
                    app.liked_song_ids_set.insert(track_id);
                } else {
                    // The song is not liked, so check if it should be removed
                    if app.liked_song_ids_set.contains(&track_id) {
                        app.liked_song_ids_set.remove(&track_id);
                    }
                }
            };
        }
    }

    async fn get_playlist_items(&mut self, playlist_id: PlaylistId<'_>, offset: u32) {
        let playlist_items = handle_error!(
            self,
            self.spotify
                .playlist_items_manual(
                    playlist_id,
                    None,
                    None,
                    Some(self.large_search_limit),
                    Some(offset),
                )
                .await
        );

        self.set_playlist_items_to_table(&playlist_items).await;

        let mut app = self.app.lock().await;
        app.playlist_items = Some(playlist_items);
        app.push_navigation_stack(RouteId::ItemTable, ActiveBlock::ItemTable);
    }

    async fn set_playlist_items_to_table(&mut self, playlist_item_page: &Page<PlaylistItem>) {
        self.set_items_to_table(
            playlist_item_page
                .items
                .clone()
                .into_iter()
                .filter_map(|item| item.track)
                .collect(),
        )
        .await;
    }

    async fn set_items_to_table(&mut self, tracks: Vec<PlayableItem>) {
        let mut app = self.app.lock().await;

        // Send this event round (don't block here)
        app.dispatch(IoEvent::CurrentUserSavedTracksContains {
            track_ids: tracks
                .iter()
                .filter_map(|item| item.id())
                .filter_map(|id| match id {
                    PlayableId::Track(track_id) => Some(track_id),
                    PlayableId::Episode(_) => None,
                })
                .map(|id| id.into_static())
                .collect(),
        });

        app.item_table.items == tracks;
    }

    async fn set_artists_to_table(&mut self, artists: Vec<FullArtist>) {
        let mut app = self.app.lock().await;
        app.artists = artists;
    }

    async fn get_made_for_you_playlist_items(&mut self, playlist_id: PlaylistId<'_>, offset: u32) {
        let made_for_you_tracks = handle_error!(
            self,
            self.spotify
                .playlist_items_manual(
                    playlist_id,
                    None,
                    None,
                    Some(self.large_search_limit),
                    Some(offset),
                )
                .await
        );

        self.set_playlist_items_to_table(&made_for_you_tracks).await;

        let mut app = self.app.lock().await;
        app.made_for_you_playlist_items = Some(made_for_you_tracks);
        if app.get_current_route().id != RouteId::ItemTable {
            app.push_navigation_stack(RouteId::ItemTable, ActiveBlock::ItemTable);
        }
    }

    async fn get_current_user_saved_shows(&mut self, offset: Option<u32>) {
        let saved_shows = handle_error!(
            self,
            self.spotify
                .get_saved_show_manual(Some(self.large_search_limit), offset)
                .await
        );

        // not to show a blank page
        if !saved_shows.items.is_empty() {
            let mut app = self.app.lock().await;
            app.library.saved_shows.add_pages(saved_shows);
        }
    }

    async fn current_user_saved_shows_contains(&mut self, show_ids: Vec<ShowId<'_>>) {
        let are_followed = handle_error!(
            self,
            self.spotify.check_users_saved_shows(show_ids.clone()).await
        );

        let mut app = self.app.lock().await;
        show_ids
            .into_iter()
            .map(ShowId::into_static)
            .enumerate()
            .for_each(|(i, show_id)| {
                if are_followed[i] {
                    app.saved_show_ids_set.insert(show_id);
                } else {
                    app.saved_show_ids_set.remove(&show_id);
                }
            })
    }

    async fn get_show_episodes(&mut self, show: Box<SimplifiedShow>) {
        let episodes = handle_error!(
            self,
            self.spotify
                .get_shows_episodes_manual(
                    show.id.clone(),
                    None,
                    Some(self.large_search_limit),
                    Some(0),
                )
                .await
        );

        if !episodes.items.is_empty() {
            let mut app = self.app.lock().await;
            app.library.show_episodes = ScrollableResultPages::default();
            app.library.show_episodes.add_pages(episodes);

            app.selected_show_simplified = Some(SelectedShow { show: *show });

            app.episode_table_context = EpisodeTableContext::Simplified;

            app.push_navigation_stack(RouteId::PodcastEpisodes, ActiveBlock::EpisodeTable);
        }
    }

    async fn get_show(&mut self, show_id: ShowId<'_>) {
        let show = handle_error!(self, self.spotify.get_a_show(show_id, None).await);

        let mut app = self.app.lock().await;

        app.selected_show_full = Some(SelectedFullShow { show });

        app.episode_table_context = EpisodeTableContext::Full;
        app.push_navigation_stack(RouteId::PodcastEpisodes, ActiveBlock::EpisodeTable);
    }

    async fn get_current_show_episodes(&mut self, show_id: ShowId<'_>, offset: Option<u32>) {
        let episodes = handle_error!(
            self,
            self.spotify
                .get_shows_episodes_manual(show_id, None, Some(self.large_search_limit), offset)
                .await
        );

        if !episodes.items.is_empty() {
            let mut app = self.app.lock().await;
            app.library.show_episodes.add_pages(episodes);
        }
    }

    async fn get_search_results(&mut self, search_term: String, country: Option<Country>) {
        let search_types = [
            SearchType::Track,
            SearchType::Artist,
            SearchType::Album,
            SearchType::Playlist,
            SearchType::Show,
            SearchType::Episode,
        ];
        let search_queries = search_types
            .into_iter()
            .map(|search_type| {
                self.spotify.search(
                    &search_term,
                    search_type,
                    country.map(Market::Country),
                    None,
                    Some(self.small_search_limit),
                    Some(0),
                )
            })
            .collect::<Vec<_>>();

        // Run the futures concurrently
        let search_results = handle_error!(self, try_join_all(search_queries).await);

        let mut app = self.app.lock().await;

        for search_result in search_results {
            match search_result {
                SearchResult::Tracks(track_results) => {
                    app.search_results.tracks = Some(track_results);
                }
                SearchResult::Artists(artist_results) => {
                    let artist_ids = artist_results
                        .items
                        .iter()
                        .map(|item| item.id.clone())
                        .collect();

                    // Check if these artists are followed
                    app.dispatch(IoEvent::UserArtistFollowCheck { artist_ids });

                    app.search_results.artists = Some(artist_results);
                }
                SearchResult::Albums(album_results) => {
                    let album_ids = album_results
                        .items
                        .iter()
                        .filter_map(|album| album.id.clone())
                        .collect();

                    // Check if these albums are saved
                    app.dispatch(IoEvent::CurrentUserSavedAlbumsContains { album_ids });

                    app.search_results.albums = Some(album_results);
                }
                SearchResult::Playlists(playlist_results) => {
                    app.search_results.playlists = Some(playlist_results);
                }
                SearchResult::Shows(show_results) => {
                    let show_ids = show_results
                        .items
                        .iter()
                        .map(|show| show.id.clone())
                        .collect();

                    // check if these shows are saved
                    app.dispatch(IoEvent::CurrentUserSavedShowsContains { show_ids });

                    app.search_results.shows = Some(show_results);
                }
                SearchResult::Episodes(episode_results) => {
                    app.search_results.episodes = Some(episode_results);
                }
            }
        }
    }

    async fn get_current_user_saved_tracks(&mut self, offset: Option<u32>) {
        let saved_tracks = handle_error!(
            self,
            self.spotify
                .current_user_saved_tracks_manual(None, Some(self.large_search_limit), offset)
                .await
        );

        let mut app = self.app.lock().await;
        app.item_table.items = saved_tracks
            .items
            .clone()
            .into_iter()
            .map(|item| PlayableItem::Track(item.track))
            .collect::<Vec<_>>();

        saved_tracks.items.iter().for_each(|item| {
            if let Some(track_id) = &item.track.id {
                app.liked_song_ids_set
                    .insert(track_id.clone().into_static());
            }
        });

        app.library.saved_tracks.add_pages(saved_tracks);
        app.item_table.context = Some(ItemTableContext::SavedTracks);
    }

    async fn start_context_playback(
        &mut self,
        play_context_id: PlayContextId<'_>,
        offset: Option<u32>,
    ) {
        let device_id = self.client_config.device_id.as_deref();

        // Offset::Position is not a straightforward enum variant because it uses a Duration
        // to represent an index (unclear why rspotify chose to do this) -- the methods
        // OAuthClient::start_context_playback and OAuthClient::start_uris_playback both use
        // the duration in Offset::Position's milliseconds as the provided position
        let offset = offset.map(|o| Offset::Position(Duration::milliseconds(o as i64)));

        handle_error!(
            self,
            self.spotify
                .start_context_playback(play_context_id, device_id, offset, None)
                .await
        );

        let mut app = self.app.lock().await;
        app.song_progress_ms = 0;
        app.dispatch(IoEvent::GetCurrentPlayback);
    }

    async fn start_playables_playback(
        &mut self,
        playable_ids: Vec<PlayableId<'_>>,
        offset: Option<u32>,
    ) {
        let device_id = self.client_config.device_id.as_deref();

        // Offset::Position is not a straightforward enum variant because it uses a Duration
        // to represent an index (unclear why rspotify chose to do this) -- the methods
        // OAuthClient::start_context_playback and OAuthClient::start_uris_playback both use
        // the duration in Offset::Position's milliseconds as the provided position
        let offset = offset.map(|o| Offset::Position(Duration::milliseconds(o as i64)));

        handle_error!(
            self,
            self.spotify
                .start_uris_playback(playable_ids, device_id, offset, None)
                .await
        );

        let mut app = self.app.lock().await;
        app.song_progress_ms = 0;
        app.dispatch(IoEvent::GetCurrentPlayback);
    }

    async fn seek(&mut self, position_ms: u32) {
        if let Some(device_id) = &self.client_config.device_id {
            handle_error!(
                self,
                self.spotify
                    .seek_track(Duration::milliseconds(position_ms as i64), Some(&device_id))
                    .await
            );

            // Wait between seek and status query.
            // Without it, the Spotify API may return the old progress.
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            self.get_current_playback().await;
        }
    }

    async fn next_track(&mut self) {
        handle_error!(
            self,
            self.spotify
                .next_track(self.client_config.device_id.as_deref())
                .await
        );
        self.get_current_playback().await;
    }

    async fn previous_track(&mut self) {
        handle_error!(
            self,
            self.spotify
                .previous_track(self.client_config.device_id.as_deref())
                .await
        );
        self.get_current_playback().await;
    }

    async fn toggle_shuffle(&mut self) {
        let shuffle_state = {
            self.app
                .lock()
                .await
                .current_playback_context
                .as_ref()
                .map(|c| c.shuffle_state)
                .unwrap_or_default()
        };

        handle_error!(
            self,
            self.spotify
                .shuffle(!shuffle_state, self.client_config.device_id.as_deref())
                .await
        );
        // Update the UI eagerly (otherwise the UI will wait until the next 5 second interval
        // due to polling playback context)
        let mut app = self.app.lock().await;
        if let Some(current_playback_context) = &mut app.current_playback_context {
            current_playback_context.shuffle_state = !shuffle_state;
        };
    }

    async fn repeat(&mut self, repeat_state: RepeatState) {
        let next_repeat_state = match repeat_state {
            RepeatState::Off => RepeatState::Context,
            RepeatState::Context => RepeatState::Track,
            RepeatState::Track => RepeatState::Off,
        };
        handle_error!(
            self,
            self.spotify
                .repeat(next_repeat_state, self.client_config.device_id.as_deref())
                .await
        );
        let mut app = self.app.lock().await;
        if let Some(current_playback_context) = &mut app.current_playback_context {
            current_playback_context.repeat_state = next_repeat_state;
        };
    }

    async fn pause_playback(&mut self) {
        handle_error!(
            self,
            self.spotify
                .pause_playback(self.client_config.device_id.as_deref())
                .await
        );
        self.get_current_playback().await;
    }

    async fn resume_playback(&mut self) {
        handle_error!(
            self,
            self.spotify
                .resume_playback(self.client_config.device_id.as_deref(), None)
                .await
        );
        self.get_current_playback().await;
    }

    async fn change_volume(&mut self, volume_percent: u8) {
        handle_error!(
            self,
            self.spotify
                .volume(volume_percent, self.client_config.device_id.as_deref())
                .await
        );
        let mut app = self.app.lock().await;
        if let Some(current_playback_context) = &mut app.current_playback_context {
            current_playback_context.device.volume_percent = Some(volume_percent.into());
        };
    }

    async fn get_artist(
        &mut self,
        artist_id: ArtistId<'_>,
        input_artist_name: String,
        country: Option<Country>,
    ) {
        let market = country.map(Market::Country);

        let (albums, top_tracks, related_artists, artist_name) = handle_error!(
            self,
            try_join!(
                self.spotify.artist_albums_manual(
                    artist_id.clone(),
                    [],
                    market,
                    Some(self.large_search_limit),
                    Some(0),
                ),
                self.spotify.artist_top_tracks(artist_id.clone(), market),
                self.spotify.artist_related_artists(artist_id.clone()),
                async {
                    if input_artist_name.is_empty() {
                        self.spotify
                            .artist(artist_id)
                            .await
                            .map(|full_artist| full_artist.name)
                    } else {
                        Ok(input_artist_name)
                    }
                }
            )
        );

        let mut app = self.app.lock().await;

        app.dispatch(IoEvent::CurrentUserSavedAlbumsContains(
            albums
                .items
                .iter()
                .filter_map(|item| item.id.as_ref())
                .map(|id| id.to_string())
                .collect(),
        ));

        app.artist = Some(Artist {
            artist_name,
            albums,
            related_artists,
            top_tracks,
            selected_album_index: 0,
            selected_related_artist_index: 0,
            selected_top_track_index: 0,
            artist_hovered_block: ArtistBlock::TopTracks,
            artist_selected_block: ArtistBlock::Empty,
        });
    }

    async fn get_album_tracks(&mut self, album: Box<SimplifiedAlbum>) {
        let album_id = match album.id.clone() {
            Some(album_id) => album_id,
            None => return,
        };

        let tracks = handle_error!(
            self,
            self.spotify
                .album_track_manual(album_id, None, Some(self.large_search_limit), Some(0),)
                .await
        );

        let track_ids = tracks
            .items
            .iter()
            .filter_map(|item| item.id.as_ref())
            .map(|id| id.to_string())
            .collect::<Vec<_>>();

        let mut app = self.app.lock().await;
        app.selected_album_simplified = Some(SelectedAlbum {
            album: *album,
            tracks,
            selected_index: 0,
        });

        app.album_table_context = AlbumTableContext::Simplified;
        app.push_navigation_stack(RouteId::AlbumTracks, ActiveBlock::AlbumTracks);
        app.dispatch(IoEvent::CurrentUserSavedTracksContains(track_ids));
    }

    async fn get_recommendations_for_seed(
        &mut self,
        seed_artist_ids: Option<Vec<ArtistId<'_>>>,
        seed_track_ids: Option<Vec<TrackId<'_>>>,
        first_track: Box<Option<FullTrack>>,
        country: Option<Country>,
    ) {
        let recommendations = handle_error!(
            self,
            self.spotify
                .recommendations(
                    [],
                    seed_artist_ids,
                    None::<[&str; 0]>,
                    seed_track_ids,
                    country.map(Market::Country),
                    Some(self.large_search_limit),
                )
                .await
        );

        if let Some(mut recommended_tracks) =
            self.extract_recommended_tracks(&recommendations).await
        {
            //custom first track
            if let Some(track) = *first_track {
                recommended_tracks.insert(0, track);
            }

            self.set_items_to_table(
                recommended_tracks
                    .clone()
                    .into_iter()
                    .map(PlayableItem::Track)
                    .collect(),
            )
            .await;

            let track_ids = recommended_tracks
                .iter()
                .filter_map(|track| track.id.clone())
                .map(|id| id.into_static())
                .collect::<Vec<_>>();

            let mut app = self.app.lock().await;
            app.recommended_tracks = recommended_tracks;
            app.item_table.context = Some(ItemTableContext::RecommendedTracks);

            if app.get_current_route().id != RouteId::Recommendations {
                app.push_navigation_stack(RouteId::Recommendations, ActiveBlock::ItemTable);
            };

            app.dispatch(IoEvent::StartPlayablesPlayback {
                playable_ids: track_ids.into_iter().map(PlayableId::Track).collect(),
                offset: Some(0),
            });
        }
    }

    async fn extract_recommended_tracks(
        &mut self,
        recommendations: &Recommendations,
    ) -> Option<Vec<FullTrack>> {
        let track_ids = recommendations
            .tracks
            .iter()
            .filter_map(|track| track.id.clone())
            .collect::<Vec<_>>();

        self.spotify.tracks(track_ids, None).await.ok()
    }

    async fn get_recommendations_for_track_id(
        &mut self,
        track_id: TrackId<'_>,
        country: Option<Country>,
    ) {
        let track = handle_error!(self, self.spotify.track(track_id.clone(), None).await);
        self.get_recommendations_for_seed(
            None,
            Some(vec![track_id]),
            Box::new(Some(track)),
            country,
        )
        .await;
    }

    async fn toggle_save_episode(&mut self, _: EpisodeId<'_>) {
        handle_error!(self, Err("cannot save episodes currently"));
        // let saved = handle_error!(
        //     self,
        //     self.spotify
        //         .current_user_saved_episodes_contains([episode_id.clone()])
        //         .await
        // );
        // match saved.first().copied().unwrap_or_default() {
        //     true => {
        //         handle_error!(
        //             self,
        //             self.spotify
        //                 .current_user_saved_episodes_delete([episode_id.clone()])
        //                 .await
        //         );
        //         let mut app = self.app.lock().await;
        //         app.liked_song_ids_set.remove(&episode_id.into_static());
        //     }
        //     false => {
        //         handle_error!(
        //             self,
        //             self.spotify
        //                 .current_user_saved_episodes_add([episode_id.clone()])
        //                 .await
        //         );
        //         // TODO: This should ideally use the same logic as `self.current_user_saved_episodes_contains`
        //         let mut app = self.app.lock().await;
        //         app.liked_song_ids_set.insert(episode_id.into_static());
        //     }
        // }
    }

    async fn toggle_save_track(&mut self, track_id: TrackId<'_>) {
        let saved = handle_error!(
            self,
            self.spotify
                .current_user_saved_tracks_contains([track_id.clone()])
                .await
        );
        match saved.first().copied().unwrap_or_default() {
            true => {
                handle_error!(
                    self,
                    self.spotify
                        .current_user_saved_tracks_delete([track_id.clone()])
                        .await
                );
                let mut app = self.app.lock().await;
                app.liked_song_ids_set.remove(&track_id.into_static());
            }
            false => {
                handle_error!(
                    self,
                    self.spotify
                        .current_user_saved_tracks_add([track_id.clone()])
                        .await
                );
                // TODO: This should ideally use the same logic as `self.current_user_saved_tracks_contains`
                let mut app = self.app.lock().await;
                app.liked_song_ids_set.insert(track_id.into_static());
            }
        }
    }

    async fn get_followed_artists(&mut self, after: Option<ArtistId<'_>>) {
        let after = after.map(|x| x.to_string());
        let saved_artists = handle_error!(
            self,
            self.spotify
                .current_user_followed_artists(after.as_deref(), Some(self.large_search_limit))
                .await
        );
        let mut app = self.app.lock().await;
        app.artists = saved_artists.items.to_owned();
        app.library.saved_artists.add_pages(saved_artists);
    }

    async fn user_artist_check_follow(&mut self, artist_ids: Vec<ArtistId<'_>>) {
        let are_followed = handle_error!(
            self,
            self.spotify
                .user_artist_check_follow(artist_ids.clone())
                .await
        );

        let mut app = self.app.lock().await;
        artist_ids
            .into_iter()
            .map(ArtistId::into_static)
            .enumerate()
            .for_each(|(i, artist_id)| {
                if are_followed[i] {
                    app.followed_artist_ids_set.insert(artist_id);
                } else {
                    app.followed_artist_ids_set.remove(&artist_id);
                }
            });
    }

    async fn get_current_user_saved_albums(&mut self, offset: Option<u32>) {
        let saved_albums = handle_error!(
            self,
            self.spotify
                .current_user_saved_albums_manual(None, Some(self.large_search_limit), offset)
                .await
        );
        // not to show a blank page
        if !saved_albums.items.is_empty() {
            let mut app = self.app.lock().await;
            app.library.saved_albums.add_pages(saved_albums);
        }
    }

    async fn current_user_saved_albums_contains(&mut self, album_ids: Vec<AlbumId<'_>>) {
        let are_followed = handle_error!(
            self,
            self.spotify
                .current_user_saved_albums_contains(album_ids.clone())
                .await
        );
        let mut app = self.app.lock().await;
        album_ids
            .into_iter()
            .map(AlbumId::into_static)
            .enumerate()
            .for_each(|(i, album_id)| {
                if are_followed[i] {
                    app.saved_album_ids_set.insert(album_id);
                } else {
                    app.saved_album_ids_set.remove(&album_id);
                }
            });
    }

    async fn current_user_saved_album_delete(&mut self, album_id: AlbumId<'_>) {
        handle_error!(
            self,
            self.spotify
                .current_user_saved_albums_delete([album_id.clone()])
                .await
        );
        self.get_current_user_saved_albums(None).await;
        let mut app = self.app.lock().await;
        app.saved_album_ids_set.remove(&album_id.into_static());
    }

    async fn current_user_saved_album_add(&mut self, album_id: AlbumId<'_>) {
        handle_error!(
            self,
            self.spotify
                .current_user_saved_albums_add([album_id.clone()])
                .await
        );
        let mut app = self.app.lock().await;
        app.saved_album_ids_set.insert(album_id.into_static());
    }

    async fn current_user_saved_shows_delete(&mut self, show_id: ShowId<'_>) {
        handle_error!(
            self,
            self.spotify
                .remove_users_saved_shows(vec![show_id.clone()], None)
                .await
        );
        self.get_current_user_saved_shows(None).await;
        let mut app = self.app.lock().await;
        app.saved_show_ids_set.remove(&show_id.into_static());
    }

    async fn current_user_saved_shows_add(&mut self, show_id: ShowId<'_>) {
        handle_error!(self, self.spotify.save_shows(vec![show_id.clone()]).await);
        self.get_current_user_saved_shows(None).await;
        let mut app = self.app.lock().await;
        app.saved_show_ids_set.insert(show_id.into_static());
    }

    async fn user_unfollow_artists(&mut self, artist_ids: Vec<ArtistId<'_>>) {
        handle_error!(
            self,
            self.spotify.user_unfollow_artists(artist_ids.clone()).await
        );
        self.get_followed_artists(None).await;
        let mut app = self.app.lock().await;
        artist_ids
            .into_iter()
            .map(ArtistId::into_static)
            .for_each(|artist_id| {
                app.followed_artist_ids_set.remove(&artist_id);
            });
    }

    async fn user_follow_artists(&mut self, artist_ids: Vec<ArtistId<'_>>) {
        handle_error!(
            self,
            self.spotify.user_follow_artists(artist_ids.clone()).await
        );
        self.get_followed_artists(None).await;
        let mut app = self.app.lock().await;
        artist_ids
            .into_iter()
            .map(ArtistId::into_static)
            .for_each(|artist_id| {
                app.followed_artist_ids_set.insert(artist_id);
            });
    }

    async fn user_follow_playlist(&mut self, playlist_id: PlaylistId<'_>, is_public: Option<bool>) {
        handle_error!(
            self,
            self.spotify.playlist_follow(playlist_id, is_public).await
        );
        self.get_current_user_playlists().await;
    }

    async fn user_unfollow_playlist(&mut self, playlist_id: PlaylistId<'_>) {
        handle_error!(self, self.spotify.playlist_unfollow(playlist_id).await);
        self.get_current_user_playlists().await;
    }

    async fn made_for_you_search_and_add(
        &mut self,
        search_string: String,
        country: Option<Country>,
    ) {
        static SPOTIFY_ID: UserId<'static> = UserId::from_id("spotify").unwrap();

        let SearchResult::Playlists(mut search_playlists) = handle_error!(
            self,
            self.spotify
                .search(
                    &search_string,
                    SearchType::Playlist,
                    country.map(Market::Country),
                    None,
                    Some(self.large_search_limit),
                    Some(0),
                )
                .await
        ) else {
            unreachable!();
        };

        let mut filtered_playlists = search_playlists
            .items
            .iter()
            .filter(|playlist| playlist.owner.id == SPOTIFY_ID && playlist.name == search_string)
            .map(|playlist| playlist.to_owned())
            .collect::<Vec<SimplifiedPlaylist>>();

        let mut app = self.app.lock().await;
        if !app.library.made_for_you_playlists.pages.is_empty() {
            app.library
                .made_for_you_playlists
                .get_mut_results(None)
                .unwrap()
                .items
                .append(&mut filtered_playlists);
        } else {
            search_playlists.items = filtered_playlists;
            app.library
                .made_for_you_playlists
                .add_pages(search_playlists);
        }
    }

    async fn get_track_analysis(&mut self, track_id: TrackId<'_>) {
        let result = handle_error!(self, self.spotify.track_analysis(track_id).await);
        let mut app = self.app.lock().await;
        app.audio_analysis = Some(result);
    }

    async fn get_current_user_playlists(&mut self) {
        let playlists = handle_error!(
            self,
            self.spotify
                .current_user_playlists_manual(Some(self.large_search_limit), None)
                .await
        );

        let mut app = self.app.lock().await;
        app.playlists = Some(playlists);
        // Select the first playlist
        app.selected_playlist_index = Some(0);
    }

    async fn get_recently_played(&mut self) {
        let result = handle_error!(
            self,
            self.spotify
                .current_user_recently_played(Some(self.large_search_limit), None)
                .await
        );

        let track_ids = result
            .items
            .iter()
            .filter_map(|item| item.track.id.clone())
            .collect::<Vec<_>>();

        self.current_user_saved_tracks_contains(track_ids).await;

        let mut app = self.app.lock().await;

        app.recently_played.result = Some(result.clone());
    }

    async fn get_album(&mut self, album_id: AlbumId<'_>) {
        let album = handle_error!(self, self.spotify.album(album_id, None).await);

        let mut app = self.app.lock().await;

        app.selected_album_full = Some(SelectedFullAlbum {
            album,
            selected_index: 0,
        });
        app.album_table_context = AlbumTableContext::Full;
        app.push_navigation_stack(RouteId::AlbumTracks, ActiveBlock::AlbumTracks);
    }

    async fn get_album_for_track(&mut self, track_id: TrackId<'_>) {
        let track = handle_error!(self, self.spotify.track(track_id, None).await);

        // It is unclear when the id can ever be None, but perhaps a track can be album-less. If
        // so, there isn't much to do here anyways, since we're looking for the parent album.
        let Some(album_id) = track.album.id else {
            return;
        };

        let album = handle_error!(self, self.spotify.album(album_id, None).await);

        // The way we map to the UI is zero-indexed, but Spotify is 1-indexed.
        let zero_indexed_track_number = track.track_number - 1;
        let selected_album = SelectedFullAlbum {
            album,
            // Overflow should be essentially impossible here, so we prefer the cleaner 'as'.
            selected_index: zero_indexed_track_number as usize,
        };

        let mut app = self.app.lock().await;

        app.selected_album_full = Some(selected_album.clone());
        app.saved_album_tracks_index = selected_album.selected_index;
        app.album_table_context = AlbumTableContext::Full;
        app.push_navigation_stack(RouteId::AlbumTracks, ActiveBlock::AlbumTracks);
    }

    async fn transfert_playback_to_device(&mut self, device_id: String) {
        handle_error!(
            self,
            self.spotify.transfer_playback(&device_id, Some(true)).await
        );
        self.get_current_playback().await;

        handle_error!(self, self.client_config.set_device_id(device_id));
        let mut app = self.app.lock().await;
        app.pop_navigation_stack();
    }

    async fn refresh_authentication(&mut self) {
        if let Some(new_token) = get_token(&mut self.oauth).await {
            let (new_spotify, new_token_expiry) = get_spotify(new_token);
            self.spotify = new_spotify;
            let mut app = self.app.lock().await;
            app.spotify_token_expiry = new_token_expiry;
        } else {
            println!("\nFailed to refresh authentication token");
            // TODO panic!
        }
    }
}
