use super::util::{Flag, Format, FormatType, JumpDirection, Type};
use crate::network::{IoEvent, Network};
use crate::user_config::UserConfig;
use crate::util::ParseFromUri;
use anyhow::{anyhow, Result};
use chrono::Duration;
use rand::{thread_rng, Rng};
use rspotify::clients::BaseClient;
use rspotify::model::idtypes::*;
use rspotify::model::{context::CurrentPlaybackContext, PlayableItem};

pub struct CliApp<'a> {
    pub net: Network<'a>,
    pub config: UserConfig,
}

macro_rules! handle_error {
    ($self:ident, $result:expr $(, $ret:expr)?) => {
        match $result {
            Ok(ok) => ok,
            Err(err) => {
                $self
                    .net
                    .app
                    .lock()
                    .await
                    .handle_error(anyhow!(err.to_string()));
                return $($ret)?;
            }
        }
    };
}

// Non-concurrent functions
// I feel that async in a cli is not working
// I just .await all processes and directly interact
// by calling network.handle_network_event
impl<'a> CliApp<'a> {
    pub fn new(net: Network<'a>, config: UserConfig) -> Self {
        Self { net, config }
    }

    async fn is_a_saved_item(&mut self, playable_id: PlayableId<'_>) -> bool {
        match playable_id {
            // Update the liked_episode_ids_set
            PlayableId::Episode(_) => false,
            // Update the liked_song_ids_set
            PlayableId::Track(track_id) => {
                self.net
                    .handle_network_event(IoEvent::CurrentUserSavedTracksContains {
                        track_ids: vec![track_id.clone()],
                    })
                    .await;
                self.net
                    .app
                    .lock()
                    .await
                    .liked_song_ids_set
                    .contains(&track_id.into_static())
            }
        }
    }

    pub fn format_output(&self, mut format: String, values: Vec<Format>) -> String {
        for val in values {
            format = format.replace(val.get_placeholder(), &val.inner(self.config.clone()));
        }
        // Replace unsupported flags with 'None'
        for p in &["%a", "%b", "%t", "%p", "%h", "%u", "%d", "%v", "%f", "%s"] {
            format = format.replace(p, "None");
        }
        format.trim().to_string()
    }

    // spt playback -t
    pub async fn toggle_playback(&mut self) {
        let context = self.net.app.lock().await.current_playback_context.clone();
        if let Some(c) = context {
            if c.is_playing {
                self.net.handle_network_event(IoEvent::PausePlayback).await;
                return;
            }
        }
        self.net.handle_network_event(IoEvent::ResumePlayback).await;
    }

    // spt pb --share-track (share the current playing song)
    // Basically copy-pasted the 'copy_playing_item_url' function
    pub async fn share_track_or_episode(&mut self) -> Result<String> {
        let app = self.net.app.lock().await;
        let mut url = None;
        if let Some(CurrentPlaybackContext {
            item: Some(item), ..
        }) = &app.current_playback_context
        {
            if let Some(id) = item.id() {
                url = Some(id.url());
            }
        }
        url.ok_or_else(|| anyhow!("failed to generate a shareable url for the current song"))
    }

    // spt pb --share-album (share the current album)
    // Basically copy-pasted the 'copy_playing_item_parent_url' function
    pub async fn share_album_or_show(&mut self) -> Result<String> {
        let app = self.net.app.lock().await;
        let mut url = None;
        if let Some(CurrentPlaybackContext {
            item: Some(item), ..
        }) = &app.current_playback_context
        {
            match item {
                PlayableItem::Track(track) => url = track.album.id.as_ref().map(Id::url),
                PlayableItem::Episode(episode) => url = Some(episode.show.id.url()),
            }
        }
        url.ok_or_else(|| anyhow!("failed to generate a shareable url for the current album/show"))
    }

