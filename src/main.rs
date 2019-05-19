use minifb::{Key, Scale, Window, WindowOptions};
use rand::Rng;
use std::fs::File;
use std::io;
use std::io::Read;
use std::{thread, time};

/// CHIP-8 Emulator/interpreter documentation
///
/// # Reading instructions
/// Each instrution is 2 bytes wide. They are stored in `mem[0x200..0x600-1]`.
/// We read the instructions at the PC (_program counter_), each time one instruction is read,
/// the program counter *have to be* increased by one, unless the instruction states otherwise.
/// To read the instructions you do `CpuState.mem[CpuState.pc]` and `CpuState.mem[CpuState.pc + 1]`.
///

const W: usize = 64;
const H: usize = 32;

const FONT_BASE: usize = 0;
const FONT_SIZE: usize = 5 * 16;

struct CpuState {
    // Program Counter, counts the current instruction.
    pc: usize,

    // Stack pointer
    sp: usize,

    // I register
    I: u16,

    //V0..VF registers
    V: [u8; 17],

    delay: u8,
    sound: u8,

    //Main memory
    mem: Vec<u8>,
    screen_buffer: Vec<u32>,

    key_state: [u8; 17],
}

fn get_font_sprite() -> Vec<u8> {
    //TODO: the rest of the hex characters;
    let mut one: Vec<u8> = vec![0xF0, 0x90, 0x90, 0x90, 0xF0];
    let mut two: Vec<u8> = vec![0x20, 0x60, 0x20, 0x20, 0x70];
    let mut three: Vec<u8> = vec![0xF0, 0x10, 0xF0, 0x10, 0xF0];
    let mut four: Vec<u8> = vec![0x90, 0x90, 0xF0, 0x10, 0x10];
    let mut five: Vec<u8> = vec![0xF0, 0x80, 0xF0, 0x10, 0xF0];

    one.append(&mut two);
    one.append(&mut three);
    one.append(&mut four);
    one.append(&mut five);

    return one;
}

impl CpuState {
    fn new(m: &Vec<u8>) -> CpuState {
        let mut mem = vec![0; 0x200 + m.len() + 5000];

        for i in 0x200..(m.len() + 0x200) {
            mem[i] = m[i - 0x200];
        }

        let font_arr = get_font_sprite();

        for i in 0..(font_arr.len()) {
            mem[i] = font_arr[i];
        }

        let c = CpuState {
            pc: 0x200,
            //pc: 0x00,
            sp: 0xfa0,
            I: 0,
            V: [0; 17],
            delay: 0,
            sound: 0,
            mem: mem,
            screen_buffer: vec![0; W * H],
            key_state: [0; 17],
        };

        c
    }

    fn not_impl(&mut self, _data: u8) {
        //
        // Debugging porpouses
        panic!(_data);
        self.pc += 2;
    }

    fn update_key_down(&mut self, keycode: u8) {
        if keycode > 0xF {
            return;
        }

        self.key_state[keycode as usize] = 1;
    }

    fn clear_keys(&mut self) {
        for i in self.key_state.iter_mut() {
            *i = 0;
        }
    }

