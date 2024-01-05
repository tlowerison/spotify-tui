use super::{
    util::{Flag, JumpDirection, Type},
    CliApp,
};
use crate::network::{IoEvent, Network};
use crate::user_config::UserConfig;
use anyhow::{anyhow, Result};
use clap::ArgMatches;

// Handle the different subcommands
pub async fn handle_matches(
    matches: &ArgMatches,
    cmd: String,
    net: Network<'_>,
    config: UserConfig,
) -> Result<String> {
    let mut cli = CliApp::new(net, config);

    cli.net.handle_network_event(IoEvent::GetDevices).await;
    cli.net
        .handle_network_event(IoEvent::GetCurrentPlayback)
        .await;

    let devices_list = match &cli.net.app.lock().await.devices {
        Some(p) => p
            .devices
            .iter()
            .filter_map(|d| d.id.clone())
            .collect::<Vec<String>>(),
        None => Vec::new(),
    };

    // If the device_id is not specified, select the first available device
    let device_id = cli.net.client_config.device_id.clone();
    if device_id.is_none() || !devices_list.contains(&device_id.unwrap()) {
        // Select the first device available
        if let Some(d) = devices_list.get(0) {
            cli.net.client_config.set_device_id(d.clone())?;
        }
    }

    if let Ok(Some(d)) = matches.try_get_one::<String>("device") {
        cli.set_device(d.to_string()).await?;
    }

    // Evalute the subcommand
    let output = match cmd.as_str() {
        "playback" => {
            let format = matches.try_get_one::<String>("format")?.unwrap();

            // Commands that are 'single'
            if matches.get_raw_occurrences("share-track").is_some() {
                return cli.share_track_or_episode().await;
            } else if matches.get_raw_occurrences("share-album").is_some() {
                return cli.share_album_or_show().await;
            }

            // Run the action, and print out the status
            // No 'else if's because multiple different commands are possible
            if matches.get_raw_occurrences("toggle").is_some() {
                cli.toggle_playback().await;
            }
            if let Ok(Some(d)) = matches.try_get_one::<String>("transfer") {
                cli.transfer_playback(d).await?;
            }
            // Multiple flags are possible
            if matches.get_raw_occurrences("flags").is_some() {
                let flags = Flag::from_matches(matches);
                for f in flags {
                    cli.mark(f).await?;
                }
            }
            if matches.get_raw_occurrences("jumps").is_some() {
                let (direction, amount) = JumpDirection::from_matches(matches);
                for _ in 0..amount {
                    cli.jump(&direction).await;
                }
            }
            if let Ok(Some(vol)) = matches.try_get_one::<String>("volume") {
                cli.volume(vol.to_string()).await?;
            }
            if let Ok(Some(secs)) = matches.try_get_one::<String>("seek") {
                cli.seek(secs.to_string()).await?;
            }

            // Print out the status if no errors were found
            cli.get_status(format.to_string()).await
        }
        "play" => {
            let queue = matches.get_raw_occurrences("queue").is_some();
            let random = matches.get_raw_occurrences("random").is_some();
            let format = matches.try_get_one::<String>("format").unwrap().unwrap();

            if let Ok(Some(uri)) = matches.try_get_one::<String>("uri") {
                cli.play_uri(uri.to_string(), queue, random).await;
            } else if let Ok(Some(name)) = matches.try_get_one::<String>("name") {
                let category = Type::play_from_matches(matches);
                cli.play(name.to_string(), category, queue, random).await?;
            }

            cli.get_status(format.to_string()).await
        }
        "list" => {
            let format = matches
                .try_get_one::<String>("format")
                .unwrap()
                .unwrap()
                .to_string();

            // Update the limits for the list and search functions
            // I think the small and big search limits are very confusing
            // so I just set them both to max, is this okay?
            if let Ok(Some(max)) = matches.try_get_one::<String>("limit") {
                cli.update_query_limits(max.to_string()).await?;
            }

            let category = Type::list_from_matches(matches);
            Ok(cli.list(category, &format).await)
        }
        "search" => {
            let format = matches
                .try_get_one::<String>("format")
                .unwrap()
                .unwrap()
                .to_string();

            // Update the limits for the list and search functions
            // I think the small and big search limits are very confusing
            // so I just set them both to max, is this okay?
            if let Ok(Some(max)) = matches.try_get_one::<String>("limit") {
                cli.update_query_limits(max.to_string()).await?;
            }

            let category = Type::search_from_matches(matches);
            Ok(cli
                .query(
                    matches
                        .try_get_one::<String>("search")
                        .unwrap()
                        .unwrap()
                        .to_string(),
                    format,
                    category,
                )
                .await)
        }
        // Clap enforces that one of the things above is specified
        _ => unreachable!(),
    };

    // Check if there was an error
    let api_error = cli.net.app.lock().await.api_error.clone();
    if api_error.is_empty() {
        output
    } else {
        Err(anyhow!("{}", api_error))
    }
}
