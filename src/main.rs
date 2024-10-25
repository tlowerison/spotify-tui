mod app;
mod banner;
mod cli;
mod config;
mod event;
mod handlers;
mod network;
mod ui;
mod user_config;

use crate::app::RouteId;
use crate::event::Key;
use anyhow::{anyhow, Result};
use app::{ActiveBlock, App};
use backtrace::Backtrace;
use banner::BANNER;
use chrono::Utc;
use clap::{builder::PossibleValue, Arg, Command};
use clap_complete::Shell;
use config::ClientConfig;
use crossterm::{
    cursor::MoveTo,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    style::Print,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, SetTitle,
    },
    ExecutableCommand,
};
use network::{IoEvent, Network};
use rspotify::{clients::OAuthClient, AuthCodePkceSpotify, Config, Credentials, OAuth, Token};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use std::cmp::{max, min};
use std::io::{self, stdout};
use std::panic::{self, PanicHookInfo};
use std::path::PathBuf;
use std::sync::{mpsc::Receiver, Arc};
use tokio::sync::RwLock;
use tui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use user_config::{UserConfig, UserConfigPaths};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

const SCOPES: [&str; 14] = [
    "playlist-read-collaborative",
    "playlist-read-private",
    "playlist-modify-private",
    "playlist-modify-public",
    "user-follow-read",
    "user-follow-modify",
    "user-library-modify",
    "user-library-read",
    "user-modify-playback-state",
    "user-read-currently-playing",
    "user-read-playback-state",
    "user-read-playback-position",
    "user-read-private",
    "user-read-recently-played",
];

/// get token automatically with local webserver
pub async fn get_token_auto(spotify: &mut AuthCodePkceSpotify) -> Option<Token> {
    let token = match spotify.token.lock().await {
        Ok(token) => token.clone(),
        Err(_) => return None,
    };
    if token.is_some() {
        return token;
    }
    let url = spotify.get_authorize_url(None).unwrap();
    spotify.prompt_for_token(&url).await.ok()?;

    match spotify.token.lock().await {
        Ok(token) => token.clone(),
        Err(_) => None,
    }
}

