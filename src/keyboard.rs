use std::{
    thread,
    collections::HashMap,
    sync::mpsc::{Receiver, Sender, TryRecvError, channel},
    time::{Duration, Instant},
};
use crossterm::event;

const KEY_PRESS_TTL: Duration = Duration::from_millis(100);

pub struct Keyboard {
    key_map: HashMap<char, u8>,
    key_press_tx: Sender<(event::KeyCode, Instant)>,
    key_press_rx: Receiver<(event::KeyCode, Instant)>,
    pressed_keys: HashMap<u8, Instant>,
    pub esc_pressed: bool,
    pub pause_toggle_on: bool,
}

impl Keyboard {
    pub fn new() -> Self {
        let (
            tx,
            rx
        ) = channel::<(event::KeyCode, Instant)>();

        Self {
            key_map: HashMap::from([
                ('1', 0x1),
                ('2', 0x2),
                ('3', 0x3),
                ('4', 0xC),
                ('q', 0x4),
                ('w', 0x5),
                ('e', 0x6),
                ('r', 0xD),
                ('a', 0x7),
                ('s', 0x8),
                ('d', 0x9),
                ('f', 0xE),
                ('z', 0xA),
                ('x', 0x0),
                ('c', 0xB),
                ('v', 0xF),
            ]),
            key_press_tx: tx,
            key_press_rx: rx,
            pressed_keys: HashMap::new(),
            esc_pressed: false,
            pause_toggle_on: false,
        }
    }

    pub fn init(&mut self) {
        self.listen();
    }

    pub fn listen(&mut self) {
        let  tx = self.key_press_tx.clone();

        thread::spawn(move || loop {
            let ev = event::read().unwrap();
            if let event::Event::Key(key) = ev {
                match key.code {
                    event::KeyCode::Char(_) => {
                        tx.send((key.code, Instant::now())).unwrap();
                    },
                    event::KeyCode::Esc => {
                        tx.send((key.code, Instant::now())).unwrap();
                        break;
                    },
                    _ => (),
                }
            }
        });
    }

    pub fn process_pressed_keys(&mut self) {
        loop {
            match self.key_press_rx.try_recv() {
                Ok((key, timestamp)) => {
                    match key {
                        event::KeyCode::Char(ch) => {
                            if ch == ' ' {
                                self.pause_toggle_on = !self.pause_toggle_on;
                                break;
                            } else if let Some(hex_key) = self.key_map.get(&ch) {
                                self.pressed_keys.insert(*hex_key, timestamp);
                            }
                        },
                        event::KeyCode::Esc => {
                            self.esc_pressed = true;
                            break;
                        },
                        _ => (),
                    }
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => panic!("Keyboard event receiver disconnected"),
            }
        }
    }

    pub fn get_next_key(&mut self, valid_after: Instant) -> Option<u8> {
        loop {
            match self.key_press_rx.try_recv() {
                Ok((key, timestamp)) => {
                    match key {
                        event::KeyCode::Char(ch) => {
                            if timestamp < valid_after { continue; }
                            if let Some(val) = self.key_map.get(&ch) { return Some(*val); }
                            if ch == ' ' {
                                self.pause_toggle_on = !self.pause_toggle_on;
                                return None;
                            }
                        },
                        event::KeyCode::Esc => {
                            self.esc_pressed = true;
                            return None
                        },
                        _ => continue,
                    }
                },
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => panic!("Keyboard event receiver disconnected"),
            }
        }
    }

    pub fn is_key_pressed(&self, key_val: u8) -> bool {
        if let Some(last_press) = self.pressed_keys.get(&key_val) {
            last_press.elapsed() < KEY_PRESS_TTL
        } else {
            false
        }
    }
}