    fn emulate_chip8(&mut self) {
        let op = self.mem[self.pc];
        let high_nib = (op & 0xf0) >> 4;

        //println!("-----I: {:x} V: {:?}", self.I, self.V);
        //self._disassemble_chip8();

        // Debug info
        //println!("----PC: {:?}, V: {:?}", self.pc, self.V);

        match high_nib {
            0x00 => match self.mem[self.pc + 1] {
                //
                0xE0 => {
                    //CLS
                    for i in self.screen_buffer.iter_mut() {
                        *i = 0;
                    }

                    self.pc += 2;
                }

                0xEE => {
                    //The interpreter sets the program counter to the
                    //address at the top of the stack, then subtracts
                    //1 from the stack pointer.

                    let target: u16 =
                        (((self.mem[self.sp] as u16) << 8) | self.mem[self.sp + 1] as u16) as u16;

                    self.sp += 2;
                    self.pc = target as usize;
                }

                x => {
                    println!(
                        "UNKNOWN {:X?}, {:X?}",
                        self.mem[self.pc],
                        self.mem[self.pc + 1]
                    );

                    self.pc += 2
                }
            },
            0x01 => {
                //1nnn - JUMP addr
                let addr =
                    (((self.mem[self.pc] & 0x0f) as u16) << 8) | self.mem[self.pc + 1] as u16;
                self.pc = addr as usize;
            }
            0x02 => {
                // The interpreter increments the stack pointer,
                // then puts the current PC on the top of the stack.
                // The PC is then set to nnn.

                self.sp -= 2;
                self.mem[self.sp] = (((self.pc + 2) & 0xff00) >> 8) as u8;
                self.mem[self.sp + 1] = ((self.pc + 2) & 0x00ff) as u8;

                self.pc = ((((self.mem[self.pc] as u16) & 0x0f) << 8)
                    | (self.mem[self.pc + 1] as u16)) as usize;
            }
            0x03 => {
                // 3xkk - SE Vx, byte
                // Skip next instruction if Vx = kk.
                let reg: usize = (self.mem[self.pc] & 0x0f) as usize;
                if self.V[reg] == self.mem[self.pc + 1] {
                    self.pc += 2;
                }

                self.pc += 2;
            }
            0x04 => {
                // 4xkk - SNE Vx, byte
                // Skip next instruction if Vx != kk.<Paste>

                let reg: usize = (self.mem[self.pc] & 0x0f) as usize;
                if self.V[reg] != self.mem[self.pc + 1] {
                    self.pc += 2;
                }

                self.pc += 2;
            }

            0x05 => {
                // 5xy0 - SE Vx, Vy
                // Skip next instruction if Vx = Vy.
                let regx: usize = (self.mem[self.pc] & 0x0f) as usize;
                let regy: usize = ((self.mem[self.pc + 1] & 0xf0) >> 4) as usize;

                if self.V[regx] == self.V[regy] {
                    self.pc += 2;
                }

                self.pc += 2;
            }

            0x06 => {
                // 6xkk - LD Vx, byte
                // Set Vx = kk.

                let reg: usize = (self.mem[self.pc] & 0x0f) as usize;
                self.V[reg] = self.mem[self.pc + 1];

                self.pc += 2;
            }

            0x07 => {
                // 7xkk - ADD Vx, byte
                // Set Vx = Vx + kk.

                let reg: usize = (self.mem[self.pc] & 0x0f) as usize;
                self.V[reg] += self.mem[self.pc + 1];

                self.pc += 2;
            }

            0x08 => {
                let sml_nib = self.mem[self.pc + 1] & 0x0f;

                match sml_nib {
                    0x0 => {
                        // 8xy0 - LD Vx, Vy
                        // Set Vx = Vy.
                        let regx: usize = (self.mem[self.pc] & 0x0f) as usize;
                        let regy: usize = ((self.mem[self.pc + 1] & 0xf0) >> 4) as usize;

                        self.V[regx] = self.V[regy];

                        self.pc += 2;
                    }

                    0x1 => {
                        //8xy1 - OR Vx, Vy
                        //Set Vx = Vx OR Vy.

                        let regx: usize = (self.mem[self.pc] & 0x0f) as usize;
                        let regy: usize = ((self.mem[self.pc + 1] & 0xf0) >> 4) as usize;

                        self.V[regx] |= self.V[regy];

                        self.pc += 2;
                    }

                    0x2 => {
                        // Bitwise AND;
                        let regx: usize = (self.mem[self.pc] & 0x0f) as usize;
                        let regy: usize = ((self.mem[self.pc + 1] & 0xf0) >> 4) as usize;

                        self.V[regx] &= self.V[regy];

                        self.pc += 2;
                    }

                    0x3 => {
                        // Bitwise XOR;
                        let regx: usize = (self.mem[self.pc] & 0x0f) as usize;
                        let regy: usize = ((self.mem[self.pc + 1] & 0xf0) >> 4) as usize;

                        self.V[regx] ^= self.V[regy];

                        self.pc += 2;
                    }

                    0x4 => {
                        //8xy4 - ADD Vx, Vy
                        //Set Vx = Vx + Vy, set VF = carry

                        let regx: usize = (self.mem[self.pc] & 0x0f) as usize;
                        let regy: usize = ((self.mem[self.pc + 1] & 0xf0) >> 4) as usize;

                        let res: u16 = self.V[regx] as u16 + self.V[regy] as u16;

                        self.V[0xF] = match res & 0xff00 {
                            0 => 0,
                            x => 1,
                        };

                        self.V[regx] = (res & 0x00ff) as u8;

                        self.pc += 2;
                    }

                    0x5 => {
                        //8xy5 - SUB Vx, Vy
                        //Set Vx = Vx - Vy, set VF = NOT borrow.

                        let regx: usize = (self.mem[self.pc] & 0x0f) as usize;
                        let regy: usize = ((self.mem[self.pc + 1] & 0xf0) >> 4) as usize;

                        let bg: bool = self.V[regx] > self.V[regy];

                        match bg {
                            true => self.V[0xF] = 1,
                            false => self.V[0xF] = 0,
                        };

                        self.V[regx] -= self.V[regy];

                        self.pc += 2;
                    }

                    0x6 => {
                        //If the least-significant bit of Vx is 1,
                        //then VF is set to 1, otherwise 0.
                        //Then Vx is divided by 2.

                        let regx: usize = (self.mem[self.pc] & 0x0f) as usize;

                        self.V[0xF] = self.V[regx] & 1;
                        self.V[regx] /= 2;

                        self.pc += 2;
                    }

                    0x7 => {
                        //8xy7 - SUBN Vx, Vy
                        //Set Vx = Vy - Vx, set VF = NOT borrow.

                        let regx: usize = (self.mem[self.pc] & 0x0f) as usize;
                        let regy: usize = ((self.mem[self.pc + 1] & 0xf0) >> 4) as usize;

                        self.V[0xF] = match self.V[regy] > self.V[regx] {
                            true => 1,
                            false => 0,
                        };

                        self.V[regx] = self.V[regy] - self.V[regx];

                        self.pc += 2;
                    }

                    0xE => {
                        //
                        let regx: usize = (self.mem[self.pc] & 0x0f) as usize;

                        self.V[0xF] = self.V[regx] & (1 << 7);
                        self.V[regx] *= 2;

                        self.pc += 2;
                    }

                    x => {
                        println!(
                            "UNKNOWN {:X?}, {:X?}",
                            self.mem[self.pc],
                            self.mem[self.pc + 1]
                        );

                        self.pc += 2
                    }
                }
            }

            0x9 => {
                let rx: usize = (self.mem[self.pc] & 0x0f) as usize;
                let ry: usize = ((self.mem[self.pc + 1] & 0xf0) >> 4) as usize;

                if self.V[rx] != self.V[ry] {
                    self.pc += 2;
                }

                self.pc += 2;
            }

            0xA => {
                //
                // I register, used to store mem addresses.
                self.I = (((self.mem[self.pc] as u16) & 0x0f) << 8) | self.mem[self.pc + 1] as u16;

                self.pc += 2;
            }

            0xB => {
                //
                self.pc = ((((self.mem[self.pc] as u16 & 0x0f) << 8)
                    | (self.mem[self.pc + 1]) as u16)
                    + (self.V[0]) as u16) as usize;

                self.pc += 2;
            }

            0xC => {
                let mut rng = rand::thread_rng();
                let r: u8 = rng.gen();

                let x = (self.mem[self.pc] & 0x0f) as usize;

                // Right implementation
                self.V[x] = r & self.mem[self.pc + 1];

                //My funny implementation
                //self.V[x] = r;

                self.pc += 2;
            }

            0xD => {
                let regx: usize = (self.mem[self.pc] & 0x0f) as usize;
                let regy: usize = ((self.mem[self.pc + 1] & 0xf0) >> 4) as usize;
                let n: usize = (self.mem[self.pc + 1] & 0x0f) as usize;

                let x: usize = self.V[regx] as usize;
                let y: usize = self.V[regy] as usize;

                for i in 0..n {
                    for j in (-8)..(0) {
                        if self.mem[i + (self.I as usize)] & (1 << j) != 0 {
                            let ii: usize = (i as usize + y) % H;
                            //TODO: attention in here.
                            let jj: usize = ((x - 1) - j as usize) % W;

                            //TODO: implement XOR with V[0xF] register
                            //if self.screen_buffer[(ii * W) + jj] == 0xffffff {
                            //self.V[0xF] = 1;
                            //}

                            self.screen_buffer[(ii * W) + jj] ^= 0xffffff;
                        }
                    }
                }

                self.pc += 2;
            }

            0xE => match self.mem[self.pc + 1] {
                0x9E => {
                    let reg: usize = (self.mem[self.pc] & 0x0f) as usize;
                    if (self.key_state[self.V[reg] as usize] == 1) {
                        self.pc += 2;
                    }

                    self.pc += 2;
                }

                0xA1 => {
                    let reg: usize = (self.mem[self.pc] & 0x0f) as usize;
                    if (self.key_state[self.V[reg] as usize] == 0) {
                        self.pc += 2;
                    }

                    self.pc += 2;
                }
                x => {
                    println!(
                        "UNKNOWN {:X?}, {:X?}",
                        self.mem[self.pc],
                        self.mem[self.pc + 1]
                    );
                    self.pc += 2
                }
            },

            0xF => match self.mem[self.pc + 1] {
                0x7 => {
                    let x: usize = (self.mem[self.pc] & 0x0f) as usize;
                    self.V[x] = self.delay;

                    self.pc += 2;
                }

                0x15 => {
                    self.delay = (self.mem[self.pc] & 0x0f);

                    self.pc += 2;
                }

                0x18 => {
                    self.sound = (self.mem[self.pc] & 0x0f);

                    self.pc += 2;
                }

                0x29 => {
                    let reg: usize = (self.mem[self.pc] & 0x0f) as usize;
                    self.I = FONT_BASE as u16 + (self.V[reg] * 5) as u16;

                    self.pc += 2;
                }

                0x33 => {
                    let reg: usize = (self.mem[self.pc] & 0x0f) as usize;
                    let mut val: u8 = self.V[reg];

                    let ones: u8 = val % 10;
                    val /= 10;
                    let tens: u8 = val % 10;
                    val /= 10;
                    let hundreds: u8 = val % 10;

                    self.mem[self.I as usize] = hundreds;
                    self.mem[self.I as usize + 1] = tens;
                    self.mem[self.I as usize + 2] = ones;

                    self.pc += 2;
                }

                0x55 => {
                    let x: usize = (self.mem[self.pc] & 0x0f) as usize;

                    for i in 0..=x {
                        self.mem[(self.I as usize) + i] = self.V[i];
                    }

                    self.I += (x + 1) as u16;

                    self.pc += 2;
                }

                0x65 => {
                    //Fx65 - LD Vx, [I]
                    //Read registers V0 through Vx from memory starting at location I.

                    let x: usize = (self.mem[self.pc] & 0x0f) as usize;

                    for i in 0..=x {
                        self.V[i] = self.mem[(self.I as usize) + i]
                    }

                    self.I += (x + 1) as u16;

                    self.pc += 2;
                }

                0x0A => {
                    let regx: usize = (self.mem[self.pc] & 0x0f) as usize;

                    for (index, i) in self.key_state.iter().enumerate() {
                        if *i != 0 {
                            self.pc += 2;
                            self.V[regx] = index as u8;
                            break;
                        }
                    }
                }

                0x1E => {
                    let x: usize = (self.mem[self.pc] & 0x0f) as usize;
                    self.I += self.V[x] as u16;

                    self.pc += 2;
                }

                x => {
                    println!(
                        "UNKNOWN {:X?}, {:X?}",
                        self.mem[self.pc],
                        self.mem[self.pc + 1]
                    );

                    self.pc += 2
                }
            },

            x => self.not_impl(x),
        }
    }