fn close_application() -> Result<()> {
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

fn panic_hook(info: &PanicHookInfo<'_>) {
    if cfg!(debug_assertions) {
        let location = info.location().unwrap();

        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<Any>",
            },
        };

        let stacktrace: String = format!("{:?}", Backtrace::new()).replace('\n', "\n\r");

        disable_raw_mode().unwrap();
        execute!(
            io::stdout(),
            LeaveAlternateScreen,
            Print(format!(
                "thread '<unnamed>' panicked at '{}', {}\n\r{}",
                msg, location, stacktrace
            )),
            DisableMouseCapture
        )
        .unwrap();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    panic::set_hook(Box::new(|info| {
        panic_hook(info);
    }));

    let mut clap_app = Command::new(env!("CARGO_PKG_NAME"))
    .version(env!("CARGO_PKG_VERSION"))
    .author(env!("CARGO_PKG_AUTHORS"))
    .about(env!("CARGO_PKG_DESCRIPTION"))
    .override_usage("Press `?` while running the app to see keybindings")
    .before_help(BANNER)
    .after_help(
      "Your spotify Client ID and Client Secret are stored in $HOME/.config/spotify-tui/client.yml",
    )
    .arg(
      Arg::new("tick-rate")
        .short('t')
        .long("tick-rate")
        .help("Set the tick rate (milliseconds): the lower the number the higher the FPS.")
        .long_help(
          "Specify the tick rate in milliseconds: the lower the number the \
higher the FPS. It can be nicer to have a lower value when you want to use the audio analysis view \
of the app. Beware that this comes at a CPU cost!",
        )
        .num_args(1),
    )
    .arg(
      Arg::new("config")
        .short('c')
        .long("config")
        .help("Specify configuration file path.")
        .num_args(1),
    )
    .arg(
      Arg::new("completions")
        .long("completions")
        .help("Generates completions for your preferred shell")
        .num_args(1)
        .value_parser([
            PossibleValue::new("bash"),
            PossibleValue::new("zsh"),
            PossibleValue::new("fish"),
            PossibleValue::new("power-shell"),
            PossibleValue::new("elvish"),
        ])
        .value_name("SHELL"),
    )
    // Control spotify from the command line
    .subcommand(cli::playback_subcommand())
    .subcommand(cli::play_subcommand())
    .subcommand(cli::list_subcommand())
    .subcommand(cli::search_subcommand());

    let matches = clap_app.clone().get_matches();

    // Shell completions don't need any spotify work
    if let Some(s) = matches.get_one::<String>("completions") {
        let shell = match &**s {
            "fish" => Shell::Fish,
            "bash" => Shell::Bash,
            "zsh" => Shell::Zsh,
            "power-shell" => Shell::PowerShell,
            "elvish" => Shell::Elvish,
            _ => return Err(anyhow!("no completions avaible for '{}'", s)),
        };
        clap_complete::generate_to(shell, &mut clap_app, "Spotify", "/dev/stdout")
            .expect("Unable to generate completions.");
        return Ok(());
    }

    let mut user_config = UserConfig::new();
    if let Some(config_file_path) = matches.get_one::<String>("config") {
        let config_file_path = PathBuf::from(config_file_path);
        let path = UserConfigPaths { config_file_path };
        user_config.path_to_config.replace(path);
    }
    user_config.load_config()?;

    if let Some(tick_rate) = matches.get_one::<u64>("tick-rate") {
        if *tick_rate >= 1000 {
            panic!("Tick rate must be below 1000");
        } else {
            user_config.behavior.tick_rate_milliseconds = *tick_rate;
        }
    }

    let mut client_config = ClientConfig::new();
    client_config.load_config()?;

    let config_paths = client_config.get_or_build_paths()?;

    // Start authorization with spotify
    let oauth = OAuth {
        redirect_uri: client_config.get_redirect_uri(),
        scopes: SCOPES.into_iter().map(String::from).collect(),
        ..Default::default()
    };
    let mut spotify = AuthCodePkceSpotify::with_config(
        Credentials::new(&client_config.client_id, &client_config.client_secret),
        oauth.clone(),
        Config {
            cache_path: config_paths.token_cache_path,
            token_cached: true,
            token_refreshing: true,
            ..Default::default()
        },
    );

    let Some(token) = get_token_auto(&mut spotify).await else {
        println!("\nSpotify auth failed");
        return Ok(());
    };

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<IoEvent>();

    // Initialise app state
    let app = Arc::new(RwLock::new(App::new(
        tx,
        user_config.clone(),
        token.expires_at.unwrap_or(Utc::now()),
    )));

    // Work with the cli (not really async)
    if let Some(cmd) = matches.subcommand_name() {
        // Save, because we checked if the subcommand is present at runtime
        let m = matches.subcommand_matches(cmd).unwrap();
        let network = Network::new(spotify, client_config, &app);
        println!(
            "{}",
            cli::handle_matches(m, cmd.to_string(), network, user_config).await?
        );
        return Ok(());
    }

    // Launch the UI (async)
    let ui_task = tokio::task::spawn({
        let app = Arc::clone(&app);
        async move { start_ui(user_config, &app).await }
    });

    // Launch the io event handler
    let io_task = tokio::task::spawn({
        let app = Arc::clone(&app);
        async move {
            let mut network = Network::new(spotify, client_config, &app);
            handle_io_events(rx, &mut network).await;
        }
    });

    // Launch the media playback controls and metadata handler
    // let metadata_task = tokio::task::spawn_blocking({
    // });

    let app = Arc::clone(&app);
    configure_metadata_and_controls(&app);

    tokio::select! {
        _ = io_task => {},
        _ = ui_task => {},
    };

    Ok(())
}

