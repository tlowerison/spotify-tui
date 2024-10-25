use rspotify_model::enums::types::Type;
use rspotify_model::{idtypes::*, PlayableItem, *};
pub use spotify_tui_util_proc_macros::*;

pub trait ToStatic {
    type Static: 'static;
    fn to_static(self) -> Self::Static;
}

macro_rules! to_static {
    ($($ty:ty),*$(,)?) => {
        $(impl ToStatic for $ty {
            type Static = $ty;
            fn to_static(self) -> Self::Static {
                self
            }
        })*
    };
}

to_static!(
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    f32,
    f64,
    String,
    bool,
    Country,
    FullAlbum,
    FullArtist,
    FullEpisode,
    FullPlaylist,
    FullShow,
    FullTrack,
    Image,
    RepeatState,
    SimplifiedAlbum,
    SimplifiedArtist,
    SimplifiedEpisode,
    SimplifiedPlaylist,
    SimplifiedShow,
    SimplifiedTrack,
);

impl<T: ToStatic> ToStatic for Option<T> {
    type Static = Option<T::Static>;
    fn to_static(self) -> Self::Static {
        self.map(T::to_static)
    }
}

impl<T: ToStatic> ToStatic for Box<T> {
    type Static = Box<T::Static>;
    fn to_static(self) -> Self::Static {
        Box::new((*self).to_static())
    }
}

impl<T: ToStatic> ToStatic for Vec<T> {
    type Static = Vec<T::Static>;
    fn to_static(self) -> Self::Static {
        self.into_iter().map(T::to_static).collect()
    }
}

impl<const N: usize, T: ToStatic> ToStatic for [T; N] {
    type Static = [T::Static; N];
    fn to_static(self) -> Self::Static {
        self.map(T::to_static)
    }
}

impl<'a> ToStatic for ArtistId<'a> {
    type Static = ArtistId<'static>;
    fn to_static(self) -> Self::Static {
        self.into_static()
    }
}

impl<'a> ToStatic for AlbumId<'a> {
    type Static = AlbumId<'static>;
    fn to_static(self) -> Self::Static {
        self.into_static()
    }
}

impl<'a> ToStatic for TrackId<'a> {
    type Static = TrackId<'static>;
    fn to_static(self) -> Self::Static {
        self.into_static()
    }
}

impl<'a> ToStatic for PlaylistId<'a> {
    type Static = PlaylistId<'static>;
    fn to_static(self) -> Self::Static {
        self.into_static()
    }
}

impl<'a> ToStatic for UserId<'a> {
    type Static = UserId<'static>;
    fn to_static(self) -> Self::Static {
        self.into_static()
    }
}

impl<'a> ToStatic for ShowId<'a> {
    type Static = ShowId<'static>;
    fn to_static(self) -> Self::Static {
        self.into_static()
    }
}

impl<'a> ToStatic for EpisodeId<'a> {
    type Static = EpisodeId<'static>;
    fn to_static(self) -> Self::Static {
        self.into_static()
    }
}

impl<'a> ToStatic for PlayContextId<'a> {
    type Static = PlayContextId<'static>;
    fn to_static(self) -> Self::Static {
        match self {
            PlayContextId::Album(id) => PlayContextId::Album(id.into_static()),
            PlayContextId::Artist(id) => PlayContextId::Artist(id.into_static()),
            PlayContextId::Playlist(id) => PlayContextId::Playlist(id.into_static()),
            PlayContextId::Show(id) => PlayContextId::Show(id.into_static()),
        }
    }
}

impl<'a> ToStatic for PlayableId<'a> {
    type Static = PlayableId<'static>;
    fn to_static(self) -> Self::Static {
        match self {
            PlayableId::Episode(id) => PlayableId::Episode(id.into_static()),
            PlayableId::Track(id) => PlayableId::Track(id.into_static()),
        }
    }
}

pub fn fmt_id<T: Id>(id: &T, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
    write!(f, "{}", id.id())
}

pub fn fmt_ids<T: Id>(id: &[T], f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
    f.debug_list().entries(id.iter().map(|id| id.id())).finish()
}

pub fn fmt_opt_ids<T: Id>(
    id: &Option<Vec<T>>,
    f: &mut std::fmt::Formatter,
) -> Result<(), std::fmt::Error> {
    match id {
        Some(id) => f.debug_list().entries(id.iter().map(|id| id.id())).finish(),
        None => f.write_str("None"),
    }
}

pub trait ParseFromUri<'a> {
    fn from_uri(uri: &'a str) -> Result<Self, IdError>
    where
        Self: Sized + 'a;
}

pub trait PlaybleItemExt {
    type Id<'a>
    where
        Self: 'a;
    fn id(&self) -> Self::Id<'_>;
    fn duration(&self) -> &chrono::Duration;
    fn name(&self) -> &str;
}

pub trait PlayableIdExt {
    fn equals(&self, other: &Self) -> bool;
    fn to_string(&self) -> String;
}

impl PlaybleItemExt for PlayableItem {
    type Id<'a> = Option<PlayableId<'a>> where Self: 'a;
    fn id(&self) -> Self::Id<'_> {
        match self {
            PlayableItem::Episode(episode) => Some(PlayableId::Episode(episode.id.clone())),
            PlayableItem::Track(track) => track.id.clone().map(PlayableId::Track),
        }
    }
    fn duration(&self) -> &chrono::Duration {
        match self {
            PlayableItem::Episode(episode) => &episode.duration,
            PlayableItem::Track(track) => &track.duration,
        }
    }
    fn name(&self) -> &str {
        match self {
            PlayableItem::Episode(episode) => &episode.name,
            PlayableItem::Track(track) => &track.name,
        }
    }
}

impl PlayableIdExt for PlayableId<'_> {
    fn equals(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Episode(a), Self::Episode(b)) => a == b,
            (Self::Track(a), Self::Track(b)) => a == b,
            _ => false,
        }
    }
    fn to_string(&self) -> String {
        match self {
            Self::Episode(id) => id.to_string(),
            Self::Track(id) => id.to_string(),
        }
    }
}

macro_rules! id_enum {
    ($name:ident { $($ty:ident),*$(,)? }) => { ::paste::paste! {
        impl<'a> ParseFromUri<'a> for $name<'a> {
            fn from_uri(uri: &'a str) -> Result<Self, IdError>
            where
                Self: Sized + 'a,
            {
                let (ty, id) = parse_uri(&uri)?;
                match ty {
                    $(Type::$ty => Ok([<$ty Id>]::from_id(id)?.into()),)*
                    _ => Err(IdError::InvalidType),
                }
            }
        }
    } };
}

id_enum!(PlayContextId {
    Album,
    Artist,
    Playlist,
    Show,
});
id_enum!(PlayableId { Episode, Track });
