use super::user_config::UserConfig;
use crate::network::IoEvent;
use anyhow::anyhow;
use arboard::Clipboard;
use chrono::{DateTime, Utc};
use derivative::Derivative;
use rspotify::model::{
    album::{FullAlbum, SavedAlbum, SimplifiedAlbum},
    artist::FullArtist,
    audio::AudioAnalysis,
    context::CurrentPlaybackContext,
    device::DevicePayload,
    enums::Country,
    idtypes::{Id, PlayContextId},
    page::{CursorBasedPage, Page},
    playing::PlayHistory,
    playlist::{PlaylistItem, SimplifiedPlaylist},
    show::{FullShow, Show, SimplifiedEpisode, SimplifiedShow},
    track::{FullTrack, SavedTrack, SimplifiedTrack},
    user::PrivateUser,
    AlbumId, ArtistId, EpisodeId, PlayableItem, ShowId, TrackId,
};
use spotify_tui_util::{PlaybleItemExt, ToStatic};
use std::{
    cmp::{max, min},
    collections::HashSet,
    time::Instant,
};
use tokio::sync::mpsc::UnboundedSender;
use tui::layout::Rect;

pub const LIBRARY_OPTIONS: [&str; 6] = [
    "Made For You",
    "Recently Played",
    "Liked Songs",
    "Albums",
    "Artists",
    "Podcasts",
];

const DEFAULT_ROUTE: Route = Route {
    id: RouteId::Home,
    active_block: ActiveBlock::Empty,
    hovered_block: ActiveBlock::Library,
};

#[derive(Clone, Derivative)]
#[derivative(Default(bound = ""))]
pub struct ScrollableResultPages<T> {
    index: usize,
    pub pages: Vec<T>,
}

impl<T> ScrollableResultPages<T> {
    pub fn get_results(&self, at_index: Option<usize>) -> Option<&T> {
        self.pages.get(at_index.unwrap_or(self.index))
    }

    pub fn get_mut_results(&mut self, at_index: Option<usize>) -> Option<&mut T> {
        self.pages.get_mut(at_index.unwrap_or(self.index))
    }

    pub fn add_pages(&mut self, new_pages: T) {
        self.pages.push(new_pages);
        // Whenever a new page is added, set the active index to the end of the vector
        self.index = self.pages.len() - 1;
    }
}

#[derive(Default)]
pub struct SpotifyResultAndSelectedIndex<T> {
    pub index: usize,
    pub result: T,
}

#[derive(Clone, Default)]
pub struct Library {
    pub selected_index: usize,
    pub saved_tracks: ScrollableResultPages<Page<SavedTrack>>,
    pub made_for_you_playlists: ScrollableResultPages<Page<SimplifiedPlaylist>>,
    pub saved_albums: ScrollableResultPages<Page<SavedAlbum>>,
    pub saved_shows: ScrollableResultPages<Page<Show>>,
    pub saved_artists: ScrollableResultPages<CursorBasedPage<FullArtist>>,
    pub show_episodes: ScrollableResultPages<Page<SimplifiedEpisode>>,
}

#[derive(PartialEq, Debug)]
pub enum SearchResultBlock {
    AlbumSearch,
    SongSearch,
    ArtistSearch,
    PlaylistSearch,
    ShowSearch,
    Empty,
}