    // spt ... -d ... (specify device to control)
    pub async fn set_device(&mut self, name: String) -> Result<()> {
        // Change the device if specified by user
        let mut app = self.net.app.lock().await;
        let mut device_index = 0;
        if let Some(dp) = &app.devices {
            for (i, d) in dp.devices.iter().enumerate() {
                if d.name == name {
                    let id = d.id.clone().ok_or_else(|| {
                        anyhow!("failed to use device with name '{name}': no device id")
                    })?;
                    device_index = i;
                    // Save the id of the device
                    self.net
                        .client_config
                        .set_device_id(id)
                        .map_err(|_e| anyhow!("failed to use device with name '{name}'"))?;
                }
            }
        } else {
            // Error out if no device is available
            return Err(anyhow!("no device available"));
        }
        app.selected_device_index = Some(device_index);
        Ok(())
    }

    // spt query ... --limit LIMIT (set max search limit)
    pub async fn update_query_limits(&mut self, max: String) -> Result<()> {
        let num = max
            .parse::<u32>()
            .map_err(|_e| anyhow!("limit must be between 1 and 50"))?;

        // 50 seems to be the maximum limit
        if num > 50 || num == 0 {
            return Err(anyhow!("limit must be between 1 and 50"));
        };

        self.net
            .handle_network_event(IoEvent::UpdateSearchLimits {
                large_search_limit: num,
                small_search_limit: num,
            })
            .await;
        Ok(())
    }

    pub async fn volume(&mut self, vol: String) -> Result<()> {
        let num = vol
            .parse::<u32>()
            .map_err(|_e| anyhow!("volume must be between 0 and 100"))?;

        // Check if it's in range
        if num > 100 {
            return Err(anyhow!("volume must be between 0 and 100"));
        };

        self.net
            .handle_network_event(IoEvent::ChangeVolume { volume: num as u8 })
            .await;
        Ok(())
    }

    // spt playback --next / --previous
    pub async fn jump(&mut self, d: &JumpDirection) {
        match d {
            JumpDirection::Next => self.net.handle_network_event(IoEvent::NextTrack).await,
            JumpDirection::Previous => self.net.handle_network_event(IoEvent::PreviousTrack).await,
        }
    }

    // spt query -l ...
    pub async fn list(&mut self, item: Type, format: &str) -> String {
        match item {
            Type::Device => {
                if let Some(devices) = &self.net.app.lock().await.devices {
                    devices
                        .devices
                        .iter()
                        .map(|d| {
                            self.format_output(
                                format.to_string(),
                                vec![
                                    Some(Format::Device(d.name.clone())),
                                    d.volume_percent.map(Format::Volume),
                                ]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<Format>>(),
                            )
                        })
                        .collect::<Vec<String>>()
                        .join("\n")
                } else {
                    "No devices available".to_string()
                }
            }
            Type::Playlist => {
                self.net.handle_network_event(IoEvent::GetPlaylists).await;
                if let Some(playlists) = &self.net.app.lock().await.playlists {
                    playlists
                        .items
                        .iter()
                        .map(|p| {
                            self.format_output(
                                format.to_string(),
                                Format::from_type(FormatType::Playlist(Box::new(p.clone()))),
                            )
                        })
                        .collect::<Vec<String>>()
                        .join("\n")
                } else {
                    "No playlists found".to_string()
                }
            }
            Type::Liked => {
                self.net
                    .handle_network_event(IoEvent::GetCurrentSavedTracks { offset: None })
                    .await;
                let liked_songs = self
                    .net
                    .app
                    .lock()
                    .await
                    .item_table
                    .items
                    .iter()
                    .filter_map(|playable_item| match playable_item {
                        PlayableItem::Episode(_) => None,
                        PlayableItem::Track(full_track) => Some(self.format_output(
                            format.to_string(),
                            Format::from_type(FormatType::Track(Box::new(full_track.clone()))),
                        )),
                    })
                    .collect::<Vec<String>>();
                // Check if there are any liked songs
                if liked_songs.is_empty() {
                    "No liked songs found".to_string()
                } else {
                    liked_songs.join("\n")
                }
            }
            // Enforced by clap
            _ => unreachable!(),
        }
    }

    // spt playback --transfer DEVICE
    pub async fn transfer_playback(&mut self, device: &str) -> Result<()> {
        // Get the device id by name
        let mut device_id = String::new();
        if let Some(devices) = &self.net.app.lock().await.devices {
            for d in &devices.devices {
                if d.name == device {
                    let id = d.id.clone().ok_or_else(|| {
                        anyhow!("failed to use device with name '{name}': no device id")
                    })?;
                    device_id.push_str(&id);
                    break;
                }
            }
        };

        if device_id.is_empty() {
            Err(anyhow!("no device with name '{}'", device))
        } else {
            self.net
                .handle_network_event(IoEvent::TransferPlaybackToDevice { device_id })
                .await;
            Ok(())
        }
    }