async fn handle_io_events<'a>(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<IoEvent<'static>>,
    network: &mut Network<'a>,
) {
    while let Some(io_event) = rx.recv().await {
        network.handle_network_event(io_event).await;
    }
}

async fn start_ui(user_config: UserConfig, app: &Arc<RwLock<App>>) -> Result<()> {
    // Terminal initialization
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    enable_raw_mode()?;

    let mut backend = CrosstermBackend::new(stdout);

    if user_config.behavior.set_window_title {
        backend.execute(SetTitle("Spotify"))?;
    }

    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let mut events = event::Events::new(user_config.behavior.tick_rate_milliseconds);

    // play music on, if not send them to the device selection view

    let mut is_first_render = true;

    loop {
        // Get the size of the screen on each loop to account for resize event
        if let Ok(size) = terminal.backend().size() {
            let mut app = app.write().await;
            // Reset the help menu is the terminal was resized
            if is_first_render || app.size != size {
                app.help_menu_max_lines = 0;
                app.help_menu_offset = 0;
                app.help_menu_page = 0;

                app.size = size;

                // Based on the size of the terminal, adjust the search limit.
                let potential_limit = max((app.size.height as i32) - 13, 0) as u32;
                let max_limit = min(potential_limit, 50);
                let large_search_limit = min((f32::from(size.height) / 1.4) as u32, max_limit);
                let small_search_limit = min((f32::from(size.height) / 2.85) as u32, max_limit / 2);

                app.dispatch(IoEvent::UpdateSearchLimits {
                    large_search_limit,
                    small_search_limit,
                });

                // Based on the size of the terminal, adjust how many lines are
                // displayed in the help menu
                if app.size.height > 8 {
                    app.help_menu_max_lines = (app.size.height as u32) - 8;
                } else {
                    app.help_menu_max_lines = 0;
                }
            }
        };

        let should_reauthenticate = {
            let app = app.read().await;
            let current_route = app.get_current_route();
            terminal.draw(|mut f| match current_route.active_block {
                ActiveBlock::HelpMenu => {
                    ui::draw_help_menu(&mut f, &app);
                }
                ActiveBlock::Error => {
                    ui::draw_error_screen(&mut f, &app);
                }
                ActiveBlock::SelectDevice => {
                    ui::draw_device_list(&mut f, &app);
                }
                ActiveBlock::Analysis => {
                    ui::audio_analysis::draw(&mut f, &app);
                }
                ActiveBlock::BasicView => {
                    ui::draw_basic_view(&mut f, &app);
                }
                _ => {
                    ui::draw_main_layout(&mut f, &app);
                }
            })?;

            if current_route.active_block == ActiveBlock::Input {
                terminal.show_cursor()?;
            } else {
                terminal.hide_cursor()?;
            }

            let cursor_offset = if app.size.height > ui::util::SMALL_TERMINAL_HEIGHT {
                2
            } else {
                1
            };

            // Put the cursor back inside the input box
            terminal.backend_mut().execute(MoveTo(
                cursor_offset + app.input_cursor_position,
                cursor_offset,
            ))?;

            // Handle authentication refresh
            Utc::now() > app.spotify_token_expiry
        };

        if should_reauthenticate {
            app.write().await.dispatch(IoEvent::RefreshAuthentication);
        }

        match events.next().await {
            Some(event::Event::Input(key)) => {
                if key == Key::Ctrl('c') {
                    break;
                }

                let current_active_block = app.read().await.get_current_route().active_block;

                // To avoid swallowing the global key presses `q` and `-` make a special
                // case for the input handler
                if current_active_block == ActiveBlock::Input {
                    handlers::input_handler(key, &mut *app.write().await);
                } else if key == app.read().await.user_config.keys.back {
                    if app.read().await.get_current_route().active_block != ActiveBlock::Input {
                        // Go back through navigation stack when not in search input mode and exit the app if there are no more places to back to

                        let pop_result = match app.write().await.pop_navigation_stack() {
                            Some(ref x) if x.id == RouteId::Search => {
                                app.write().await.pop_navigation_stack()
                            }
                            Some(x) => Some(x),
                            None => None,
                        };
                        if pop_result.is_none() {
                            break; // Exit application
                        }
                    }
                } else {
                    handlers::handle_app(key, &mut *app.write().await);
                }
            }
            Some(event::Event::Tick) => {
                app.write().await.update_on_tick();
            }
            None => {}
        }

        // Delay spotify request until first render, will have the effect of improving
        // startup speed
        if is_first_render {
            let mut app = app.write().await;
            app.dispatch(IoEvent::GetPlaylists);
            app.dispatch(IoEvent::GetUser);
            app.dispatch(IoEvent::GetCurrentPlayback);
            app.help_docs_size = ui::help::get_help_docs(&app.user_config.keys).len() as u32;

            is_first_render = false;
        }
    }

    terminal.show_cursor()?;
    close_application()?;

    Ok(())
}

