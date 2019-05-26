use std::cell::{RefCell};
use std::io;
use std::io::{Read, Write};
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};

struct Client {
    stream: TcpStream,
}

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

    pub fn get_opt(&self) -> Option<Client> {
        match self.server_socket.accept() {
            Ok((stream, _addr)) => {
                stream.set_nonblocking(true).expect("Couldn't set client socket to non-blocking.");
                Some(Client { stream })
            },
            Err(_e) => None,
        }
    }
}

struct Exit {
    room_id: u32,
    direction: String,
}

struct Room {
    id: u32,
    description: String,
    exits: Vec<Exit>,
}

struct Area {
    rooms: HashMap<u32, Room>,
}

impl Area {
    pub fn new() -> Area {
        let mut rooms: HashMap<u32, Room> = HashMap::new();
        rooms.insert(0, Room {
            id: 0,
            description: "You're in a cozy tavern warmed by an open fire.".to_string(),
            exits: vec![
                Exit {
                    room_id: 1,
                    direction: "outside".to_string()
                }
            ],
        });
        rooms.insert(1, Room {
            id: 1,
            description: "You're standing outside a tavern. It's raining.".to_string(),
            exits: vec![
                Exit {
                    room_id: 0,
                    direction: "inside".to_string()
                }
            ],
        });
        Area {
            rooms,
        }
    }
}

struct Player {
    id: u32,
    stream: TcpStream,
    name: Option<String>,
    room_id: Option<u32>,
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

fn main() -> io::Result<()> {
    let client_source = ClientSource::new(50000);
    let area = Area::new();
    let mut next_id = 0u32;
    let mut players: HashMap<u32, RefCell<Player>> = HashMap::new();
    loop {
        client_source.get_opt().map(|Client{ stream }| {
            let mut new_player = Player {
                id: next_id,
                stream, 
                name: None,
                room_id: None,
            };
            next_id += 1;
            new_player.write("What is your name? ");
            players.insert(new_player.id, RefCell::new(new_player));
        });

        let imm_players: &HashMap<u32, RefCell<Player>> = &players;
        let mut new_names = vec![];
        imm_players.values().for_each(|cell| {
            let mut p = cell.borrow_mut();
            if let Some(command) = p.read() {
                if p.name.is_none() {
                    if !command.is_empty() {
                        p.room_id = Some(0);
                        p.name = Some(command.clone());
                        new_names.push(command.clone());
                        p.writeln(format!("Welcome to the game, {}.  Type 'help' for a list of commands. Have fun!", command));
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
                    p.room_id.and_then(|room_id| area.rooms.get(&room_id)).map(|room| {
                        p.writeln(&room.description);
                        let players_here = players.iter()
                            .filter(|(id, _other_p)| id != &&p.id)
                            .filter(|(_id, other_p)| {
                                let other_b = other_p.borrow();
                                other_b.room_id == p.room_id && other_b.name.is_some()
                            })
                            .map(|(_id, other_p)| other_p.borrow().name.clone()).flatten().collect::<Vec<String>>().join(", ");
                        p.writeln(format!("Players here: {}", players_here));

                        let exits_here = room.exits.iter().map(|e| e.direction.clone()).collect::<Vec<String>>().join(", ");
                        p.writeln(format!("Exits are: {}", exits_here));
                    });
                } else if command.starts_with("go ") {
                    let exit_name = &command[3..];
                    let original_room_id = p.room_id;
                    p.room_id.map(|initial_room_id| {
                        area.rooms.get(&initial_room_id).map(|initial_room| {
                            initial_room.exits.iter()
                                .find(|e| e.direction == exit_name)
                                .map(|exit_to_new_room| {
                                    p.room_id = Some(exit_to_new_room.room_id);
                                    p.name.iter().for_each(|player_name| {
                                        imm_players.iter()
                                            .filter(|(other_player_id, other_player)| *other_player_id != &p.id && other_player.borrow().room_id == Some(initial_room_id))
                                            .for_each(|(_other_player_id, other_player)| {
                                                other_player.borrow_mut().writeln(format!("{} left to the {}", player_name, exit_name));
                                            });
                                        area.rooms.get(&exit_to_new_room.room_id).map(|new_room| {
                                            new_room.exits.iter()
                                                .find(|e| e.room_id == initial_room_id)
                                                .map(|exit_back| {
                                                    imm_players.iter()
                                                        .filter(|(other_player_id, other_player)| *other_player_id != &p.id && other_player.borrow().room_id == Some(new_room.id))
                                                        .for_each(|(_other_player_id, other_player)| {
                                                            other_player.borrow_mut().writeln(format!("{} arrived from the {}", player_name, exit_back.direction));
                                                        });
                                                });
                                        });
                                    });
                                });
                        });
                    });
                    if original_room_id != p.room_id {
                        p.room_id.map(|new_room_id| {
                            area.rooms.get(&new_room_id).map(|new_room| {
                                p.writeln(&new_room.description);
                            });
                        });
                    }
                } else {
                    p.writeln(format!("Unknown command: {}", command));
                }
            }
        });
        imm_players.values().for_each(|p| {
            new_names.iter().for_each(|n| {
                p.borrow_mut().writeln(format!("{} entered the game. ", n));
            });
        });
    }
    //~ Ok(())
}