    fn disassemble_chip8(&mut self) {
        loop {
            if self.pc + 1 >= self.mem.len() {
                break;
            }

            self.pc += self._disassemble_chip8();
        }
    }

    fn _disassemble_chip8(&self) -> usize {
        let instruction_size = 2;

        let data = &self.mem;

        let nibble = data[self.pc] >> 4;

        match nibble {
            0x0 => match data[self.pc + 1] {
                0xe0 => println!("CLS"),
                0xee => println!("RET"),

                x => println!("00{:02x} not implemented", x),
            },

            // Using the lowest 12 bits by masking out the 4 upper bits
            0x1 => println!(
                "JUMP ${:02x}{:04x}",
                data[self.pc] & 0x0f,
                data[self.pc + 1]
            ),
            0x2 => println!(
                "CALL ${:02x}{:04x}",
                data[self.pc] & 0x0f,
                data[self.pc + 1]
            ),
            // SKIP EQUALS
            0x3 => println!(
                "SE V{:02x}, #${:04x}",
                data[self.pc] & 0x0f,
                data[self.pc + 1]
            ),

            0x4 => println!(
                "SNE V{:02x}, #${:04x}",
                data[self.pc] & 0x0f,
                data[self.pc + 1]
            ),
            // 5xy0 - SE Vx, Vy
            0x5 => println!(
                "SE V{:02x}, V{:02x}",
                data[self.pc] & 0x0f,
                data[self.pc + 1] & 0xf0
            ),

            0x6 => println!(
                "LD V{:02x}, #${:04x}",
                data[self.pc] & 0x0f,
                data[self.pc + 1]
            ),

            0x7 => println!(
                "ADD V{:02x}, #${:04x}",
                data[self.pc] & 0x0f,
                data[self.pc + 1]
            ),

            0x8 => {
                let nib = data[self.pc + 1] >> 4;
                match nib {
                    0 => println!(
                        "LD V{:02x}, V{:02x}",
                        data[self.pc] & 0x0f,
                        data[self.pc + 1] & 0xf0
                    ),

                    1 => println!(
                        "OR V{:02x}, V{:02x}",
                        data[self.pc] & 0x0f,
                        data[self.pc + 1] & 0xf0
                    ),
                    2 => println!(
                        "AND V{:02x}, V{:02x}",
                        data[self.pc] & 0x0f,
                        data[self.pc + 1] & 0xf0
                    ),

                    3 => println!(
                        "XOR V{:02x}, V{:02x}",
                        data[self.pc] & 0x0f,
                        data[self.pc + 1] & 0xf0
                    ),
                    4 => println!(
                        "ADD V{:02x}, V{:02x}",
                        data[self.pc] & 0x0f,
                        data[self.pc + 1] & 0xf0
                    ),

                    5 => println!(
                        "SUB V{:02x}, V{:02x}",
                        data[self.pc] & 0x0f,
                        data[self.pc + 1] & 0xf0
                    ),
                    6 => println!(
                        "SHR V{:02x}, V{:02x}",
                        data[self.pc] & 0x0f,
                        data[self.pc + 1] & 0xf0
                    ),

                    7 => println!(
                        "SUBN V{:02x}, V{:02x}",
                        data[self.pc] & 0x0f,
                        data[self.pc + 1] & 0xf0
                    ),
                    0xe => println!(
                        "SHL V{:02x}, V{:02x}",
                        data[self.pc] & 0x0f,
                        data[self.pc + 1] & 0xf0
                    ),

                    x => println!("{:04x} not implemented", x),
                }
            }

            0x9 => println!(
                "SNE V{:02x}, V{:02x}",
                data[self.pc] & 0x0f,
                data[self.pc + 1] & 0xf0
            ),

            0xA => println!(
                "LD I, ${:03x}",
                (((data[self.pc] as u32 & 0x0f) << 8) | data[self.pc + 1] as u32)
            ),

            0xB => println!(
                "JUMP V0, ${:02x}{:04x}",
                data[self.pc] & 0x0f,
                data[self.pc + 1]
            ),

            // Set Vx = random byte AND kk.
            0xC => println!(
                "RND V{:02x}, #${:04x}",
                data[self.pc] & 0x0f,
                data[self.pc + 1]
            ),

            // Display n-byte sprite starting at memory location I at (Vx, Vy),
            0xD => println!(
                "DRAW V{:02x}, V{:02x}, #${:02x}",
                data[self.pc] & 0x0f,
                (data[self.pc + 1] & 0xf0) >> 1,
                data[self.pc + 1] & 0x0f
            ),

            0xE => match data[self.pc + 1] {
                0x9E => println!("SKP V{:02x}", data[self.pc] & 0x0f),
                0xA1 => println!("SKNP V{:02x}", data[self.pc] & 0x0f),
                _ => println!("E{:02x}{:04x}", data[self.pc] & 0x0f, data[self.pc + 1]),
            },

            0xF => match data[self.pc + 1] {
                0x7 => println!("LD V{:02x}, DT", data[self.pc] & 0x0f),
                0xA => println!("LD V{:02x}, K", data[self.pc] & 0x0f),
                0x15 => println!("LD DT, V{:02x}", data[self.pc] & 0x0f),
                0x18 => println!("LD ST, V{:02x}", data[self.pc] & 0x0f),
                0xE => println!("ADD I, V{:02x}", data[self.pc] & 0x0f),
                0x29 => println!("LD F, V{:02x}", data[self.pc] & 0x0f),
                0x33 => println!("LD B, V{:02x}", data[self.pc] & 0x0f),
                0x55 => println!("LD [I], V{:02x}", data[self.pc] & 0x0f),
                0x65 => println!("LD V{:02x}, [I]", data[self.pc] & 0x0f),

                x => println!("F{:04x} not implemented", x),
            },

            x => println!("{:04x} not implemented", x),
        }

        instruction_size
    }
}

