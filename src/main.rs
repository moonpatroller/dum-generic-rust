use std::io;
use std::io::{Read, Write};
use std::collections::HashMap;
use std::collections::hash_map::ValuesMut;
use std::net::{TcpListener, TcpStream};

struct Client {
    id: u32,
    stream: TcpStream,
    buffer: String,
}

struct MudServer {
    clients: HashMap<u32, Client>,
    nextid: u32,
    new_connections: Vec<u32>,
    server_socket: TcpListener,
}

impl MudServer {
    pub fn new(port: u32) -> MudServer {
        let sock = TcpListener::bind("127.0.0.1:".to_string() + &port.to_string()).expect("Couldn't create listener socket.");
        sock.set_nonblocking(true).expect("Couldn't set listener socket to non-blocking.");
        MudServer {
            clients: HashMap::new(),
            nextid: 0,
            new_connections: vec![],
            server_socket: sock,
        }
    }

    pub fn get_new_clients(&mut self) -> Vec<&Client> {
        let clients = &self.clients;
        let new_clients = self.new_connections.iter().map(|id| &(clients[id])).collect::<Vec<_>>();
        self.new_connections.clear();
        new_clients
    }

    pub fn get_clients(&mut self) -> ValuesMut<u32, Client> {
        self.clients.values_mut()
    }

    pub fn update(&mut self) {
        self.check_for_new_connections();
        self.check_for_messages();
    }

    fn check_for_new_connections(&mut self) {
        match self.server_socket.accept() {
            Ok((socket, _addr)) => {
                socket.set_nonblocking(true).expect("Couldn't set client socket to non-blocking.");
                self.nextid += 1;
                let client = Client{
                    id: self.nextid,
                    stream: socket,
                    buffer: String::from(""),
                };
                self.clients.insert(self.nextid, client);
                self.new_connections.push(self.nextid);
            },
            Err(_e) => (),
        }
    }

    fn check_for_messages(&mut self) {
        for (_id, client) in self.clients.iter_mut() {
            let mut buf: [u8; 1024] = [0; 1024];
            match client.stream.read(&mut buf) {
                Ok(size) => {
                    let message = String::from_utf8_lossy(&buf[0..size]);
                    client.buffer += &message;
                },
                
                Err(_) => {},
            }
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
    name: Option<String>,
    room_id: Option<u32>,
}

fn main() -> io::Result<()> {
    let mut m = MudServer::new(50000);
    let a = Area::new();
    let mut players: HashMap<u32, Player> = HashMap::new();
    loop {
        m.update();
        m.get_new_clients().iter().for_each(|c| {
            players.insert(c.id, Player {
                id: c.id,
                name: None,
                room_id: None,
            });
            let mut stream = &c.stream;
            stream.write_all("What is your name?".as_bytes());
        });
        let mut new_names = vec![];
        m.get_clients().for_each(|c| {
            let trimmed_command: String = c.buffer.clone().trim().to_string();
            if !c.buffer.is_empty() {
                let p = players.get_mut(&c.id).expect(&("Bad player id: ".to_string() + &c.id.to_string()));
                if p.name.is_none() {
                    p.room_id = Some(0);
                    p.name = Some(trimmed_command.clone());
                    new_names.push(trimmed_command.clone());
                    let buffer = &mut c.buffer;
                    c.stream.write_all(format!("Welcome to the game, {}.  Type 'help' for a list of commands. Have fun!", trimmed_command).as_bytes());
                    buffer.clear();
                }
            } else if trimmed_command == "help" {
                c.stream.write_all("Commands:
    say <message>  - Says something out loud, e.g. 'say Hello'
    look           - Examines the surroundings, e.g. 'look'
    go <exit>      - Moves through the exit specified, e.g. 'go outside'
".as_bytes());

            } else if trimmed_command == "say" {
                let my_player = players.get(&c.id).unwrap();
                let my_room_id = my_player.room_id;
                players.values()
                    .filter(|p| p.room_id == my_room_id)
                    .for_each(|p| {
                        let client = m.clients.get(&p.id).unwrap();
                        client.stream.write_all(format!("{} says: {}", my_player.name.unwrap(), trimmed_command).as_bytes());
                    });
            }
        });
        m.get_clients().for_each(|c| {
            new_names.iter().for_each(|n| {
                c.stream.write_all(format!("{} entered the game", n).as_bytes());
            });
        });
    }
    //~ Ok(())
}


