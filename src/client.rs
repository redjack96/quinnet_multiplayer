use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::Duration;
use bevy::app::{AppExit, ScheduleRunnerPlugin};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy_quinnet::client::{Client, QuinnetClientPlugin};
use bevy_quinnet::shared::ClientId;
use crate::protocol::{ClientMessage, ServerMessage};
use std::collections::HashMap;
use bevy_quinnet::client::certificate::CertificateVerificationMode;
use bevy_quinnet::client::connection::{ConnectionConfiguration, ConnectionEvent};
use rand::distributions::Alphanumeric;
use rand::Rng;

mod protocol;

/// Single User data. Also contains the name of the other users
#[derive(Resource, Debug, Clone, Default)]
struct Users {
    self_id: ClientId,
    names: HashMap<ClientId, String>,
}

/// TerminalReceiver needs to be Sync to be safely a resource. To make this possible,
/// it must be wrapped in a Mutex (so we can avoid to unsafely implement Sync!)
#[derive(Resource, Deref, DerefMut)]
struct TerminalReceiver(Mutex<mpsc::Receiver<String>>);

pub fn on_app_exit(app_exit_events: EventReader<AppExit>, client: Res<Client>) {
    if !app_exit_events.is_empty() {
        if let Ok(_) = client.connection().send_message(ClientMessage::Disconnect {}) {
            // TODO Clean: event to let the async client send his last messages.
            thread::sleep(Duration::from_secs_f32(0.1));
        }
    }
}

fn handle_server_messages(mut users: ResMut<Users>, mut client: ResMut<Client>, mut exit_event: EventWriter<AppExit>) {
    /*
    while let Some(message) = match client
        .connection_mut()
        .receive_message::<ServerMessage>() {
        Ok(message) => message,
        Err(err) => {
            error!("error while receiving message: {err}");
            exit_event.send(AppExit);
            None
        }
    }
    */

    while let Some(message) = client
        .connection_mut()
        .receive_message::<ServerMessage>()
        .or_else(|err| -> Result<Option<ServerMessage>, ()> {
            error!("error while receiving message: {err}");
            exit_event.send(AppExit);
            Ok(None)
        })
        .ok()
        .and_then(|o| o) {
        match message {
            ServerMessage::ClientConnected { client_id, username } => {
                info!("{} joined", username);
                users.names.insert(client_id, username);
            }
            ServerMessage::ClientDisconnected { client_id } => {
                if let Some(username) = users.names.remove(&client_id) {
                    println!("{} left", username);
                } else {
                    warn!("ClientDisconnected for an unknown client_id: {}", client_id)
                }
            }
            ServerMessage::ChatMessage { client_id, message } => {
                if let Some(username) = users.names.get(&client_id) {
                    if client_id != users.self_id {
                        println!("{}: {}", username, message);
                    }
                } else {
                    warn!("Chat message from an unknown client_id: {}", client_id)
                }
            }
            ServerMessage::InitClient {
                client_id,
                usernames,
            } => {
                users.self_id = client_id;
                users.names = usernames;
            }
        }
    }
}

/// specific to the chat message example
fn handle_terminal_messages(
    terminal_messages: ResMut<TerminalReceiver>,
    mut app_exit_events: EventWriter<AppExit>,
    client: Res<Client>,
) {
    while let Ok(message) = terminal_messages.try_lock().unwrap().try_recv() {
        if message == "quit" {
            app_exit_events.send(AppExit);
        } else {
            client
                .connection()
                .try_send_message(ClientMessage::ChatMessage { message });
        }
    }
}

fn start_terminal_listener(mut commands: Commands) {
    let (from_terminal_sender, from_terminal_receiver) = mpsc::channel::<String>();
    // this thread is needed to listen for messages in the terminal. It is specific to this chat example
    thread::spawn(move || loop {
        let mut buffer = String::new();
        std::io::stdin().read_line(&mut buffer).unwrap();
        match from_terminal_sender
            .send(buffer.trim_end().to_string()) {
            Ok(_) => {}
            Err(err) => error!("terminal thread error : {err}"),
        };
    });

    commands.insert_resource(TerminalReceiver(from_terminal_receiver.into()));
}

fn start_connection(mut client: ResMut<Client>, mut exit_event: EventWriter<AppExit>) {
    match client.open_connection(
        ConnectionConfiguration::from_strings("127.0.0.1:6000", "0.0.0.0:0").unwrap(),
        CertificateVerificationMode::SkipVerification,
    ) {
        Ok(_) => {}
        Err(err) => {
            error!("{err}");
            exit_event.send(AppExit)
        }
    };

    // You can already send message(s) even before being connected, they will be buffered. In this example we will wait for a ConnectionEvent.
}

fn handle_client_events(
    mut connection_events: EventReader<ConnectionEvent>,
    client: ResMut<Client>,
) {
    if !connection_events.is_empty() {
        // We are connected
        let username: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();

        println!("--- Joining with name: {}", username);
        println!("--- Type 'quit' to disconnect");

        match client.connection()
            .send_message(ClientMessage::Join { name: username }) {
            Ok(_) => {}
            Err(err) => error!("error while sending join event {err}"),
        };

        connection_events.clear();
    }
}

fn main() {
    println!("client");
    App::new()
        .add_plugins((ScheduleRunnerPlugin::default(), LogPlugin::default(), QuinnetClientPlugin::default()))
        .insert_resource(Users::default())
        .add_systems(Startup, (start_terminal_listener, start_connection))
        .add_systems(Update, (handle_terminal_messages, handle_server_messages, handle_client_events))
        // CoreSet::PostUpdate so that AppExit events generated in the previous stage are available
        .add_systems(PostUpdate, on_app_exit)
        .run();
}