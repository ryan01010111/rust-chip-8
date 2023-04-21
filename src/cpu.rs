use crate::display::{Display, COLS, ROWS};
use crate::keyboard::Keyboard;

use rand::Rng;
use std::{
    cmp, fs, io,
    time::{Duration, Instant},
};

const MEMORY_SIZE: usize = 4096;
const FPS_INTERVAL: Duration = Duration::from_millis(1000 / 60);

const SPRITE_BYTES: [u8; 0x50] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

struct NextKeyParams {
    destination_idx: usize,
    valid_after: Instant,
}

pub struct Cpu {
    memory: [u8; MEMORY_SIZE],
    v: [u8; 0x10], // registers V0-VF
    i: u16,        // "I" register
    delay_timer: u8,
    sound_timer: u8, // audio not implemented
    pc: u16,         // program counter
    stack: Vec<u16>,
    last_tick: Instant,
    paused: bool,
    should_quit: bool,
    speed: u16,
    next_key_params: Option<NextKeyParams>,
    display: Display,
    keyboard: Keyboard,
}

impl Cpu {
    pub fn new(display: Display, keyboard: Keyboard) -> Self {
        Self {
            memory: [0; MEMORY_SIZE],
            v: [0; 0x10],
            i: 0,
            delay_timer: 0,
            sound_timer: 0,
            pc: 0x200,
            stack: vec![],
            last_tick: Instant::now(),
            paused: false,
            next_key_params: None,
            should_quit: false,
            speed: (700.0 * FPS_INTERVAL.as_secs_f32()) as u16, // CPU cycles per frame
            keyboard,
            display,
        }
    }

    pub fn init(&mut self, path: std::path::PathBuf) -> Result<(), io::Error> {
        self.read_sprites_into_memory();
        self.load_rom(path)?;

        self.display.init()?;
        self.keyboard.init();

        while !self.should_quit {
            self.cycle().unwrap_or_else(|err| {
                self.display.exit().unwrap();
                panic!("{:?}", err);
            });
        }

        self.display.exit()?;

        Ok(())
    }

    pub fn read_sprites_into_memory(&mut self) {
        // load into interpreter area of memory, starting at 0x000
        self.memory[..SPRITE_BYTES.len()].copy_from_slice(&SPRITE_BYTES[..]);
    }

    pub fn load_rom(&mut self, path: std::path::PathBuf) -> Result<(), io::Error> {
        let file_bytes = fs::read(path)?;

        let start_addr = 0x200;
        self.memory[start_addr..start_addr + file_bytes.len()].copy_from_slice(&file_bytes);

        Ok(())
    }

    fn cycle(&mut self) -> Result<(), io::Error> {
        self.last_tick = Instant::now();

        for _ in 0..self.speed {
            if self.next_key_params.is_some() {
                // program paused, and waiting for next key press
                self.process_next_key();
            } else {
                self.keyboard.process_pressed_keys();
                if self.paused && !self.keyboard.pause_toggle_on {
                    // not waiting for next key, and no longer paused by user
                    self.paused = false;
                }
            }

            if self.keyboard.esc_pressed {
                self.should_quit = true;
                return Ok(());
            } else if self.keyboard.pause_toggle_on {
                self.paused = true;
            }

            if self.paused {
                continue;
            }

            let opcode = ((self.memory[self.pc as usize] as u16) << 8)
                | (self.memory[self.pc as usize + 1]) as u16;
            self.exec_instruction(opcode);
        }

        if !self.paused {
            self.update_timers();
        }

        if self.keyboard.pause_toggle_on {
            self.display.render_key_map()?;
        } else {
            self.display.render()?;
        }

        // maintain 60 FPS
        let timeout = FPS_INTERVAL
            .checked_sub(self.last_tick.elapsed())
            .unwrap_or(Duration::from_secs(0));
        if !timeout.is_zero() {
            std::thread::sleep(timeout);
        }

        Ok(())
    }

    fn process_next_key(&mut self) {
        let params = self
            .next_key_params
            .as_ref()
            .expect("Attempt to process next key without setting next_key_params");

        if let Some(val) = self.keyboard.get_next_key(params.valid_after) {
            if self.keyboard.esc_pressed || self.keyboard.pause_toggle_on {
                return;
            }

            self.v[params.destination_idx] = val;
            self.paused = false;
            self.next_key_params = None;
        }
    }