#[derive(PartialEq, Debug, Clone)]
pub enum ArtistBlock {
    TopTracks,
    Albums,
    RelatedArtists,
    Empty,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DialogContext {
    PlaylistWindow,
    PlaylistSearch,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActiveBlock {
    Analysis,
    PlayBar,
    AlbumTracks,
    AlbumList,
    ArtistBlock,
    Empty,
    Error,
    HelpMenu,
    Home,
    Input,
    Library,
    MyPlaylists,
    Podcasts,
    EpisodeTable,
    RecentlyPlayed,
    SearchResultBlock,
    SelectDevice,
    ItemTable,
    MadeForYou,
    Artists,
    BasicView,
    Dialog(DialogContext),
}

#[derive(Clone, PartialEq, Debug)]
pub enum RouteId {
    Analysis,
    AlbumTracks,
    AlbumList,
    Artist,
    BasicView,
    Error,
    Home,
    RecentlyPlayed,
    Search,
    SelectedDevice,
    ItemTable,
    MadeForYou,
    Artists,
    Podcasts,
    PodcastEpisodes,
    Recommendations,
    Dialog,
}

#[derive(Debug)]
pub struct Route {
    pub id: RouteId,
    pub active_block: ActiveBlock,
    pub hovered_block: ActiveBlock,
}

// Is it possible to compose enums?
#[derive(PartialEq, Debug)]
pub enum ItemTableContext {
    MyPlaylists,
    AlbumSearch,
    PlaylistSearch,
    SavedTracks,
    RecommendedTracks,
    MadeForYou,
}

// Is it possible to compose enums?
#[derive(Clone, PartialEq, Debug, Copy)]
pub enum AlbumTableContext {
    Simplified,
    Full,
}

#[derive(Clone, PartialEq, Debug, Copy)]
pub enum EpisodeTableContext {
    Simplified,
    Full,
}

#[derive(Clone, PartialEq, Debug)]
pub enum RecommendationsContext {
    Artist,
    Song,
}

#[derive(Derivative)]
#[derivative(Default)]
pub struct SearchResult {
    pub albums: Option<Page<SimplifiedAlbum>>,
    pub artists: Option<Page<FullArtist>>,
    pub playlists: Option<Page<SimplifiedPlaylist>>,
    pub tracks: Option<Page<FullTrack>>,
    pub shows: Option<Page<SimplifiedShow>>,
    pub episodes: Option<Page<SimplifiedEpisode>>,
    pub selected_album_index: Option<usize>,
    pub selected_artists_index: Option<usize>,
    pub selected_playlists_index: Option<usize>,
    pub selected_tracks_index: Option<usize>,
    pub selected_shows_index: Option<usize>,
    #[derivative(Default(value = "SearchResultBlock::SongSearch"))]
    pub hovered_block: SearchResultBlock,
    #[derivative(Default(value = "SearchResultBlock::Empty"))]
    pub selected_block: SearchResultBlock,
}

#[derive(Default)]
pub struct ItemTable {
    pub items: Vec<PlayableItem>,
    pub selected_index: usize,
    pub context: Option<ItemTableContext>,
}

#[derive(Clone)]
pub struct SelectedShow {
    pub show: SimplifiedShow,
}

#[derive(Clone)]
pub struct SelectedFullShow {
    pub show: FullShow,
}

#[derive(Clone)]
pub struct SelectedAlbum {
    pub album: SimplifiedAlbum,
    pub tracks: Page<SimplifiedTrack>,
    pub selected_index: usize,
}

#[derive(Clone)]
pub struct SelectedFullAlbum {
    pub album: FullAlbum,
    pub selected_index: usize,
}

#[derive(Clone)]
pub struct Artist {
    pub artist_name: String,
    pub albums: Page<SimplifiedAlbum>,
    pub related_artists: Vec<FullArtist>,
    pub top_tracks: Vec<FullTrack>,
    pub selected_album_index: usize,
    pub selected_related_artist_index: usize,
    pub selected_top_track_index: usize,
    pub artist_hovered_block: ArtistBlock,
    pub artist_selected_block: ArtistBlock,
}

#[derive(Derivative)]
#[derivative(Default)]
pub struct App {
    #[derivative(Default(value = "Instant::now()"))]
    pub instant_since_last_current_playback_poll: Instant,
    #[derivative(Default(value = "vec![DEFAULT_ROUTE]"))]
    navigation_stack: Vec<Route>,
    pub audio_analysis: Option<AudioAnalysis>,
    pub home_scroll: u16,
    #[derivative(Default(value = "UserConfig::new()"))]
    pub user_config: UserConfig,
    pub artists: Vec<FullArtist>,
    pub artist: Option<Artist>,
    #[derivative(Default(value = "AlbumTableContext::Full"))]
    pub album_table_context: AlbumTableContext,
    pub saved_album_tracks_index: usize,
    pub api_error: String,
    pub current_playback_context: Option<CurrentPlaybackContext>,
    pub devices: Option<DevicePayload>,
    // Inputs:
    // input is the string for input;
    // input_idx is the index of the cursor in terms of character;
    // input_cursor_position is the sum of the width of characters preceding the cursor.
    // Reason for this complication is due to non-ASCII characters, they may
    // take more than 1 bytes to store and more than 1 character width to display.
    pub input: Vec<char>,
    pub input_idx: usize,
    pub input_cursor_position: u16,
    pub liked_episode_ids_set: HashSet<EpisodeId<'static>>,
    pub liked_song_ids_set: HashSet<TrackId<'static>>,
    pub followed_artist_ids_set: HashSet<ArtistId<'static>>,
    pub saved_album_ids_set: HashSet<AlbumId<'static>>,
    pub saved_show_ids_set: HashSet<ShowId<'static>>,
    #[derivative(Default(value = "20"))]
    pub large_search_limit: u32,
    pub library: Library,
    pub playlist_offset: u32,
    pub made_for_you_offset: u32,
    pub playlist_items: Option<Page<PlaylistItem>>,
    pub made_for_you_playlist_items: Option<Page<PlaylistItem>>,
    pub playlists: Option<Page<SimplifiedPlaylist>>,
    pub recently_played: SpotifyResultAndSelectedIndex<Option<CursorBasedPage<PlayHistory>>>,
    pub recommended_tracks: Vec<FullTrack>,
    pub recommendations_seed: String,
    pub recommendations_context: Option<RecommendationsContext>,
    pub search_results: SearchResult,
    pub selected_album_simplified: Option<SelectedAlbum>,
    pub selected_album_full: Option<SelectedFullAlbum>,
    pub selected_device_index: Option<usize>,
    pub selected_playlist_index: Option<usize>,
    pub active_playlist_index: Option<usize>,
    pub size: Rect,
    #[derivative(Default(value = "4"))]
    pub small_search_limit: u32,
    pub song_progress_ms: u128,
    pub seek_ms: Option<u128>,
    pub item_table: ItemTable,
    #[derivative(Default(value = "EpisodeTableContext::Full"))]
    pub episode_table_context: EpisodeTableContext,
    pub selected_show_simplified: Option<SelectedShow>,
    pub selected_show_full: Option<SelectedFullShow>,
    pub user: Option<PrivateUser>,
    pub album_list_index: usize,
    pub made_for_you_index: usize,
    pub artists_list_index: usize,
    #[derivative(Default(value = "Clipboard::new().ok()"))]
    pub clipboard: Option<Clipboard>,
    pub shows_list_index: usize,
    pub episode_list_index: usize,
    pub help_docs_size: u32,
    pub help_menu_page: u32,
    pub help_menu_max_lines: u32,
    pub help_menu_offset: u32,
    pub is_loading: bool,
    io_tx: Option<UnboundedSender<IoEvent<'static>>>,
    pub is_fetching_current_playback: bool,
    #[derivative(Default(value = "Utc::now()"))]
    pub spotify_token_expiry: DateTime<Utc>,
    pub dialog: Option<String>,
    pub confirm: bool,
}

macro_rules! handle_error {
    ($self:ident, $result:expr $(, |$err:ident| $err_expr:expr)?) => {
        match $result {
            Ok(ok) => ok,
            Err(err) => {
                let err = handle_error!(@ err $(, |$err| $err_expr)?);
                $self.handle_error(err);
                return;
            },
        }
    };
    (@ $err_ident:ident) => { anyhow!($err_ident) };
    (@ $err_ident:ident |$err:ident| $err_expr:expr) => {
        let $err = $err_ident;
        $err_expr
    };
}

impl App {
    pub fn new(
        io_tx: UnboundedSender<IoEvent<'static>>,
        user_config: UserConfig,
        spotify_token_expiry: DateTime<Utc>,
    ) -> App {
        App {
            io_tx: Some(io_tx),
            user_config,
            spotify_token_expiry,
            ..App::default()
        }
    }

    // Send a network event to the network thread
    pub fn dispatch(&mut self, event: IoEvent<'_>) {
        // `is_loading` will be set to false again after the async action has finished in network.rs
        self.is_loading = true;
        if let Some(io_tx) = &self.io_tx {
            if let Err(err) = io_tx.send(event.to_static()) {
                self.is_loading = false;
                println!("Error from dispatch: {err}");
                // TODO: handle error
            };
        }
    }

    fn apply_seek(&mut self, seek_ms: u32) {
        if let Some(CurrentPlaybackContext {
            item: Some(item), ..
        }) = &self.current_playback_context
        {
            let event = if seek_ms < item.duration().num_milliseconds() as u32 {
                IoEvent::Seek {
                    position_ms: seek_ms,
                }
            } else {
                IoEvent::NextTrack
            };

            self.dispatch(event);
        }
    }

    fn poll_current_playback(&mut self) {
        // Poll every 5 seconds
        let poll_interval_ms = 5_000;

        let elapsed = self
            .instant_since_last_current_playback_poll
            .elapsed()
            .as_millis();

        if !self.is_fetching_current_playback && elapsed >= poll_interval_ms {
            self.is_fetching_current_playback = true;
            // Trigger the seek if the user has set a new position
            match self.seek_ms {
                Some(seek_ms) => self.apply_seek(seek_ms as u32),
                None => self.dispatch(IoEvent::GetCurrentPlayback),
            }
        }
    }

    pub fn update_on_tick(&mut self) {
        self.poll_current_playback();
        if let Some(CurrentPlaybackContext {
            item: Some(item),
            progress: Some(progress),
            is_playing,
            ..
        }) = &self.current_playback_context
        {
            // Update progress even when the song is not playing,
            // because seeking is possible while paused
            let elapsed = if *is_playing {
                self.instant_since_last_current_playback_poll
                    .elapsed()
                    .as_millis()
            } else {
                0u128
            } + progress.num_milliseconds() as u128;

            if elapsed < item.duration().num_milliseconds() as u128 {
                self.song_progress_ms = elapsed;
            } else {
                self.song_progress_ms = item.duration().num_milliseconds() as u128;
            }
        }
    }

    pub fn seek_forwards(&mut self) {
        if let Some(CurrentPlaybackContext {
            item: Some(item), ..
        }) = &self.current_playback_context
        {
            let old_progress = match self.seek_ms {
                Some(seek_ms) => seek_ms,
                None => self.song_progress_ms,
            };

            let new_progress = min(
                old_progress as u32 + self.user_config.behavior.seek_milliseconds,
                item.duration().num_milliseconds() as u32,
            );

            self.seek_ms = Some(new_progress as u128);
        }
    }

    pub fn seek_backwards(&mut self) {
        let old_progress = match self.seek_ms {
            Some(seek_ms) => seek_ms,
            None => self.song_progress_ms,
        };
        let new_progress = if old_progress as u32 > self.user_config.behavior.seek_milliseconds {
            old_progress as u32 - self.user_config.behavior.seek_milliseconds
        } else {
            0u32
        };
        self.seek_ms = Some(new_progress as u128);
    }

    pub fn get_recommendations_for_seed(
        &mut self,
        seed_artist_ids: Option<Vec<ArtistId<'_>>>,
        seed_track_ids: Option<Vec<TrackId<'_>>>,
        first_track: Option<FullTrack>,
    ) {
        let country = self.get_user_country();
        self.dispatch(IoEvent::GetRecommendationsForSeed {
            seed_artist_ids,
            seed_track_ids,
            country,
            first_track: Box::new(first_track),
        });
    }

    pub fn get_recommendations_for_track_id(&mut self, track_id: TrackId<'_>) {
        let country = self.get_user_country();
        self.dispatch(IoEvent::GetRecommendationsForTrackId { track_id, country });
    }

    pub fn increase_volume(&mut self) {
        if let Some(context) = self.current_playback_context.clone() {
            let current_volume = context.device.volume_percent.unwrap_or_default() as u8;
            let next_volume = min(
                current_volume + self.user_config.behavior.volume_increment,
                100,
            );

            if next_volume != current_volume {
                self.dispatch(IoEvent::ChangeVolume {
                    volume: next_volume,
                });
            }
        }
    }

    pub fn decrease_volume(&mut self) {
        if let Some(context) = self.current_playback_context.clone() {
            let current_volume = context.device.volume_percent.unwrap_or_default() as i8;
            let next_volume = max(
                current_volume - self.user_config.behavior.volume_increment as i8,
                0,
            );

            if next_volume != current_volume {
                self.dispatch(IoEvent::ChangeVolume {
                    volume: next_volume as u8,
                });
            }
        }
    }

    pub fn handle_error(&mut self, e: anyhow::Error) {
        self.push_navigation_stack(RouteId::Error, ActiveBlock::Error);
        self.api_error = e.to_string();
    }

    pub fn is_playing(&self) -> bool {
        let Some(CurrentPlaybackContext { is_playing, .. }) = &self.current_playback_context else {
            return false;
        };
        *is_playing
    }

    pub fn toggle_playback(&mut self) {
        if let Some(CurrentPlaybackContext {
            is_playing: true, ..
        }) = &self.current_playback_context
        {
            self.dispatch(IoEvent::PausePlayback);
        } else {
            // When no offset or uris are passed, spotify will resume current playback
            self.dispatch(IoEvent::ResumePlayback);
        }
    }

    pub fn resume_playback(&mut self) {
        if let Some(CurrentPlaybackContext {
            is_playing: false, ..
        }) = &self.current_playback_context
        {
            self.dispatch(IoEvent::ResumePlayback);
        }
    }

    pub fn pause_playback(&mut self) {
        if let Some(CurrentPlaybackContext {
            is_playing: true, ..
        }) = &self.current_playback_context
        {
            self.dispatch(IoEvent::PausePlayback);
        }
    }

    pub fn previous_track(&mut self) {
        if self.song_progress_ms >= 3_000 {
            self.dispatch(IoEvent::Seek { position_ms: 0 });
        } else {
            self.dispatch(IoEvent::PreviousTrack);
        }
    }

    // The navigation_stack actually only controls the large block to the right of `library` and
    // `playlists`
    pub fn push_navigation_stack(
        &mut self,
        next_route_id: RouteId,
        next_active_block: ActiveBlock,
    ) {
        if !self
            .navigation_stack
            .last()
            .map(|last_route| last_route.id == next_route_id)
            .unwrap_or(false)
        {
            self.navigation_stack.push(Route {
                id: next_route_id,
                active_block: next_active_block,
                hovered_block: next_active_block,
            });
        }
    }

    pub fn pop_navigation_stack(&mut self) -> Option<Route> {
        if self.navigation_stack.len() == 1 {
            None
        } else {
            self.navigation_stack.pop()
        }
    }

    pub fn get_current_route(&self) -> &Route {
        // if for some reason there is no route return the default
        self.navigation_stack.last().unwrap_or(&DEFAULT_ROUTE)
    }

    fn get_current_route_mut(&mut self) -> &mut Route {
        self.navigation_stack.last_mut().unwrap()
    }

    pub fn set_current_route_state(
        &mut self,
        active_block: Option<ActiveBlock>,
        hovered_block: Option<ActiveBlock>,
    ) {
        let current_route = self.get_current_route_mut();
        if let Some(active_block) = active_block {
            current_route.active_block = active_block;
        }
        if let Some(hovered_block) = hovered_block {
            current_route.hovered_block = hovered_block;
        }
    }

    pub fn copy_playing_item_url(&mut self) {
        let (
            Some(clipboard),
            Some(CurrentPlaybackContext {
                item: Some(item), ..
            }),
        ) = (&mut self.clipboard, &self.current_playback_context)
        else {
            return;
        };
        let Some(playable_id) = item.id() else { return };
        handle_error!(self, clipboard.set_text(playable_id.uri()));
    }

    pub fn copy_playing_item_parent_url(&mut self) {
        let (
            Some(clipboard),
            Some(CurrentPlaybackContext {
                item: Some(item), ..
            }),
        ) = (&mut self.clipboard, &self.current_playback_context)
        else {
            return;
        };

        let play_context_id = match item {
            PlayableItem::Track(track) => track.album.id.clone().map(PlayContextId::from),
            PlayableItem::Episode(episode) => Some(PlayContextId::from(episode.show.id.clone())),
        };
        let Some(play_context_id) = play_context_id else {
            return;
        };

        handle_error!(self, clipboard.set_text(play_context_id.uri()));
    }

    pub fn set_saved_tracks_to_table(&mut self, saved_track_page: &Page<SavedTrack>) {
        self.dispatch(IoEvent::SetTracksToTable {
            tracks: saved_track_page
                .items
                .clone()
                .into_iter()
                .map(|item| item.track)
                .collect::<Vec<FullTrack>>(),
        });
    }

    pub fn set_saved_artists_to_table(&mut self, saved_artists_page: &CursorBasedPage<FullArtist>) {
        self.dispatch(IoEvent::SetArtistsToTable {
            artists: saved_artists_page
                .items
                .clone()
                .into_iter()
                .collect::<Vec<FullArtist>>(),
        })
    }

    pub fn get_current_user_saved_artists_next(&mut self) {
        match self
            .library
            .saved_artists
            .get_results(Some(self.library.saved_artists.index + 1))
            .cloned()
        {
            Some(saved_artists) => {
                self.set_saved_artists_to_table(&saved_artists);
                self.library.saved_artists.index += 1
            }
            None => {
                if let Some(saved_artists) = &self.library.saved_artists.clone().get_results(None) {
                    if let Some(last_artist) = saved_artists.items.last() {
                        self.dispatch(IoEvent::GetFollowedArtists {
                            after: Some(last_artist.id.clone()),
                        });
                    }
                }
            }
        }
    }

    pub fn get_current_user_saved_artists_previous(&mut self) {
        if self.library.saved_artists.index > 0 {
            self.library.saved_artists.index -= 1;
        }

        if let Some(saved_artists) = &self.library.saved_artists.get_results(None).cloned() {
            self.set_saved_artists_to_table(saved_artists);
        }
    }

    pub fn get_current_user_saved_tracks_next(&mut self) {
        // Before fetching the next tracks, check if we have already fetched them
        match self
            .library
            .saved_tracks
            .get_results(Some(self.library.saved_tracks.index + 1))
            .cloned()
        {
            Some(saved_tracks) => {
                self.set_saved_tracks_to_table(&saved_tracks);
                self.library.saved_tracks.index += 1
            }
            None => {
                if let Some(saved_tracks) = &self.library.saved_tracks.get_results(None) {
                    let offset = Some(saved_tracks.offset + saved_tracks.limit);
                    self.dispatch(IoEvent::GetCurrentUserSavedTracks { offset });
                }
            }
        }
    }

    pub fn get_current_user_saved_tracks_previous(&mut self) {
        if self.library.saved_tracks.index > 0 {
            self.library.saved_tracks.index -= 1;
        }

        if let Some(saved_tracks) = &self.library.saved_tracks.get_results(None).cloned() {
            self.set_saved_tracks_to_table(saved_tracks);
        }
    }

    pub fn shuffle(&mut self) {
        self.dispatch(IoEvent::ToggleShuffle);
    }

    pub fn get_current_user_saved_albums_next(&mut self) {
        match self
            .library
            .saved_albums
            .get_results(Some(self.library.saved_albums.index + 1))
            .cloned()
        {
            Some(_) => self.library.saved_albums.index += 1,
            None => {
                if let Some(saved_albums) = &self.library.saved_albums.get_results(None) {
                    let offset = Some(saved_albums.offset + saved_albums.limit);
                    self.dispatch(IoEvent::GetCurrentUserSavedAlbums { offset });
                }
            }
        }
    }

    pub fn get_current_user_saved_albums_previous(&mut self) {
        if self.library.saved_albums.index > 0 {
            self.library.saved_albums.index -= 1;
        }
    }

    pub fn current_user_saved_album_delete(&mut self, block: ActiveBlock) {
        match block {
            ActiveBlock::SearchResultBlock => {
                if let Some(albums) = &self.search_results.albums {
                    if let Some(selected_index) = self.search_results.selected_album_index {
                        let selected_album = &albums.items[selected_index];
                        if let Some(album_id) = selected_album.id.clone() {
                            self.dispatch(IoEvent::CurrentUserSavedAlbumDelete { album_id });
                        }
                    }
                }
            }
            ActiveBlock::AlbumList => {
                if let Some(albums) = self.library.saved_albums.get_results(None) {
                    if let Some(selected_album) = albums.items.get(self.album_list_index) {
                        let album_id = selected_album.album.id.clone();
                        self.dispatch(IoEvent::CurrentUserSavedAlbumDelete { album_id });
                    }
                }
            }
            ActiveBlock::ArtistBlock => {
                if let Some(artist) = &self.artist {
                    if let Some(selected_album) =
                        artist.albums.items.get(artist.selected_album_index)
                    {
                        if let Some(album_id) = selected_album.id.clone() {
                            self.dispatch(IoEvent::CurrentUserSavedAlbumDelete { album_id });
                        }
                    }
                }
            }
            _ => (),
        }
    }

    pub fn current_user_saved_album_add(&mut self, block: ActiveBlock) {
        match block {
            ActiveBlock::SearchResultBlock => {
                if let Some(albums) = &self.search_results.albums {
                    if let Some(selected_index) = self.search_results.selected_album_index {
                        let selected_album = &albums.items[selected_index];
                        if let Some(album_id) = selected_album.id.clone() {
                            self.dispatch(IoEvent::CurrentUserSavedAlbumAdd { album_id });
                        }
                    }
                }
            }
            ActiveBlock::ArtistBlock => {
                if let Some(artist) = &self.artist {
                    if let Some(selected_album) =
                        artist.albums.items.get(artist.selected_album_index)
                    {
                        if let Some(album_id) = selected_album.id.clone() {
                            self.dispatch(IoEvent::CurrentUserSavedAlbumAdd { album_id });
                        }
                    }
                }
            }
            _ => (),
        }
    }

    pub fn get_current_user_saved_shows_next(&mut self) {
        match self
            .library
            .saved_shows
            .get_results(Some(self.library.saved_shows.index + 1))
            .cloned()
        {
            Some(_) => self.library.saved_shows.index += 1,
            None => {
                if let Some(saved_shows) = &self.library.saved_shows.get_results(None) {
                    let offset = Some(saved_shows.offset + saved_shows.limit);
                    self.dispatch(IoEvent::GetCurrentUserSavedShows { offset });
                }
            }
        }
    }

    pub fn get_current_user_saved_shows_previous(&mut self) {
        if self.library.saved_shows.index > 0 {
            self.library.saved_shows.index -= 1;
        }
    }

    pub fn get_episode_table_next(&mut self, show_id: ShowId<'_>) {
        match self
            .library
            .show_episodes
            .get_results(Some(self.library.show_episodes.index + 1))
            .cloned()
        {
            Some(_) => self.library.show_episodes.index += 1,
            None => {
                if let Some(show_episodes) = &self.library.show_episodes.get_results(None) {
                    let offset = Some(show_episodes.offset + show_episodes.limit);
                    self.dispatch(IoEvent::GetCurrentShowEpisodes { show_id, offset });
                }
            }
        }
    }

    pub fn get_episode_table_previous(&mut self) {
        if self.library.show_episodes.index > 0 {
            self.library.show_episodes.index -= 1;
        }
    }

    pub fn user_unfollow_artists(&mut self, block: ActiveBlock) {
        match block {
            ActiveBlock::SearchResultBlock => {
                if let Some(artists) = &self.search_results.artists {
                    if let Some(selected_index) = self.search_results.selected_artists_index {
                        let selected_artist: &FullArtist = &artists.items[selected_index];
                        let artist_id = selected_artist.id.clone();
                        self.dispatch(IoEvent::UserUnfollowArtists {
                            artist_ids: vec![artist_id],
                        });
                    }
                }
            }
            ActiveBlock::AlbumList => {
                if let Some(artists) = self.library.saved_artists.get_results(None) {
                    if let Some(selected_artist) = artists.items.get(self.artists_list_index) {
                        let artist_id = selected_artist.id.clone();
                        self.dispatch(IoEvent::UserUnfollowArtists {
                            artist_ids: vec![artist_id],
                        });
                    }
                }
            }
            ActiveBlock::ArtistBlock => {
                if let Some(artist) = &self.artist {
                    let selected_artis =
                        &artist.related_artists[artist.selected_related_artist_index];
                    let artist_id = selected_artis.id.clone();
                    self.dispatch(IoEvent::UserUnfollowArtists {
                        artist_ids: vec![artist_id],
                    });
                }
            }
            _ => (),
        };
    }

    pub fn user_follow_artists(&mut self, block: ActiveBlock) {
        match block {
            ActiveBlock::SearchResultBlock => {
                if let Some(artists) = &self.search_results.artists {
                    if let Some(selected_index) = self.search_results.selected_artists_index {
                        let selected_artist: &FullArtist = &artists.items[selected_index];
                        let artist_id = selected_artist.id.clone();
                        self.dispatch(IoEvent::UserFollowArtists {
                            artist_ids: vec![artist_id],
                        });
                    }
                }
            }
            ActiveBlock::ArtistBlock => {
                if let Some(artist) = &self.artist {
                    let selected_artis =
                        &artist.related_artists[artist.selected_related_artist_index];
                    let artist_id = selected_artis.id.clone();
                    self.dispatch(IoEvent::UserFollowArtists {
                        artist_ids: vec![artist_id],
                    });
                }
            }
            _ => (),
        }
    }

    pub fn user_follow_playlist(&mut self) {
        if let SearchResult {
            playlists: Some(ref playlists),
            selected_playlists_index: Some(selected_index),
            ..
        } = self.search_results
        {
            let selected_playlist: &SimplifiedPlaylist = &playlists.items[selected_index];
            let playlist_id = selected_playlist.id.clone();
            let is_public = selected_playlist.public;
            self.dispatch(IoEvent::UserFollowPlaylist {
                playlist_id,
                is_public,
            });
        }
    }

    pub fn user_unfollow_playlist(&mut self) {
        if let (Some(playlists), Some(selected_index), Some(_)) =
            (&self.playlists, self.selected_playlist_index, &self.user)
        {
            let selected_playlist = &playlists.items[selected_index];
            let playlist_id = selected_playlist.id.clone();
            self.dispatch(IoEvent::UserUnfollowPlaylist { playlist_id })
        }
    }

    pub fn user_unfollow_playlist_search_result(&mut self) {
        if let (Some(playlists), Some(selected_index), Some(_)) = (
            &self.search_results.playlists,
            self.search_results.selected_playlists_index,
            &self.user,
        ) {
            let selected_playlist = &playlists.items[selected_index];
            let playlist_id = selected_playlist.id.clone();
            self.dispatch(IoEvent::UserUnfollowPlaylist { playlist_id })
        }
    }

    pub fn user_follow_show(&mut self, block: ActiveBlock) {
        match block {
            ActiveBlock::SearchResultBlock => {
                if let Some(shows) = &self.search_results.shows {
                    if let Some(selected_index) = self.search_results.selected_shows_index {
                        if let Some(show_id) =
                            shows.items.get(selected_index).map(|item| item.id.clone())
                        {
                            self.dispatch(IoEvent::CurrentUserSavedShowAdd { show_id });
                        }
                    }
                }
            }
            ActiveBlock::EpisodeTable => match self.episode_table_context {
                EpisodeTableContext::Full => {
                    if let Some(selected_episode) = self.selected_show_full.clone() {
                        let show_id = selected_episode.show.id;
                        self.dispatch(IoEvent::CurrentUserSavedShowAdd { show_id });
                    }
                }
                EpisodeTableContext::Simplified => {
                    if let Some(selected_episode) = self.selected_show_simplified.clone() {
                        let show_id = selected_episode.show.id;
                        self.dispatch(IoEvent::CurrentUserSavedShowAdd { show_id });
                    }
                }
            },
            _ => (),
        }
    }

    pub fn user_unfollow_show(&mut self, block: ActiveBlock) {
        match block {
            ActiveBlock::Podcasts => {
                if let Some(shows) = self.library.saved_shows.get_results(None) {
                    if let Some(selected_show) = shows.items.get(self.shows_list_index) {
                        let show_id = selected_show.show.id.clone();
                        self.dispatch(IoEvent::CurrentUserSavedShowDelete { show_id });
                    }
                }
            }
            ActiveBlock::SearchResultBlock => {
                if let Some(shows) = &self.search_results.shows {
                    if let Some(selected_index) = self.search_results.selected_shows_index {
                        let show_id = shows.items[selected_index].id.to_owned();
                        self.dispatch(IoEvent::CurrentUserSavedShowDelete { show_id });
                    }
                }
            }
            ActiveBlock::EpisodeTable => match self.episode_table_context {
                EpisodeTableContext::Full => {
                    if let Some(selected_episode) = self.selected_show_full.clone() {
                        let show_id = selected_episode.show.id;
                        self.dispatch(IoEvent::CurrentUserSavedShowDelete { show_id });
                    }
                }
                EpisodeTableContext::Simplified => {
                    if let Some(selected_episode) = self.selected_show_simplified.clone() {
                        let show_id = selected_episode.show.id;
                        self.dispatch(IoEvent::CurrentUserSavedShowDelete { show_id });
                    }
                }
            },
            _ => (),
        }
    }

    pub fn get_made_for_you(&mut self) {
        // TODO: replace searches when relevant endpoint is added
        const DISCOVER_WEEKLY: &str = "Discover Weekly";
        const RELEASE_RADAR: &str = "Release Radar";
        const ON_REPEAT: &str = "On Repeat";
        const REPEAT_REWIND: &str = "Repeat Rewind";
        const DAILY_DRIVE: &str = "Daily Drive";

        if self.library.made_for_you_playlists.pages.is_empty() {
            // We shouldn't be fetching all the results immediately - only load the data when the
            // user selects the playlist
            self.made_for_you_search_and_add(DISCOVER_WEEKLY);
            self.made_for_you_search_and_add(RELEASE_RADAR);
            self.made_for_you_search_and_add(ON_REPEAT);
            self.made_for_you_search_and_add(REPEAT_REWIND);
            self.made_for_you_search_and_add(DAILY_DRIVE);
        }
    }

    fn made_for_you_search_and_add(&mut self, search_term: &str) {
        let country = self.get_user_country();
        self.dispatch(IoEvent::MadeForYouSearchAndAdd {
            search_term: search_term.to_string(),
            country,
        });
    }

    pub fn get_audio_analysis(&mut self) {
        match &self.current_playback_context {
            Some(CurrentPlaybackContext {
                item: Some(item), ..
            }) => {
                match item {
                    PlayableItem::Episode(_) => {}
                    PlayableItem::Track(track) => match track.id.clone() {
                        Some(track_id) => {
                            if self.get_current_route().id != RouteId::Analysis {
                                self.dispatch(IoEvent::GetTrackAnalysis { track_id });
                            }
                        }
                        None => {}
                    },
                };
                self.push_navigation_stack(RouteId::Analysis, ActiveBlock::Analysis);
            }
            _ => {}
        }
    }

    pub fn repeat(&mut self) {
        if let Some(context) = &self.current_playback_context.clone() {
            self.dispatch(IoEvent::Repeat {
                state: context.repeat_state,
            });
        }
    }

    pub fn get_artist(&mut self, artist_id: ArtistId<'_>, input_artist_name: String) {
        let country = self.get_user_country();
        self.dispatch(IoEvent::GetArtist {
            artist_id,
            input_artist_name,
            country,
        });
    }

    pub fn get_user_country(&self) -> Option<Country> {
        self.user.to_owned().and_then(|user| user.country)
    }

    pub fn calculate_help_menu_offset(&mut self) {
        let old_offset = self.help_menu_offset;

        if self.help_menu_max_lines < self.help_docs_size {
            self.help_menu_offset = self.help_menu_page * self.help_menu_max_lines;
        }
        if self.help_menu_offset > self.help_docs_size {
            self.help_menu_offset = old_offset;
            self.help_menu_page -= 1;
        }
    }
}
