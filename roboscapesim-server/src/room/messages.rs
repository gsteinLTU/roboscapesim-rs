use std::sync::Weak;


use super::*;

#[derive(Debug, Default)]
pub struct MessageHandler {
    room: Weak<RoomData>
}

impl MessageHandler {
    pub fn new(room: Weak<RoomData>) -> Self {
        Self { room }
    }

    fn with_room<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&RoomData) -> R,
    {
        self.room.upgrade().map(|room| f(&*room))
    }

    pub fn get_iotscape_messages(&self) {
        self.with_room(|room| {
            let mut msgs: Vec<(iotscape::Request, Option<<StdSystem<C> as System<C>>::RequestKey>)> = vec![];

            while let Ok(msg) = room.iotscape_rx.lock().unwrap().recv_timeout(Duration::ZERO) {
                if msg.0.function != "heartbeat" {
                    // TODO: figure out which interactions should keep room alive
                    //self.room.last_interaction_time = get_timestamp();
                    msgs.push(msg);
                }
            }
                
            for (msg, key) in msgs {
                trace!("{:?}", msg);

                let response = self.handle_iotscape_message(msg);

                if let Some(key) = key {
                    key.complete(response.0.map_err(|e| e.into()));
                }

                // If an IoTScape event was included in the response, send it to the NetsBlox server
                if let Some(iotscape) = response.1 {
                    room.netsblox_msg_tx.send(iotscape).unwrap();
                }
            }
        });
    }

    pub fn handle_iotscape_message(&self, msg: iotscape::Request) -> (Result<SimpleValue, String>, Option<((String, ServiceType), String, BTreeMap<String, String>)>) {
        self.with_room(|room| {
            let mut response = None;

            let service = room.services.get(&(msg.device.clone(), msg.service.clone().into())).map(|s| s.value().clone());

            if let Some(service) = service {
                response = Some(service.handle_message(&*room, &msg));

                // Update entities if position or rotation changed
                if ServiceType::Entity == msg.service.clone().into() {
                    if msg.function == "setPosition" || msg.function == "setRotation" {
                        if let Some(mut obj) = room.objects.get_mut(msg.device.as_str()) {
                            obj.value_mut().updated = true;
                        }
                    }
                }
            }
            
            response.unwrap_or((Err(format!("Service type {:?} not yet implemented.", &msg.service)), None))
        }).unwrap_or((Err("Room not found".to_string()), None))
    }

    pub fn handle_client_message(&self, msg: ClientMessage, needs_reset: &mut bool, robot_resets: &mut Vec<String>, client_username: &String, client_id: u128) {
        self.with_room(|room| {
            let client = CLIENTS.get(&client_id);

            if let Some(client) = client {
                match msg {
                    ClientMessage::ResetAll => { *needs_reset = true; },
                    ClientMessage::ResetRobot(robot_id) => {
                        if room.is_authorized(*client.key(), &robot_id) {
                            robot_resets.push(robot_id);
                        } else {
                            info!("Client {} not authorized to reset robot {}", client_username, robot_id);
                        }
                    },
                    ClientMessage::ClaimRobot(robot_id) => {
                        // Check if robot is free
                        if room.is_authorized(*client.key(), &robot_id) {
                            // Claim robot
                            if let Some(mut robot) = room.robots.get_mut(&robot_id) {
                                if robot.claimed_by.is_none() {
                                    robot.claimed_by = Some(client_username.clone());

                                    // Send claim message to clients
                                    room.clients_manager.send_to_all_clients(&UpdateMessage::RobotClaimed(robot_id.clone(), client_username.clone()));
                                } else {
                                    info!("Robot {} already claimed by {}, but {} tried to claim it", robot_id, robot.claimed_by.clone().unwrap(), client_username.clone());
                                }
                            }
                        } else {
                            info!("Client {} not authorized to claim robot {}", client_username, robot_id);
                        }
                    },
                    ClientMessage::UnclaimRobot(robot_id) => {
                        // Check if robot is free
                        if room.is_authorized(*client.key(), &robot_id) {
                            // Claim robot
                            if let Some(mut robot) = room.robots.get_mut(&robot_id) {
                                if robot.claimed_by.clone().is_some_and(|claimed_by| &claimed_by == client_username) {
                                    robot.claimed_by = None;

                                    // Send Unclaim message to clients
                                    room.clients_manager.send_to_all_clients(&UpdateMessage::RobotClaimed(robot_id.clone(), "".to_owned()));
                                } else {
                                    info!("Robot {} not claimed by {} who tried to unclaim it", robot_id, client_username);
                                }
                            }
                        } else {
                            info!("Client {} not authorized to unclaim robot {}", client_username, robot_id);
                        }
                    },
                    ClientMessage::EncryptRobot(robot_id) => {
                        if room.is_authorized(*client.key(), &robot_id) {
                            if let Some(mut robot) = room.robots.get_mut(&robot_id) {
                                robot.send_roboscape_message(&[b'P', 0]).unwrap();
                                robot.send_roboscape_message(&[b'P', 1]).unwrap();
                            }
                        } else {
                            info!("Client {} not authorized to encrypt robot {}", client_username, robot_id);
                        }
                    },
                    _ => {
                        warn!("Unhandled client message: {:?}", msg);
                    }
                }
            }
        });
    }
}