    fn update_timers(&mut self) {
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }
        if self.sound_timer > 0 {
            self.sound_timer -= 1;
        }
    }

    fn exec_instruction(&mut self, opcode: u16) {
        self.pc += 2;

        let x = (opcode as usize & 0x0F00) >> 8;
        let y = (opcode as usize & 0x00F0) >> 4;

        match opcode & 0xF000 {
            0x0000 => match opcode {
                // CLS
                0x00E0 => {
                    // Clear the display.
                    self.display.clear();
                }
                // RET
                0x00EE => {
                    // Return from a subroutine.
                    // The interpreter sets the program counter to the address at the top of the
                    // stack, then subtracts 1 from the stack pointer.
                    match self.stack.pop() {
                        Some(addr) => self.pc = addr,
                        None => panic!("Attempt to pop from empty stack"),
                    }
                }
                // 0nnn - SYS addr
                _ => {
                    // *** ignored ***
                    // Jump to a machine code routine at nnn.
                    // This instruction is only used on the old computers on which Chip-8 was
                    // originally implemented. It is ignored by modern interpreters.
                }
            },
            // JP addr
            0x1000 => {
                // Jump to location nnn.
                // The interpreter sets the program counter to nnn.
                self.pc = opcode & 0xFFF;
            }
            // CALL addr
            0x2000 => {
                // Call subroutine at nnn.
                // The interpreter increments the stack pointer, then puts the current PC on the top
                // of the stack. The PC is then set to nnn.
                self.stack.push(self.pc);
                self.pc = opcode & 0xFFF;
            }
            // SE Vx, byte
            0x3000 => {
                // Skip next instruction if Vx = kk.
                // The interpreter compares register Vx to kk, and if they are equal, increments the
                // program counter by 2.
                if self.v[x] == opcode as u8 {
                    self.pc += 2;
                }
            }
            // SNE Vx, byte
            0x4000 => {
                // Skip next instruction if Vx != kk.
                // The interpreter compares register Vx to kk, and if they are not equal, increments
                // the program counter by 2.
                if self.v[x] != opcode as u8 {
                    self.pc += 2;
                }
            }
            // SE Vx, Vy
            0x5000 => {
                // Skip next instruction if Vx = Vy.
                // The interpreter compares register Vx to register Vy, and if they are equal,
                // increments the program counter by 2.
                if self.v[x] == self.v[y] {
                    self.pc += 2;
                }
            }
            // LD Vx, byte
            0x6000 => {
                // Set Vx = kk.
                // The interpreter puts the value kk into register Vx.
                self.v[x] = opcode as u8;
            }
            // ADD Vx, byte
            0x7000 => {
                // Set Vx = Vx + kk.
                // Adds the value kk to the value of register Vx, then stores the result in Vx.
                self.v[x] = (self.v[x] as u16 + opcode) as u8;
            }
            0x8000 => {
                match opcode & 0xF {
                    // LD Vx, Vy
                    0x0 => {
                        // Set Vx = Vy.
                        // Stores the value of register Vy in register Vx.
                        self.v[x] = self.v[y];
                    }
                    // OR Vx, Vy
                    0x1 => {
                        // Set Vx = Vx OR Vy.
                        // Performs a bitwise OR on the values of Vx and Vy, then stores the result
                        // in Vx. A bitwise OR compares the corrseponding bits from two values, and
                        // if either bit is 1, then the same bit in the result is also 1. Otherwise,
                        // it is 0.
                        self.v[x] |= self.v[y]
                    }
                    // AND Vx, Vy
                    0x2 => {
                        // Set Vx = Vx AND Vy.
                        // Performs a bitwise AND on the values of Vx and Vy, then stores the result
                        // in Vx. A bitwise AND compares the corrseponding bits from two values, and
                        // if both bits are 1, then the same bit in the result is also 1. Otherwise,
                        // it is 0.
                        self.v[x] &= self.v[y]
                    }
                    // XOR Vx, Vy
                    0x3 => {
                        // Set Vx = Vx XOR Vy.
                        // Performs a bitwise exclusive OR on the values of Vx and Vy, then stores
                        // the result in Vx. An exclusive OR compares the corrseponding bits from
                        // two values, and if the bits are not both the same, then the corresponding
                        // bit in the result is set to 1. Otherwise, it is 0.
                        self.v[x] ^= self.v[y]
                    }
                    // ADD Vx, Vy
                    0x4 => {
                        // Set Vx = Vx + Vy, set VF = carry.
                        // The values of Vx and Vy are added together. If the result is greater than
                        // 8 bits (i.e., > 255,) VF is set to 1, otherwise 0. Only the lowest 8 bits
                        // of the result are kept, and stored in Vx.
                        let sum = self.v[x] as u16 + self.v[y] as u16;
                        self.v[x] = sum as u8;
                        self.v[0xF] = if sum > 0xFF { 1 } else { 0 };
                    }
                    // SUB Vx, Vy
                    0x5 => {
                        // Set Vx = Vx - Vy, set VF = NOT borrow.
                        // If Vx > Vy, then VF is set to 1, otherwise 0. Then Vy is subtracted from
                        // Vx, and the results stored in Vx.
                        self.v[0xF] = if self.v[x] > self.v[y] { 1 } else { 0 };
                        self.v[x] = self.v[x].overflowing_sub(self.v[y]).0;
                    }
                    // SHR Vx {, Vy}
                    0x6 => {
                        // Set Vx = Vx SHR 1.
                        // If the least-significant bit of Vx is 1, then VF is set to 1, otherwise
                        // 0. Then Vx is divided by 2.
                        self.v[0xF] = self.v[x] & 0x1;
                        self.v[x] >>= 1;
                    }
                    // SUBN Vx, Vy
                    0x7 => {
                        // Set Vx = Vy - Vx, set VF = NOT borrow.
                        // If Vy > Vx, then VF is set to 1, otherwise 0. Then Vx is subtracted from
                        // Vy, and the results stored in Vx.
                        self.v[0xF] = if self.v[y] > self.v[x] { 1 } else { 0 };
                        self.v[x] = self.v[y].overflowing_sub(self.v[x]).0;
                    }
                    // SHL Vx {, Vy}
                    0xE => {
                        // Set Vx = Vx SHL 1.
                        // If the most-significant bit of Vx is 1, then VF is set to 1, otherwise
                        // to 0. Then Vx is multiplied by 2.
                        self.v[0xF] = self.v[x] & 0x80;
                        self.v[x] <<= 1;
                    }
                    _ => (),
                }
            }
            // SNE Vx, Vy
            0x9000 => {
                // Skip next instruction if Vx != Vy.
                // The values of Vx and Vy are compared, and if they are not equal, the program
                // counter is increased by 2.
                if self.v[x] != self.v[y] {
                    self.pc += 2;
                }
            }
            // LD I, addr
            0xA000 => {
                // Set I = nnn.
                // The value of register I is set to nnn.
                self.i = opcode & 0xFFF;
            }
            // JP V0, addr
            0xB000 => {
                // Jump to location nnn + V0.
                // The program counter is set to nnn plus the value of V0.
                self.pc = (opcode & 0xFFF) + self.v[0x0] as u16;
            }
            // RND Vx, byte
            0xC000 => {
                // Set Vx = random byte AND kk.
                // The interpreter generates a random number from 0 to 255, which is then ANDed with
                // the value kk. The results are stored in Vx. See instruction 8xy2 for more
                // information on AND.
                self.v[x] = rand::thread_rng().gen_range(0..=0xFF) & opcode as u8;
            }
            // DRW Vx, Vy, nibble
            0xD000 => {
                // Display n-byte sprite starting at memory location I at (Vx, Vy), set VF =
                // collision.
                // The interpreter reads n bytes from memory, starting at the address stored in I.
                // These bytes are then displayed as sprites on screen at coordinates (Vx, Vy).
                // Sprites are XORed onto the existing screen. If this causes any pixels
                // to be erased, VF is set to 1, otherwise it is set to 0. If the sprite is
                // positioned so part of it is outside the coordinates of the display, it wraps
                // around to the opposite side of the screen. See instruction 8xy3 for more
                // information on XOR, and section 2.4, Display, for more information on the Chip-8
                // screen and sprites.
                let sprite_byte_len = opcode & 0xF;
                let start_addr = self.i as usize;
                let x_start = self.v[x] as u16 % COLS as u16;
                let y_start = self.v[y] as u16 % ROWS as u16;
                let max_width = COLS as u16 - x_start;
                let max_height = ROWS as u16 - y_start;

                self.v[0xF] = 0;

                for row in 0..cmp::min(sprite_byte_len, max_height) {
                    let mut sprite_row = self.memory[start_addr + row as usize];

                    for col in 0..cmp::min(8, max_width) {
                        // check if leftmost bit, representing current block is set
                        if sprite_row & 0x80 > 0 {
                            let has_collision =
                                self.display.set_block(x_start + col, y_start + row);
                            if has_collision {
                                self.v[0xF] = 1;
                            }
                        }

                        sprite_row <<= 1; // shift next bit into leftmost position
                    }
                }
            }
            0xE000 => match opcode & 0xFF {
                // SKP Vx
                0x9E => {
                    // Skip next instruction if key with the value of Vx is pressed.
                    // Checks the keyboard, and if the key corresponding to the value of Vx is
                    // currently in the down position, PC is increased by 2.
                    let key_val = self.v[x];
                    if self.keyboard.is_key_pressed(key_val) {
                        self.pc += 2;
                    }
                }
                // SKNP Vx
                0xA1 => {
                    // Skip next instruction if key with the value of Vx is not pressed.
                    // Checks the keyboard, and if the key corresponding to the value of Vx is
                    // currently in the up position, PC is increased by 2.
                    let key_val = self.v[x];
                    if !self.keyboard.is_key_pressed(key_val) {
                        self.pc += 2;
                    }
                }
                _ => (),
            },
            0xF000 => match opcode & 0xFF {
                // LD Vx, DT
                0x07 => {
                    // Set Vx = delay timer value.
                    // The value of DT is placed into Vx.
                    self.v[x] = self.delay_timer;
                }
                // LD Vx, K
                0x0A => {
                    // Wait for a key press, store the value of the key in Vx.
                    // All execution stops until a key is pressed, then the value of that key is
                    // stored in Vx.
                    self.next_key_params = Some(NextKeyParams {
                        destination_idx: x as usize,
                        valid_after: Instant::now(),
                    });
                    self.paused = true;
                }
                // LD DT, Vx
                0x15 => {
                    // Set delay timer = Vx.
                    // DT is set equal to the value of Vx.
                    self.delay_timer = self.v[x];
                }
                // LD ST, Vx
                0x18 => {
                    // Set sound timer = Vx.
                    // ST is set equal to the value of Vx.
                    self.sound_timer = self.v[x];
                }
                // ADD I, Vx
                0x1E => {
                    // Set I = I + Vx.
                    // The values of I and Vx are added, and the results are stored in I.
                    self.i += self.v[x] as u16;
                }
                // LD F, Vx
                0x29 => {
                    // Set I = location of sprite for digit Vx.
                    // The value of I is set to the location for the hexadecimal sprite
                    // corresponding to the value of Vx. See section 2.4, Display, for more
                    // information on the Chip-8 hexadecimal font.
                    self.i = self.v[x] as u16 * 5; // hex sprites are 5 bytes each
                }
                // LD B, Vx
                0x33 => {
                    // Store BCD representation of Vx in memory locations I, I+1, and I+2.
                    // The interpreter takes the decimal value of Vx, and places the hundreds digit
                    // in memory at location in I, the tens digit at location I+1, and the ones
                    // digit at location I+2.
                    let idx = self.i as usize;
                    self.memory[idx] = self.v[x] / 100;
                    self.memory[idx + 1] = (self.v[x] % 100) / 10;
                    self.memory[idx + 2] = self.v[x] % 10;
                }
                // LD [I], Vx
                0x55 => {
                    // Store registers V0 through Vx in memory starting at location I.
                    // The interpreter copies the values of registers V0 through Vx into memory,
                    // starting at the address in I.
                    let start_addr = self.i as usize;
                    self.memory[start_addr..=start_addr + x].copy_from_slice(&self.v[0x0..=x])
                }
                // LD Vx, [I]
                0x65 => {
                    // Read registers V0 through Vx from memory starting at location I.
                    // The interpreter reads values from memory starting at location I into
                    // registers V0 through Vx.
                    let start_addr = self.i as usize;
                    self.v[0x0..=x].copy_from_slice(&self.memory[start_addr..=start_addr + x])
                }
                _ => (),
            },
            _ => panic!("Unknown opcode 0x{:X}", opcode),
        }
    }
}
