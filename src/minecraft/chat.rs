use bevy::{
    prelude::{Plugin, Query},
    window::Window,
};
use valence::{
    client::event::{ChatMessage, CommandExecution},
    prelude::*,
    server::EventLoopSchedule,
};

use super::world_gen::Instances;
use crate::SPAWN_POS;

#[allow(dead_code)]
pub enum Message {
    ChatMessage(ChatMessage),
    ServerMessage(Text),
}

#[derive(Resource, Default)]
pub struct ChatMessages(pub Vec<Message>);

impl ChatMessages {
    pub fn add(&mut self, msg: Message) { self.0.push(msg) }
}

pub struct ChatPlugin;

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.insert_resource(ChatMessages::default())
            .add_system(chat_message.in_schedule(EventLoopSchedule))
            .add_system(interpret_command.in_schedule(EventLoopSchedule));
    }
}

fn chat_message(
    mut clients: Query<&mut Client>,
    mut events: EventReader<ChatMessage>,
    mut messages: ResMut<ChatMessages>,
) {
    for event in events.iter() {
        let Ok(sender) = clients.get_component::<Client>(event.client) else {
            warn!("Unable to find client for message: {:?}", event);
            continue;
        };

        let message = event.message.to_string();

        let username = Text::from(sender.username().to_string());

        info!(target: "minecraft::chat", "{username}: {}", message);

        let formatted = username + ": ".into_text() + message.color(Color::WHITE);

        clients.par_iter_mut().for_each_mut(|mut client| {
            client.send_message(formatted.clone());
        });

        messages.add(Message::ChatMessage(event.clone()));
    }
}

fn interpret_command(
    mut clients: Query<&mut Client>,
    mut events: EventReader<CommandExecution>,
    instances_list: Res<Instances>,
) {
    for event in events.iter() {
        let Ok(mut client) = clients.get_component_mut::<Client>(event.client) else {
            continue;
        };

        let mut args = event.command.split_whitespace();
        let command = args.next().unwrap_or_default();

        if command == "gamemode" {
            if client.op_level() < 2 {
                // not enough permissions to use gamemode command
                client.send_message("Not enough permissions to use gamemode command.".italic());
                continue;
            }

            let mode = args.next().unwrap_or_default();
            let mode = match mode {
                "adventure" => GameMode::Adventure,
                "creative" => GameMode::Creative,
                "survival" => GameMode::Survival,
                "spectator" => GameMode::Spectator,
                _ => {
                    client.send_message("Invalid gamemode.".italic());
                    continue;
                }
            };

            client.set_game_mode(mode);
            client.send_message(format!("Set gamemode to {mode:?}.").italic());
        } else if command == "terrain" {
            client.set_instance(instances_list.terrain);
            let spawn = *SPAWN_POS.lock().unwrap();
            client.set_position([spawn.x, spawn.y, spawn.z]);
        } else if command == "wait" {
            client.set_instance(instances_list.wait);
            client.set_position([0., 203., 0.]);
        } else {
            client.send_message("Invalid command.".italic());
        }
    }
}

#[cfg(feature = "gui")]
pub fn gui_chat_window(
    mut egui_context: bevy_egui::EguiContexts,
    mut messages: ResMut<ChatMessages>,
    mut clients: Query<(&mut Client, Option<&mut McEntity>)>,
    mut send_message_content: Local<String>,
    mut display_messages: Local<Vec<(String, String)>>,
) {
    use bevy_egui::egui;

    messages.0.iter().for_each(|m| match m {
        Message::ChatMessage(m) => {
            let Ok(sender) = clients.get_component::<Client>(m.client) else {return;};

            let username = sender.username().to_string();
            display_messages.push((username, m.message.to_string()));
        }
        Message::ServerMessage(msg) => {
            display_messages.push(("Server".to_string(), msg.to_string()));
        }
    });

    messages.0.clear();

    egui::Window::new("Chat")
        .resizable(true)
        .collapsible(true)
        .show(&egui_context.ctx_mut(), |ui| {
            ui.horizontal(|row| {
                row.label("Total amount of messages:");
                row.label(format!("{}", messages.0.len()));
            });

            ui.horizontal(|row| {
                row.label("Send Message:");

                let _response = row.add(egui::TextEdit::singleline(&mut *send_message_content));

                let button = row.button("Send");

                if row.input(|i| i.key_pressed(egui::Key::Enter)) || button.clicked() {
                    let text = send_message_content.clone();

                    for (mut c, _) in clients.iter_mut() {
                        c.send_message("[Server]: ".color(Color::GRAY) + text.clone());
                        messages.add(Message::ServerMessage(text.clone().into()));
                    }

                    *send_message_content = String::new();
                }
            });

            ui.group(|group| {
                egui::ScrollArea::vertical().show(group, |g| {
                    display_messages.iter().for_each(|(from, msg)| {
                        g.horizontal(|row| {
                            row.label(format!("[{from}]"));
                            row.label(msg);
                        });
                    });
                });
            });
        });
}
