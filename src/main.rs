mod protocol;
mod vector;

use protocol::ClientPacket;
use std::collections::BTreeSet;
use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use vector::Vector;

use crate::protocol::{Packet, PacketProtocol};

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

struct Ship {
	id: i32,
	position: Vector,
	velocity: Vector,
	orientation: f32,
	design: u8,
	propulsor: [bool; 4],
	can_shoot: i32,
	hits: i32,
}

impl Ship {
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
	ship_id: i32,
	orientation: f32,
	propulsor: [bool; 4],
}

struct PlayerData {
	stream: TcpStream,
	ships: BTreeSet<usize>,
	buffer: Vec<u8>,
	remaining_message: usize,
	remaining_header: usize,
	messages_received: i32,
	protocol: u8,
}

impl PlayerData {
	const HEADER_SIZE: usize = 9;

	fn new(stream: TcpStream) -> Self {
		PlayerData {
			stream,
			ships: BTreeSet::new(),
			buffer: Vec::new(),
			remaining_message: 0,
			remaining_header: Self::HEADER_SIZE,
			messages_received: 0,
			protocol: 0,
		}
	}
	// Protocol zero.
	// [  32 bits  |   8 bits    |     32 bits     | message ]
	// [message id | protocol id | size of message | message ]

	// Client Message:
	// [ 32 bits   |   32 bits   |  8 bits   ]
	// [ player id | orientation | propulsor ]
	fn read_client_binary_message(&mut self) -> ClientData {
		let message: Vec<_> = self.buffer.drain(0..=8).collect();
		let ship_id = i32::from_be_bytes([message[0], message[1], message[2], message[3]]);
		let orientation = f32::from_be_bytes([message[4], message[5], message[6], message[7]]);
		let propulsor = message[8];

		let pw = propulsor & 0b0001 != 0;
		let pa = propulsor & 0b0010 != 0;
		let ps = propulsor & 0b0100 != 0;
		let pd = propulsor & 0b1000 != 0;

		ClientData {
			ship_id,
			orientation,
			propulsor: [pw, pa, ps, pd],
		}
	}
}

struct Game {
	// Player data.
	ships: Vec<Ship>,
	players: Vec<PlayerData>,
}

impl Game {
	fn new() -> Self {
		Game {
			ships: Vec::new(),
			players: Vec::new(),
		}
	}

	fn new_player(&mut self, new_stream: TcpStream) {
		let amount = self.ships.len();
		self.ships.push(Ship {
			id: amount as i32,
			position: Vector { x: 0.0, y: 0.0 },
			velocity: Vector { x: 0.0, y: 0.0 },
			orientation: 0.0,
			design: 0,
			propulsor: [false, false, false, false],
			can_shoot: 0,
			hits: 0,
		});

		self.players.push(PlayerData::new(new_stream));
	}

	// This iterates the game with respect to time.
	fn iterate_game(&mut self, elapsed_time: f32) {
		for player in self.players.iter_mut() {
			// Verify if we need to read the header. If yes, do so.
			if player.remaining_header != 0 {
				let mut bytes = vec![0; player.remaining_header];
				let size_read = player
					.stream
					.read(&mut bytes[0..player.remaining_header])
					.unwrap();

				// If receive full header, process it and proceed to message.
				if size_read == player.remaining_header {
					player.buffer.append(&mut bytes);
					let id = i32::from_be_bytes([
						player.buffer[0],
						player.buffer[1],
						player.buffer[2],
						player.buffer[3],
					]);

					let protocol = bytes[4];
					let size_of_message = i32::from_be_bytes([
						player.buffer[5],
						player.buffer[6],
						player.buffer[7],
						player.buffer[8],
					]);

					// Save received header. Clear the buffer.
					player.protocol = protocol;
					player.remaining_header = 0;
					player.remaining_message = size_of_message as usize;
					player.buffer.clear();
				}
				// If not, save it in the buffer and move on.
				else {
					player.buffer.append(&mut bytes);
					player.remaining_header -= size_read;
				}
			}

			// Proceed and read message.
			if player.remaining_message != 0 {
				let mut bytes = vec![0; player.remaining_message];
				let size_read = player
					.stream
					.read(&mut bytes[0..player.remaining_message])
					.unwrap();

				// If receive full message, catalog it and proceed.
				if size_read == player.remaining_message {
					player.buffer.append(&mut bytes);
				// let client_data = self.read_client_binary_message(&player.buffer);
				}
				// If not received full message, save in buffer and move on.
				else {
					player.buffer.append(&mut bytes);
					player.remaining_message -= size_read;
				}
			}
		}
	}

