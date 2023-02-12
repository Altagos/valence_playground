use bevy::prelude::{Plugin, Query};
use bevy_inspector_egui::{bevy_egui, egui};
use valence::{ prelude::*, server::EventLoop, client::event::{ChatMessage, ChatCommand}};

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
            .add_system_to_stage(EventLoop, chat_message)
            .add_system_to_stage(EventLoop, interpret_command);
    }
}

fn chat_message(
    mut clients: Query<(&mut Client, Option<&mut McEntity>)>,
    mut events: EventReader<ChatMessage>,
    mut messages: ResMut<ChatMessages>,
) {
    for event in events.iter() {
        let Ok(sender) = clients.get_component::<Client>(event.client) else {
            continue;
        };

        let username = match sender.player().get_custom_name() {
            Some(name) => name.clone(),
            None => Text::from(sender.username().to_string()),
        };

        for (mut c, _) in clients.iter_mut() {
            c.send_message(format!("{username}: {}", event.message));
        }

        messages.add(Message::ChatMessage(event.clone()));
    }
}

fn interpret_command(mut clients: Query<&mut Client>, mut events: EventReader<ChatCommand>) {
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
        } else {
            client.send_message("Invalid command.".italic());
        }
    }
}

#[allow(dead_code)]
pub fn gui_chat_window(
    mut egui_context: ResMut<bevy_egui::EguiContext>,
    mut messages: ResMut<ChatMessages>,
    mut send_message_content: Local<String>,
    mut clients: Query<(&mut Client, Option<&mut McEntity>)>,
) {
    let egui_context = egui_context
        .ctx_mut()
        .clone();

    egui::Window::new("Chat")
        .resizable(true)
        .collapsible(true)
        // .hscroll(true)
        // .vscroll(true)
        .show(&egui_context, |ui| {
            ui.horizontal(|row| {
                row.label("Total amount of messages:");
                row.label(format!("{}", messages.0.len()));
            });

            ui.horizontal(|row| {
                row.label("Send Message:");

                let _response = row.add(egui::TextEdit::singleline(&mut *send_message_content));

                let button = row.button("Send");

                if row.input().key_pressed(egui::Key::Enter) || button.clicked() {
                    let text = "[Server]: ".color(Color::GRAY) + send_message_content.clone();

                    for (mut c, _) in clients.iter_mut() {
                        c.send_message(text.clone());
                        messages.add(Message::ServerMessage(text.clone()));
                    }

                    *send_message_content = String::new();
                }
            });

            ui.group(|group| {
                egui::ScrollArea::vertical().show(group, |g| {
                    messages.0.iter().for_each(|item| {
                        g.horizontal(|row| {
                            match item {
                                Message::ChatMessage(item) => {
                                    let Ok(sender) = clients.get_component::<Client>(item.client) else {return;};

                                    let username = match sender.player().get_custom_name() {
                                        Some(name) => name.clone(),
                                        None => Text::from(sender.username().to_string()),
                                    };

                                    row.label(format!("[{username}]"));
                                    row.label(item.message.to_string());
                                },
                                Message::ServerMessage(message) => {row.label(message.to_string());},
                            };
                        
                            
                        });
                    });
                });
            });
        });
}