    pub async fn seek(&mut self, seconds_str: String) -> Result<()> {
        let seconds = Duration::seconds(match seconds_str.parse::<i32>() {
            Ok(s) => s.abs() as i64,
            Err(_) => return Err(anyhow!("failed to convert seconds to i32")),
        });

        let (current_pos, duration) = {
            self.net
                .handle_network_event(IoEvent::GetCurrentPlayback)
                .await;
            let app = self.net.app.lock().await;
            if let Some(CurrentPlaybackContext {
                progress: Some(progress),
                item: Some(item),
                ..
            }) = &app.current_playback_context
            {
                let duration: &Duration = match item {
                    PlayableItem::Track(track) => &track.duration,
                    PlayableItem::Episode(episode) => &episode.duration,
                };

                (progress.clone(), duration.clone())
            } else {
                return Err(anyhow!("no context available"));
            }
        };

        // Calculate new positon
        let position_to_seek = if seconds_str.starts_with('+') {
            current_pos + seconds
        } else if seconds_str.starts_with('-') {
            // Jump to the beginning if the position_to_seek would be
            // negative, must be checked before the calculation to avoid
            // an 'underflow'
            if seconds > current_pos {
                Duration::seconds(0)
            } else {
                current_pos - seconds
            }
        } else {
            seconds
        };

        // Check if position_to_seek is greater than duration (next track)
        if position_to_seek > duration {
            self.jump(&JumpDirection::Next).await;
        } else {
            let position_ms = position_to_seek.num_milliseconds() as u32;
            // This seeks to a position in the current song
            self.net
                .handle_network_event(IoEvent::Seek { position_ms })
                .await;
        }

        Ok(())
    }

    // spt playback --like / --dislike / --shuffle / --repeat
    pub async fn mark(&mut self, flag: Flag) -> Result<()> {
        let c = {
            let app = self.net.app.lock().await;
            app.current_playback_context
                .clone()
                .ok_or_else(|| anyhow!("no context available"))?
        };

        match flag {
            Flag::Like(s) => {
                // Get the id of the current song
                let playable_id = match &c.item {
                    Some(item) => item.id().ok_or_else(|| anyhow!("item has no id")),
                    None => Err(anyhow!("no item playing")),
                }?;
                let PlayableId::Track(track_id) = &playable_id else {
                    return Ok(());
                };
                let track_id = track_id.clone();

                match s {
                    // Want to like but is already liked -> do nothing
                    // Want to like and is not liked yet -> like
                    true => {
                        if !self.is_a_saved_item(playable_id).await {
                            self.net
                                .handle_network_event(IoEvent::ToggleSaveTrack { track_id })
                                .await;
                        }
                    }
                    // Want to dislike but is already disliked -> do nothing
                    // Want to dislike and is liked currently -> remove like
                    false => {
                        if self.is_a_saved_item(playable_id).await {
                            self.net
                                .handle_network_event(IoEvent::ToggleSaveTrack { track_id })
                                .await;
                        }
                    }
                }
            }
            Flag::Shuffle => self.net.handle_network_event(IoEvent::ToggleShuffle).await,
            Flag::Repeat => {
                self.net
                    .handle_network_event(IoEvent::Repeat {
                        state: c.repeat_state,
                    })
                    .await;
            }
        }

        Ok(())
    }

