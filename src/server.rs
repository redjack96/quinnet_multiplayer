/*
1) A system that initialize the end point for the server. FOr now it listen on all address and uses a Self-signed certificate on localhost.
2) A system should handle all client messages. In this example we have Join, Disconnect and Chat.
*/




use std::net::{IpAddr, Ipv4Addr};
use bevy::prelude::*;
use bevy_quinnet::server::certificate::CertificateRetrievalMode;
use bevy_quinnet::server::ConnectionLostEvent;
use bevy_quinnet::server::{Endpoint, QuinnetServerPlugin, Server, ServerConfiguration};
use bevy_quinnet::shared::ClientId;
use std::collections::{HashMap};
use bevy::app::{AppExit, ScheduleRunnerPlugin};
use bevy::input::{InputPlugin};
use bevy::log::LogPlugin;
use bevy_quinnet::shared::channel::ChannelId;
use crate::protocol::{ClientMessage, ServerMessage};

mod protocol;

/// A map that contains all users data
#[derive(Resource, Debug, Clone, Default)]
pub struct Users {
    pub names: HashMap<ClientId, String>,
}


/// Used to start listening from localhost
fn start_listening(mut server: ResMut<Server>, mut exit_event: EventWriter<AppExit>) {
    match server.start_endpoint(
        ServerConfiguration::from_ip(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 6000),
        CertificateRetrievalMode::GenerateSelfSigned { server_hostname: "127.0.0.1".to_string() },
    ) {
        Ok(_) => {}
        Err(err) => {
            error!("{err}");
            exit_event.send(AppExit)
        }
    };
}

/// handles the client messages (Join, Disconnected, ChatMessage)
fn handle_client_messages(mut server: ResMut<Server>, mut users: ResMut<Users>) {
    let endpoint = server.endpoint_mut();
    for client_id in endpoint.clients() {
        while let Some(message) = endpoint.receive_message_from::<ClientMessage>(client_id)
            .transpose() // inverts result with option, so that we can call ok()
            .and_then(|r| return r.ok()) // flatten an option<option<t>> into option<t>
        {
            match message {
                // when the servers receive a Join messages...
                // ... if the client is already joined, shows a warning ...
                ClientMessage::Join { name } => {
                    if users.names.contains_key(&client_id) {
                        warn!("Received a Join from an already connected client: {client_id}")
                    } else {
                        // ... otherwise initialize the client data
                        info!("{} connected", name);
                        users.names.insert(client_id, name.clone());
                        // Sends to the client the existing state...
                        let Ok(_) = endpoint.send_message(
                            client_id,
                            ServerMessage::InitClient {
                                client_id,
                                usernames: users.names.clone(),
                            }) else { return; };
                        // And broadcast the connection event to every connected client (including the last one)
                        let Ok(_) = endpoint.send_group_message(
                            users.names.keys().into_iter(),
                            ServerMessage::ClientConnected {
                                client_id,
                                username: name,
                            }) else { return; };
                    }
                }
                ClientMessage::Disconnect {} => {
                    // We tell the server to disconnect this user
                    let Ok(_) = endpoint.disconnect_client(client_id) else { return; };
                    handle_disconnect(endpoint, &mut users, client_id);
                }
                ClientMessage::ChatMessage { message } => {
                    if let Some(user) = users.names.get(&client_id) {
                        info!("Chat message | {user}: {message}");
                        endpoint.try_send_group_message_on(
                            users.names.keys().into_iter(),
                            ChannelId::UnorderedReliable,
                            ServerMessage::ChatMessage {
                                client_id,
                                message,
                            },
                        );
                    }
                }
            }
        }
    }
}

fn handle_server_events(
    mut connection_lost_events: EventReader<ConnectionLostEvent>,
    mut server: ResMut<Server>,
    mut users: ResMut<Users>,
) {
    // The server signals us about users that lost connection
    for client in connection_lost_events.read() {
        handle_disconnect(server.endpoint_mut(), &mut users, client.id);
    }
}

/// Shared disconnection behaviour, whether the client lost connection or asked to disconnect
fn handle_disconnect(endpoint: &mut Endpoint, users: &mut ResMut<Users>, client_id: ClientId) {
    // Remove this user
    if let Some(username) = users.names.remove(&client_id) {
        // Broadcast its disconnection
        let Ok(_) = endpoint.send_group_message(
            users.names.keys().into_iter(),
            ServerMessage::ClientDisconnected {
                client_id,
            },
        ) else { return; };
        info!("{username} disconnected");
    } else {
        warn!("Received a Disconnect from an unknown or disconnected client: {client_id}")
    }
}

fn main() {
    println!("server");
    App::new()
        .insert_resource(ClearColor(Color::DARK_GRAY))
        .insert_resource(Users::default())
        .add_plugins((ScheduleRunnerPlugin::default(),
                      LogPlugin::default(),
                      QuinnetServerPlugin::default(),
                      InputPlugin)
        )
        .add_systems(Startup, start_listening)
        .add_systems(Update, (handle_client_messages, handle_server_events))
        .run();
}