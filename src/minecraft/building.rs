use bevy::prelude::Plugin;
use valence::{
    client::event::{PlayerInteractBlock, StartDigging, StopDestroyBlock},
    prelude::*,
    protocol::types::Hand,
};

pub struct BuildingPlugin;

impl Plugin for BuildingPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_system_to_stage(EventLoop, digging_creative_mode)
            .add_system_to_stage(EventLoop, digging_survival_mode)
            .add_system_to_stage(EventLoop, place_blocks);
    }
}

fn digging_creative_mode(
    clients: Query<&Client>,
    mut instances: Query<&mut Instance>,
    mut events: EventReader<StartDigging>,
) {
    let mut instance = instances.single_mut();

    for event in events.iter() {
        let Ok(client) = clients.get_component::<Client>(event.client) else {
            continue;
        };
        if client.game_mode() == GameMode::Creative {
            instance.set_block(event.position, BlockState::AIR);
        }
    }
}

fn digging_survival_mode(
    clients: Query<&Client>,
    mut instances: Query<&mut Instance>,
    mut events: EventReader<StopDestroyBlock>,
) {
    let mut instance = instances.single_mut();

    for event in events.iter() {
        let Ok(client) = clients.get_component::<Client>(event.client) else {
            continue;
        };
        if client.game_mode() == GameMode::Survival {
            instance.set_block(event.position, BlockState::AIR);
        }
    }
}

fn place_blocks(
    mut clients: Query<(&Client, &mut Inventory)>,
    mut instances: Query<&mut Instance>,
    mut events: EventReader<PlayerInteractBlock>,
) {
    let mut instance = instances.single_mut();

    for event in events.iter() {
        let Ok((client, mut inventory)) = clients.get_mut(event.client) else {
            continue;
        };
        if event.hand != Hand::Main {
            continue;
        }

        // get the held item
        let slot_id = client.held_item_slot();
        let Some(stack) = inventory.slot(slot_id) else {
            // no item in the slot
            continue;
        };

        let Some(block_kind) = stack.item.to_block_kind() else {
            // can't place this item as a block
            continue;
        };

        if client.game_mode() == GameMode::Survival {
            // check if the player has the item in their inventory and remove
            // it.
            let slot = if stack.count() > 1 {
                let mut stack = stack.clone();
                stack.set_count(stack.count() - 1);
                Some(stack)
            } else {
                None
            };
            inventory.replace_slot(slot_id, slot);
        }
        let real_pos = event.position.get_in_direction(event.direction);
        instance.set_block(real_pos, block_kind.to_state());
    }
}