    // spt playback -s
    pub async fn get_status(&mut self, format: String) -> Result<String> {
        // Update info on current playback
        self.net
            .handle_network_event(IoEvent::GetCurrentPlayback)
            .await;
        self.net
            .handle_network_event(IoEvent::GetCurrentSavedTracks { offset: None })
            .await;

        let context = self
            .net
            .app
            .lock()
            .await
            .current_playback_context
            .clone()
            .ok_or_else(|| anyhow!("no context available"))?;

        let playing_item = context.item.ok_or_else(|| anyhow!("no track playing"))?;

        let mut hs = match playing_item {
            PlayableItem::Track(track) => {
                let track_id = track
                    .id
                    .clone()
                    .ok_or_else(|| anyhow!("no track id found"))?;
                let mut hs = Format::from_type(FormatType::Track(Box::new(track.clone())));
                if let Some(progress) = &context.progress {
                    hs.push(Format::Position((
                        progress.num_milliseconds() as u32,
                        track.duration.num_milliseconds() as u32,
                    )))
                }
                hs.push(Format::Flags((
                    context.repeat_state,
                    context.shuffle_state,
                    self.is_a_saved_item(PlayableId::Track(track_id)).await,
                )));
                hs
            }
            PlayableItem::Episode(episode) => {
                let mut hs = Format::from_type(FormatType::Episode(Box::new(episode.clone())));
                if let Some(progress) = &context.progress {
                    hs.push(Format::Position((
                        progress.num_milliseconds() as u32,
                        episode.duration.num_milliseconds() as u32,
                    )))
                }
                hs.push(Format::Flags((
                    context.repeat_state,
                    context.shuffle_state,
                    false,
                )));
                hs
            }
        };

        hs.push(Format::Device(context.device.name));
        hs.push(Format::Playing(context.is_playing));
        context
            .device
            .volume_percent
            .map(|vp| hs.push(Format::Volume(vp)));

        Ok(self.format_output(format, hs))
    }

    // spt play -u URI
    pub async fn play_uri(&mut self, uri: String, queue: bool, random: bool) {
        let offset = if random {
            let play_context_id = handle_error!(self, PlayContextId::from_uri(&uri));
            match play_context_id {
                PlayContextId::Album(id) => {
                    let album = handle_error!(self, self.net.spotify.album(id, None).await);
                    let num = album.tracks.total;
                    Some(thread_rng().gen_range(0..num) as usize)
                }
                PlayContextId::Artist(id) => {
                    let tracks =
                        handle_error!(self, self.net.spotify.artist_top_tracks(id, None).await);
                    let num = tracks.len();
                    Some(thread_rng().gen_range(0..num) as usize)
                }
                PlayContextId::Playlist(id) => {
                    let playlist =
                        handle_error!(self, self.net.spotify.playlist(id, None, None).await);
                    let num = playlist.tracks.total;
                    Some(thread_rng().gen_range(0..num) as usize)
                }
                PlayContextId::Show(id) => {
                    let show = handle_error!(self, self.net.spotify.get_a_show(id, None).await);
                    let num = show.episodes.total;
                    Some(thread_rng().gen_range(0..num) as usize)
                }
            }
        } else {
            None
        };

        if uri.contains("spotify:track:") {
            let playable_id = handle_error!(self, PlayableId::from_uri(&uri));

            if queue {
                self.net
                    .handle_network_event(IoEvent::AddItemToQueue { playable_id })
                    .await;
            } else {
                self.net
                    .handle_network_event(IoEvent::StartPlayablesPlayback {
                        playable_ids: vec![playable_id],
                        offset: Some(0),
                    })
                    .await;
            }
        } else {
            let play_context_id = handle_error!(self, PlayContextId::from_uri(&uri));
            self.net
                .handle_network_event(IoEvent::StartContextPlayback {
                    play_context_id,
                    offset: offset.map(|o| o as u32),
                })
                .await;
        }
    }

