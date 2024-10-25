use crate::event::Key;
use anyhow::Error;
use crossterm::event::{self, EventStream};
use futures_util::{FutureExt, StreamExt};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy)]
/// Configuration for event handling.
pub struct EventConfig {
    /// The key that is used to exit the application.
    pub exit_key: Key,
    /// The tick rate at which the application will sent an tick event.
    pub tick_rate: Duration,
}

impl Default for EventConfig {
    fn default() -> EventConfig {
        EventConfig {
            exit_key: Key::Ctrl('c'),
            tick_rate: Duration::from_millis(250),
        }
    }
}

/// An occurred event.
pub enum Event<I> {
    /// An input event occurred.
    Input(I),
    /// An tick event occurred.
    Tick,
}

/// A small event handler that wrap crossterm input and tick event. Each event
/// type is handled in its own thread and returned to a common `Receiver`
pub struct Events {
    rx: mpsc::UnboundedReceiver<Event<Key>>,
    // Need to be kept around to prevent disposing the sender side.
    #[allow(dead_code)]
    tx: mpsc::UnboundedSender<Event<Key>>,
}

impl Events {
    /// Constructs an new instance of `Events` with the default config.
    pub fn new(tick_rate: u64) -> Events {
        Events::with_config(EventConfig {
            tick_rate: Duration::from_millis(tick_rate),
            ..Default::default()
        })
    }

    /// Constructs an new instance of `Events` from given config.
    pub fn with_config(config: EventConfig) -> Events {
        let (tx, rx) = mpsc::unbounded_channel();

        let event_tx = tx.clone();
        tokio::spawn(async move {
            let mut reader = EventStream::new();
            loop {
                let result = tokio::select! {
                    _ = tokio::time::sleep(config.tick_rate).fuse() => event_tx.send(Event::Tick).map_err(Error::msg),
                    event = reader.next().fuse() => {
                        match event {
                            Some(Ok(event::Event::Key(key))) => event_tx.send(Event::Input(Key::from(key))).map_err(Error::msg),
                            Some(res) => res.map(|_| ()).map_err(Error::msg),
                            None => break,
                        }
                    }
                };
                if let Err(err) = result {
                    eprintln!("Error: {err}");
                }
            }
        });

        Events { rx, tx }
    }

    /// Attempts to read an event.
    /// This function will block the current thread.
    pub async fn next(&mut self) -> Option<Event<Key>> {
        self.rx.recv().await
    }
}