	// Server Message:
	// [ 32 bits   |     2 * 3 * 32 bits   |   32 bits   | 8 bits |  8 bits   | 32 bits ]
	// [ player id | position and velocity | orientation | design | propulsor | hits ]
	fn send_server_binary_message(&self) -> Vec<u8> {
		let mut array: Vec<u8> = Vec::new();

		for ship in self.ships.iter() {
			array.extend(ship.id.to_be_bytes());
			array.extend(ship.position.x.to_be_bytes());
			array.extend(ship.position.y.to_be_bytes());
			array.extend(ship.velocity.x.to_be_bytes());
			array.extend(ship.velocity.y.to_be_bytes());
			array.extend(ship.orientation.to_be_bytes());
			array.push(ship.design);

			let mut prop: u8 = 0;
			if ship.propulsor[0] {
				prop |= 1
			}
			if ship.propulsor[1] {
				prop |= 2
			}
			if ship.propulsor[2] {
				prop |= 4
			}
			if ship.propulsor[3] {
				prop |= 8
			}
			array.push(prop);
			array.extend(ship.hits.to_be_bytes());
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
	let message = ClientPacket {
		player_id: 1,
		orientation: 5,
		propulsor: 0b1101,
	};
	println!(
		"Zero Protocol: {:?}",
		PacketProtocol::Zero(message.clone()).serialize().unwrap()
	);
	println!(
		"JSON Protocol: {}",
		String::from_utf8_lossy(&PacketProtocol::Json(message).serialize().unwrap())
	);

	let received_bytes: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0, 1, 0, 0, 0, 5, 13];

	let received_message: ClientPacket = PacketProtocol::try_from(received_bytes)
		.unwrap()
		.deserialize()
		.unwrap();

	println!("\n...In another computer: {:?}", received_message);

	println!("");

	#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	struct MyMessage {
		a: u8,
		b: f32,
		c: String,
	}

	impl Packet for MyMessage {
		fn id() -> u32 {
			123
		}
	}

	let message = MyMessage {
		a: 10,
		b: 5.122,
		c: String::from("Rust rocks !!"),
	};

	println!(
		"Zero Protocol: {:?}",
		PacketProtocol::Zero(message.clone()).serialize().unwrap()
	);
	println!(
		"JSON Protocol: {}",
		String::from_utf8_lossy(&PacketProtocol::Json(message).serialize().unwrap())
	);

	// let game = Arc::new(Mutex::new(Game::new()));

	// {
	// 	let game = Arc::clone(&game);
	// 	thread::spawn(move || {
	// 		let listener = TcpListener::bind("127.0.0.1:50000").unwrap();

	// 		// accept connections and process them serially
	// 		for stream in listener.incoming().flatten() {
	// 			game.lock().unwrap().new_player(stream);
	// 		}
	// 	});
	// }

	// let mut now = std::time::Instant::now();
	// loop {
	// 	let mut game = game.lock().unwrap();
	// 	game.iterate_game(now.elapsed().as_secs() as f32);
	// 	std::thread::sleep(std::time::Duration::from_secs(1));
	// 	now = std::time::Instant::now();
	// }
}
