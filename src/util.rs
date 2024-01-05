use rspotify::model::enums::types::Type;
use rspotify::model::{idtypes::*, PlayableItem};

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
}

impl PlaybleItemExt for PlayableItem {
    type Id<'a> = Option<PlayableId<'a>> where Self: 'a;
    fn id(&self) -> Self::Id<'_> {
        match self {
            PlayableItem::Episode(episode) => Some(PlayableId::from(episode.id)),
            PlayableItem::Track(track) => track.id.map(PlayableId::from),
        }
    }
    fn duration(&self) -> &chrono::Duration {
        match self {
            PlayableItem::Episode(episode) => &episode.duration,
            PlayableItem::Track(track) => &track.duration,
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
