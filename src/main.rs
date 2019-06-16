extern crate serde;
extern crate serde_json;

use serde::{Serialize, Deserialize};

use std::cell::{RefCell};
use std::fs;
use std::io;
use std::io::{Read, Write};
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};


struct ClientSource {
    server_socket: TcpListener,
}

impl ClientSource {
    pub fn new(port: u32) -> ClientSource {
        let sock = TcpListener::bind("127.0.0.1:".to_string() + &port.to_string()).expect("Couldn't create listener socket.");
        sock.set_nonblocking(true).expect("Couldn't set listener socket to non-blocking.");
        ClientSource {
            server_socket: sock,
        }
    }

    pub fn get_opt(&self) -> Option<TcpStream> {
        match self.server_socket.accept() {
            Ok((stream, _addr)) => {
                stream.set_nonblocking(true).expect("Couldn't set client socket to non-blocking.");
                Some(stream)
            },
            Err(_e) => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Exit {
    id: u32,
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Room {
    id: u32,
    name: String,
    description: String,
    exits: Vec<Exit>,
}

struct Area {
    rooms: Vec<Room>,
}

impl Area {
    pub fn new() -> Area {
        Area {
            rooms: load_rooms(),
        }
    }

    pub fn get_room_by_id_mut(&mut self, id: u32) -> Option<&mut Room> {
        self.rooms.iter_mut().find(|el| el.id == id)
    }

    pub fn get_dest_room_id(&mut self, source_room_id: u32, exit_name: &str) -> Option<u32> {
        self.get_room_by_id_mut(source_room_id).and_then(|initial_room| {
            initial_room.exits.iter()
                .find(|e| e.name == exit_name)
                .map(|exit_to_new_room| exit_to_new_room.id)
        })
    }
}

struct Player {
    id: u32,
    attacking_id: Option<u32>,
    stream: TcpStream,
    name: Option<String>,
    room_id: u32,
}

impl Player {
    pub fn read(&mut self) -> Option<String> {
        let mut buf: [u8; 1024] = [0; 1024];
        self.stream.read(&mut buf).ok().map(|size| String::from_utf8_lossy(&buf[0..size]).trim().to_string())
    }

    pub fn write<T: AsRef<str>>(&mut self, msg: T) {
        self.stream.write_all(msg.as_ref().as_bytes());
    }

    pub fn writeln<T: AsRef<str>>(&mut self, msg: T) {
        self.write(msg);
        self.stream.write_all("\r\n".as_bytes());
    }
}

fn load_rooms() -> Vec<Room> {
    let data = fs::read_to_string("rooms.json").expect("Unable to read file");
    serde_json::from_str(&data).expect("Unable to deserialize json")
}

fn main() -> io::Result<()> {
    let client_source = ClientSource::new(50000);
    let mut area = Area::new();
    let mut next_id = 0u32;
    let mut players: HashMap<u32, RefCell<Player>> = HashMap::new();
    loop {
        client_source.get_opt().map(|stream| {
            let mut new_player = Player {
                id: next_id,
                attacking_id: None,
                stream, 
                name: None,
                room_id: 0,
            };
            next_id += 1;
            new_player.write("What is your name? ");
            players.insert(new_player.id, RefCell::new(new_player));
        });

        let imm_players: &HashMap<u32, RefCell<Player>> = &players;
        imm_players.values().for_each(|cell| {
            let mut p = cell.borrow_mut();
            if let Some(command) = p.read() {
                if p.name.is_none() {
                    if !command.is_empty() {
                        p.room_id = 0;
                        p.name = Some(command.clone());
                        p.writeln(format!("Welcome to the game, {}.  Type 'help' for a list of commands. Have fun!", command));
                        imm_players.iter()
                            .filter(|(other_player_id, _other_player)| *other_player_id != &p.id)
                            .for_each(|(_other_player_id, other_player)| {
                                other_player.borrow_mut().writeln(format!("{} entered the game. ", &command));
                            });
                    }
                } else if command == "help" {
                    p.writeln("Commands:
    say <message>  - Says something out loud, e.g. 'say Hello'
    look           - Examines the surroundings, e.g. 'look'
    go <exit>      - Moves through the exit specified, e.g. 'go outside'");

                } else if command.starts_with("say ") {
                    let my_room_id = p.room_id;
                    p.name.iter().for_each(|my_name| {
                        imm_players.iter()
                            .filter(|(id, other_p)| *id != &p.id && other_p.borrow().room_id == my_room_id)
                            .for_each(|(_id, other_p)| {
                                other_p.borrow_mut().writeln(format!("{} says: {}", my_name, &command[4..]));
                            });
                    });
                } else if command == "look" {
                    area.get_room_by_id_mut(p.room_id).map(|room| {
                        p.writeln(&room.description);
                        let players_here = players.iter()
                            .filter(|(id, _other_p)| id != &&p.id)
                            .filter(|(_id, other_p)| {
                                let other_b = other_p.borrow();
                                other_b.room_id == p.room_id && other_b.name.is_some()
                            })
                            .map(|(_id, other_p)| other_p.borrow().name.clone()).flatten().collect::<Vec<String>>().join(", ");
                        p.writeln(format!("Players here: {}", players_here));

                        let exits_here = room.exits.iter().map(|e| e.name.clone()).collect::<Vec<String>>().join(", ");
                        p.writeln(format!("Exits are: {}", exits_here));
                    });
                } else if command.starts_with("go ") {
                    let exit_name = &command[3..];
                    let original_room_id = p.room_id;
                    let dest_room_id_opt: Option<u32> = area.get_dest_room_id(original_room_id, exit_name);

                    dest_room_id_opt.map(|dest_room_id| {
                        p.room_id = dest_room_id;
                        p.name.iter().for_each(|player_name| {
                            imm_players.iter()
                                .filter(|(other_player_id, other_player)| *other_player_id != &p.id && other_player.borrow().room_id == original_room_id)
                                .for_each(|(_other_player_id, other_player)| {
                                    other_player.borrow_mut().writeln(format!("{} left to the {}", player_name, exit_name));
                                });
                            area.get_room_by_id_mut(dest_room_id).map(|new_room| {
                                new_room.exits.iter()
                                    .find(|e| e.id == original_room_id)
                                    .map(|exit_back| {
                                        imm_players.iter()
                                            .filter(|(other_player_id, other_player)| *other_player_id != &p.id && other_player.borrow().room_id == new_room.id)
                                            .for_each(|(_other_player_id, other_player)| {
                                                other_player.borrow_mut().writeln(format!("{} arrived from the {}", player_name, exit_back.name));
                                            });
                                    });
                            });
                        });
                    });
                    
                    // If we changed rooms, write new rooom description
                    // TODO: make this call the 'look' command
                    if original_room_id != p.room_id {
                        area.get_room_by_id_mut(p.room_id).map(|new_room| {
                            p.writeln(&new_room.description);
                        });
                    }
                } else if command.starts_with("attack ") {
                    let target = &command[7..];
                    match p.attacking_id {
                        Some(attacking_id) => p.writeln(format!("You are already attacking {}", imm_players.get(&attacking_id).unwrap().borrow().name.as_ref().or(Some(&String::from("a thing"))).unwrap())),
                        None => {
                            let victim_opt = imm_players.iter()
                                .filter(|(_id, v)| {
                                    let name_opt_ref: &Option<String> = &v.borrow().name;
                                    name_opt_ref.as_ref().map(|n| n.starts_with(target)).unwrap_or(false)
                                }).nth(0).map(|(_id, v)| v);
                            match victim_opt {
                                None => p.writeln(format!("You do not see {} anywhere.", target)),
                                Some(victim_p) => {
                                    p.writeln(format!("You attack {}.", victim_p.borrow().name.as_ref().unwrap_or(&String::from("someone"))));
                                    p.attacking_id = Some(victim_p.borrow().id);
                                },
                            };
                        },
                    };
                    
                } else {
                    p.writeln(format!("Unknown command: {}", command));
                }
            }
        });
    }
    //~ Ok(())
}
