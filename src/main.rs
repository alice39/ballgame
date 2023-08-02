use std::ops::{Add, Sub, Mul, Div, AddAssign, SubAssign, MulAssign, DivAssign, Neg};
use std::net::{TcpListener, TcpStream};
use std::thread;

#[derive(Debug, Clone, Copy, Default)]
struct Vector {
    pub x: f32,
    pub y: f32
}

impl Add<Vector> for Vector {
    type Output = Vector;

    fn add(self, rhs: Vector) -> Self::Output {
        Vector {
            x: self.x + rhs.x,
            y: self.y + rhs.y
        }
    }
}

impl Sub<Vector> for Vector {
    type Output = Vector;

    fn sub(self, rhs: Vector) -> Self::Output {
        Vector {
            x: self.x - rhs.x,
            y: self.y - rhs.y
        }
    }
}

impl Mul<Vector> for Vector {
    type Output = f32;

    fn mul(self, rhs: Vector) -> Self::Output {
        self.x * rhs.x + self.y * rhs.y
    }
}

impl Mul<f32> for Vector {
    type Output = Vector;

    fn mul(self, rhs: f32) -> Self::Output {
        Vector {
            x: self.x * rhs,
            y: self.y * rhs
        }
    }
}

impl Mul<Vector> for f32 {
    type Output = Vector;

    fn mul(self, rhs: Vector) -> Self::Output {
        Vector {
            x: self * rhs.x,
            y: self * rhs.y
        }
    }
}

impl Div<f32> for Vector {
    type Output = Vector;

    fn div(self, rhs: f32) -> Self::Output {
        Vector {
            x: self.x / rhs,
            y: self.y / rhs
        }
    }
}

impl Neg for Vector {
    type Output = Vector;

    fn neg(self) -> Self::Output {
        Vector {
            x: -self.x,
            y: -self.y
        }
    }
}

impl AddAssign<Vector> for Vector {
    fn add_assign(&mut self, rhs: Vector) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl SubAssign<Vector> for Vector {
    fn sub_assign(&mut self, rhs: Vector) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl MulAssign<f32> for Vector {
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl DivAssign<f32> for Vector {
    fn div_assign(&mut self, rhs: f32) {
        self.x /= rhs;
        self.y /= rhs;
    }
}

struct Bullet {
	pub id: i32,
	pub position: Vector,
	pub velocity: Vector,
}

impl Bullet {
	fn update(&mut self, dt : f32) {
		self.position += self.velocity * dt;	
	}
}

struct Player {
	id: i32,
	position: Vector,
	velocity: Vector,
	orientation: f32,
	design: u8,
	propulsor : [bool; 4],
	can_shoot : i32,
	hits : i32,
}

impl Player {
	fn update(&mut self, dt : f32) {
		let mut acc : Vector = Vector { x: 0.0, y: 0.0, };
		
		// WASD order.	
		if self.propulsor[0] == true { acc.y += 1.0; }
		if self.propulsor[1] == true { acc.x -= 1.0; }
		if self.propulsor[2] == true { acc.y -= 1.0; }
		if self.propulsor[3] == true { acc.x += 1.0; }

		// Update response.
		self.position += self.velocity * 0.5 * dt + acc * dt * dt;
		self.velocity += acc * dt;
	}

	fn shoot(&mut self) {
		self.velocity.x -= 0.1 * f32::cos(self.orientation);
		self.velocity.y -= 0.1 * f32::sin(self.orientation);		
	}

	fn receive_hit(&mut self, bullet : &Bullet) {
		self.velocity += 0.1 * bullet.velocity;
		self.hits += 1;
	}
}

struct ClientData {
	id: i32,
	orientation : f32,
	propulsor : [bool; 4],
}

struct Game {
	// Player data.
	players : Vec<Player>,
	streams : Vec<TcpStream>,
}

impl Game {
	fn new() -> Self {
		Game {
			players: Vec::new(),
			streams: Vec::new(),
		}
	}

	fn new_player(&mut self, new_stream : TcpStream) {
		let amount = self.players.len();
		self.players.push(Player {
			id: amount as i32,
			position : Vector{x: 0.0, y: 0.0},
			velocity : Vector{x: 0.0, y: 0.0},
			orientation : 0.0,
			design : 0,
			propulsor : [false,false,false,false],
			can_shoot : 0,
			hits : 0,
		});

		self.streams.push(new_stream);
	}

	// This iterates the game with respect to time.
	fn iterate_game(&mut self) {
			
	}

	// Protocol zero.
	// [  32 bits  |   8 bits    |     64 bits     | message ]
	// [message id | protocol id | size of message | message ]

	// Client Message:
	// [ 32 bits   |   32 bits   |  8 bits   ]
	// [ player id | orientation | propulsor ] 
	fn read_client_binary_message(&self, message : &Vec<u8>) -> ClientData {
		let id = i32::from_be_bytes([message[0], message[1], message[2], message[3]]);	
		let orientation = f32::from_be_bytes([message[4], message[5], message[6], message[7]]);
		let propulsor = message[8];

		let pw = if propulsor & 1 == 0 { false } else { true };
		let pa = if propulsor & 2 == 0 { false } else { true };
		let ps = if propulsor & 4 == 0 { false } else { true };
		let pd = if propulsor & 8 == 0 { false } else { true };

		return ClientData {
			id: id,
			orientation: orientation,
			propulsor: [pw, pa, ps, pd],
		}
	}
	
	// Server Message:
	// [ 32 bits   |     2 * 3 * 32 bits   |   32 bits   | 8 bits |  8 bits   | 32 bits ]
	// [ player id | position and velocity | orientation | design | propulsor | hits ] 
	fn send_server_binary_message(&self) -> Vec<u8> {
		let mut array : Vec<u8> = Vec::new();
		
		for player in self.players.iter() {
			array.extend(player.id.to_be_bytes());
			array.extend(player.position.x.to_be_bytes());
			array.extend(player.position.y.to_be_bytes());
			array.extend(player.velocity.x.to_be_bytes());
			array.extend(player.velocity.y.to_be_bytes());
			array.extend(player.orientation.to_be_bytes());
			array.push(player.design);
			
			let mut prop : u8 = 0;
			if player.propulsor[0] == true { prop |= 1 }
			if player.propulsor[1] == true { prop |= 2 }
			if player.propulsor[2] == true { prop |= 4 }
			if player.propulsor[3] == true { prop |= 8 }
			array.push(prop);
			array.extend(player.hits.to_be_bytes());
		}

		return array;
	}

	fn send_server_packet(&self, id: i32, protocol_id: u8) -> Vec<u8> {
		// Get the message.
		let message = match protocol_id {
			0 => self.send_server_binary_message(),
			_ => vec![0, 0, 0, 0], // i32 zero.
		};

		// Prepare the packet.
		let mut packet : Vec<u8> = Vec::new();
		let size = message.len() as u64;
		packet.extend(id.to_be_bytes());
		packet.extend(protocol_id.to_be_bytes());
		packet.extend(size.to_be_bytes());
		packet.extend(message);
		return packet;
	}
}


/*fn main() {
    let mut game = Game::new();

    thread::scope(|s| {
        s.spawn(|| {
            let listener = TcpListener::bind("127.0.0.1:80").unwrap();

            // accept connections and process them serially
            for stream in listener.incoming() {
                if let Ok(stream) = stream {
                    game.new_player(stream);
                }
            }
        });

        s.spawn(|| {
            // possible a loop here
            game.iterate_game();
        })
    });
}*/


fn main() {
	println!("Hello World!")
}