    // spt play -n NAME ...
    pub async fn play(
        &mut self,
        name: String,
        item: Type,
        queue: bool,
        random: bool,
    ) -> Result<()> {
        self.net
            .handle_network_event(IoEvent::GetSearchResults {
                search_term: name.clone(),
                country: None,
            })
            .await;
        // Get the uri of the first found
        // item + the offset or return an error message
        let uri = {
            let results = &self.net.app.lock().await.search_results;
            match item {
                Type::Album => results
                    .albums
                    .as_ref()
                    .map(|r| r.items.iter().find(|item| item.id.is_some()))
                    .flatten()
                    .ok_or_else(|| anyhow!("no albums with name '{name}'"))?
                    .id
                    .as_ref()
                    .unwrap()
                    .uri(),
                Type::Artist => results
                    .artists
                    .as_ref()
                    .map(|r| r.items.first())
                    .flatten()
                    .ok_or_else(|| anyhow!("no artists with name '{name}'"))?
                    .id
                    .uri(),
                Type::Episode => results
                    .episodes
                    .as_ref()
                    .map(|r| r.items.first())
                    .flatten()
                    .ok_or_else(|| anyhow!("no episodes with name '{name}'"))?
                    .id
                    .as_ref()
                    .uri(),
                Type::Playlist => results
                    .playlists
                    .as_ref()
                    .map(|r| r.items.first())
                    .flatten()
                    .ok_or_else(|| anyhow!("no playlists with name '{name}'"))?
                    .id
                    .uri(),
                Type::Show => results
                    .shows
                    .as_ref()
                    .map(|r| r.items.first())
                    .flatten()
                    .ok_or_else(|| anyhow!("no shows with name '{name}'"))?
                    .id
                    .uri(),
                Type::Track => results
                    .tracks
                    .as_ref()
                    .map(|r| r.items.iter().find(|item| item.id.is_some()))
                    .flatten()
                    .ok_or_else(|| anyhow!("no tracks with name '{name}'"))?
                    .id
                    .as_ref()
                    .unwrap()
                    .uri(),
                _ => unreachable!(),
            }
        };

        // Play or queue the uri
        self.play_uri(uri, queue, random).await;

        Ok(())
    }

    // spt query -s SEARCH ...
    pub async fn query(&mut self, search: String, format: String, item: Type) -> String {
        self.net
            .handle_network_event(IoEvent::GetSearchResults {
                search_term: search.clone(),
                country: None,
            })
            .await;

        let app = self.net.app.lock().await;
        match item {
            Type::Album => {
                if let Some(results) = &app.search_results.albums {
                    results
                        .items
                        .iter()
                        .map(|r| {
                            self.format_output(
                                format.clone(),
                                Format::from_type(FormatType::Album(Box::new(r.clone()))),
                            )
                        })
                        .collect::<Vec<String>>()
                        .join("\n")
                } else {
                    format!("no albums with name '{}'", search)
                }
            }
            Type::Artist => {
                if let Some(results) = &app.search_results.artists {
                    results
                        .items
                        .iter()
                        .map(|r| {
                            self.format_output(
                                format.clone(),
                                Format::from_type(FormatType::Artist(Box::new(r.clone()))),
                            )
                        })
                        .collect::<Vec<String>>()
                        .join("\n")
                } else {
                    format!("no artists with name '{}'", search)
                }
            }
            Type::Episode => {
                if let Some(results) = &app.search_results.episodes {
                    results
                        .items
                        .iter()
                        .map(|r| {
                            self.format_output(
                                format.clone(),
                                Format::from_type(FormatType::Episode(Box::new(r.clone()))),
                            )
                        })
                        .collect::<Vec<String>>()
                        .join("\n")
                } else {
                    format!("no episodes with name '{}'", search)
                }
            }
            Type::Playlist => {
                if let Some(results) = &app.search_results.playlists {
                    results
                        .items
                        .iter()
                        .map(|r| {
                            self.format_output(
                                format.clone(),
                                Format::from_type(FormatType::Playlist(Box::new(r.clone()))),
                            )
                        })
                        .collect::<Vec<String>>()
                        .join("\n")
                } else {
                    format!("no playlists with name '{}'", search)
                }
            }
            Type::Show => {
                if let Some(results) = &app.search_results.shows {
                    results
                        .items
                        .iter()
                        .map(|r| {
                            self.format_output(
                                format.clone(),
                                Format::from_type(FormatType::Show(Box::new(r.clone()))),
                            )
                        })
                        .collect::<Vec<String>>()
                        .join("\n")
                } else {
                    format!("no shows with name '{}'", search)
                }
            }
            Type::Track => {
                if let Some(results) = &app.search_results.tracks {
                    results
                        .items
                        .iter()
                        .map(|r| {
                            self.format_output(
                                format.clone(),
                                Format::from_type(FormatType::Track(Box::new(r.clone()))),
                            )
                        })
                        .collect::<Vec<String>>()
                        .join("\n")
                } else {
                    format!("no tracks with name '{}'", search)
                }
            }
            // Enforced by clap
            _ => unreachable!(),
        }
    }
}