fn main() -> io::Result<()> {
    //let args: Vec<String> = env::args().collect();
    //if args.len() == 0 {
    //panic!("Please provide the ROM's file path");
    //}
    //
    let mut f = File::open("./roms/game_sub.ch8")?;

    let mut data = Vec::new();
    f.read_to_end(&mut data)?;

    let mut cpu = CpuState::new(&data);

    //cpu.disassemble_chip8();

    let mut window = Window::new(
        "CHIP-8",
        W,
        H,
        WindowOptions {
            resize: false,
            scale: Scale::X16,
            ..WindowOptions::default()
        },
    )
    .unwrap();

    while window.is_open() {
        //thread::sleep(time::Duration::from_millis(1));

        cpu.clear_keys();

        if window.is_key_pressed(Key::Space, minifb::KeyRepeat::No) {
            cpu.emulate_chip8();
            cpu._disassemble_chip8();
        }

        if window.is_key_down(Key::Escape) {
            break;
        }

        window.get_keys().map(|keys| {
            for t in keys {
                match t {
                    Key::Key0 => cpu.update_key_down(0),
                    Key::Key1 => cpu.update_key_down(1),
                    Key::Key2 => cpu.update_key_down(2),
                    Key::Key3 => cpu.update_key_down(3),
                    Key::Key4 => cpu.update_key_down(4),
                    Key::Key5 => cpu.update_key_down(5),
                    Key::Key6 => cpu.update_key_down(6),
                    Key::Key7 => cpu.update_key_down(7),
                    Key::Key8 => cpu.update_key_down(8),
                    Key::Key9 => cpu.update_key_down(9),
                    Key::A => cpu.update_key_down(0xA),
                    Key::B => cpu.update_key_down(0xB),
                    Key::C => cpu.update_key_down(0xC),
                    Key::D => cpu.update_key_down(0xD),
                    Key::E => cpu.update_key_down(0xE),
                    Key::F => cpu.update_key_down(0xF),
                    _ => (),
                }
            }
        });

        cpu.emulate_chip8();

        window.update_with_buffer(&cpu.screen_buffer).unwrap();
    }

    Ok(())
}