fn configure_metadata_and_controls(app: &Arc<RwLock<App>>) {
    let event_loop = EventLoop::new().unwrap();
    let mut state = WindowState::new(app);
    let _ = event_loop.run_app(&mut state);
}

struct WindowState {
    app: Arc<RwLock<App>>,
    controls: MediaControls,
    rx: Receiver<MediaControlEvent>,
    // Use an `Option` to allow the window to not be available until the
    // application is properly running.
    window: Option<Window>,
}

impl WindowState {
    fn new(app: &Arc<RwLock<App>>) -> Self {
        #[cfg(not(target_os = "windows"))]
        let hwnd = None;

        #[cfg(target_os = "windows")]
        let hwnd = {
            use raw_window_handle::windows::WindowsHandle;

            let handle: WindowsHandle = unimplemented!();
            Some(handle.hwnd)
        };

        let config = PlatformConfig {
            dbus_name: "spotify-tui",
            display_name: "Spotify",
            hwnd,
        };

        let mut controls = MediaControls::new(config).unwrap();
        let (tx, rx) = std::sync::mpsc::sync_channel(32);

        controls.attach(move |e| tx.send(e).unwrap()).unwrap();

        // Update the media metadata.
        controls
            .set_metadata(MediaMetadata {
                title: Some("Souvlaki Space Station"),
                artist: Some("Slowdive"),
                album: Some("Souvlaki"),
                ..Default::default()
            })
            .unwrap();

        Self {
            app: app.clone(),
            controls,
            rx,
            window: Default::default(),
        }
    }
}

impl ApplicationHandler for WindowState {
    // This is a common indicator that you can create a window.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.window = Some(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => (),
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        _event: DeviceEvent,
    ) {
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);

        let app = self.app.clone();
        let events = self.rx.try_iter();
        let changed = futures::executor::block_on(async move {
            let mut changed = false;
            for event in events {
                match event {
                    MediaControlEvent::Toggle => app.write().await.toggle_playback(),
                    MediaControlEvent::Play => app.write().await.resume_playback(),
                    MediaControlEvent::Pause => app.write().await.pause_playback(),
                    MediaControlEvent::Next => app.write().await.dispatch(IoEvent::NextTrack),
                    MediaControlEvent::Previous => {
                        app.write().await.dispatch(IoEvent::PreviousTrack)
                    }
                    MediaControlEvent::Stop => app.write().await.pause_playback(),
                    _ => (),
                }
                changed = true;
            }
            changed
        });
        std::thread::sleep(std::time::Duration::from_millis(50));

        let playback = futures::executor::block_on({
            let app = self.app.clone();
            async move {
                if app.read().await.is_playing() {
                    MediaPlayback::Playing { progress: None }
                } else {
                    MediaPlayback::Paused { progress: None }
                }
            }
        });

        if changed {
            self.controls.set_playback(playback).unwrap();
        }

        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}
