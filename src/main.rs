mod vector;

use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use vector::Vector;

struct Bullet {
    pub id: i32,
    pub position: Vector,
    pub velocity: Vector,
}

impl Bullet {
    fn update(&mut self, dt: f32) {
        self.position += self.velocity * dt;
    }
}

struct Player {
    id: i32,
    position: Vector,
    velocity: Vector,
    orientation: f32,
    design: u8,
    propulsor: [bool; 4],
    can_shoot: i32,
    hits: i32,
}

impl Player {
    fn update(&mut self, dt: f32) {
        let mut acc: Vector = [0.0, 0.0].into();

        // WASD order.
        if self.propulsor[0] {
            acc.y += 1.0;
        }
        if self.propulsor[1] {
            acc.x -= 1.0;
        }
        if self.propulsor[2] {
            acc.y -= 1.0;
        }
        if self.propulsor[3] {
            acc.x += 1.0;
        }

        // Update response.
        self.position += self.velocity * 0.5 * dt + acc * dt * dt;
        self.velocity += acc * dt;
    }

    fn shoot(&mut self) {
        self.velocity.x -= 0.1 * f32::cos(self.orientation);
        self.velocity.y -= 0.1 * f32::sin(self.orientation);
    }

    fn receive_hit(&mut self, bullet: &Bullet) {
        self.velocity += 0.1 * bullet.velocity;
        self.hits += 1;
    }
}

struct ClientData {
    id: i32,
    orientation: f32,
    propulsor: [bool; 4],
}

struct Game {
    // Player data.
    players: Vec<Player>,
    streams: Vec<TcpStream>,
}

impl Game {
    fn new() -> Self {
        Game {
            players: Vec::new(),
            streams: Vec::new(),
        }
    }

    fn new_player(&mut self, new_stream: TcpStream) {
        let amount = self.players.len();
        self.players.push(Player {
            id: amount as i32,
            position: Vector { x: 0.0, y: 0.0 },
            velocity: Vector { x: 0.0, y: 0.0 },
            orientation: 0.0,
            design: 0,
            propulsor: [false, false, false, false],
            can_shoot: 0,
            hits: 0,
        });

        self.streams.push(new_stream);
    }

    // This iterates the game with respect to time.
    fn iterate_game(&mut self) {}

    // Protocol zero.
    // [  32 bits  |   8 bits    |     64 bits     | message ]
    // [message id | protocol id | size of message | message ]

    // Client Message:
    // [ 32 bits   |   32 bits   |  8 bits   ]
    // [ player id | orientation | propulsor ]
    fn read_client_binary_message(&self, message: &[u8]) -> ClientData {
        let id = i32::from_be_bytes([message[0], message[1], message[2], message[3]]);
        let orientation = f32::from_be_bytes([message[4], message[5], message[6], message[7]]);
        let propulsor = message[8];

        let pw = propulsor & 0b0001 != 0;
        let pa = propulsor & 0b0010 != 0;
        let ps = propulsor & 0b0100 != 0;
        let pd = propulsor & 0b1000 != 0;

        ClientData {
            id,
            orientation,
            propulsor: [pw, pa, ps, pd],
        }
    }

    // Server Message:
    // [ 32 bits   |     2 * 3 * 32 bits   |   32 bits   | 8 bits |  8 bits   | 32 bits ]
    // [ player id | position and velocity | orientation | design | propulsor | hits ]
    fn send_server_binary_message(&self) -> Vec<u8> {
        let mut array: Vec<u8> = Vec::new();

        for player in self.players.iter() {
            array.extend(player.id.to_be_bytes());
            array.extend(player.position.x.to_be_bytes());
            array.extend(player.position.y.to_be_bytes());
            array.extend(player.velocity.x.to_be_bytes());
            array.extend(player.velocity.y.to_be_bytes());
            array.extend(player.orientation.to_be_bytes());
            array.push(player.design);

            let mut prop: u8 = 0;
            if player.propulsor[0] {
                prop |= 1
            }
            if player.propulsor[1] {
                prop |= 2
            }
            if player.propulsor[2] {
                prop |= 4
            }
            if player.propulsor[3] {
                prop |= 8
            }
            array.push(prop);
            array.extend(player.hits.to_be_bytes());
        }

        array
    }

    fn send_server_packet(&self, id: i32, protocol_id: u8) -> Vec<u8> {
        // Get the message.
        let message = match protocol_id {
            0 => self.send_server_binary_message(),
            _ => vec![0, 0, 0, 0], // i32 zero.
        };

        // Prepare the packet.
        let mut packet: Vec<u8> = Vec::new();
        let size = message.len() as u64;
        packet.extend(id.to_be_bytes());
        packet.extend(protocol_id.to_be_bytes());
        packet.extend(size.to_be_bytes());
        packet.extend(message);

        packet
    }
}

fn main() {
    let game = Arc::new(Mutex::new(Game::new()));

    {
        let game = Arc::clone(&game);
        thread::spawn(move || {
            let listener = TcpListener::bind("127.0.0.1:80").unwrap();

            // accept connections and process them serially
            for stream in listener.incoming().flatten() {
                game.lock().unwrap().new_player(stream);
            }
        });
    }

    loop {
        let mut game = game.lock().unwrap();
        game.iterate_game();

        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
