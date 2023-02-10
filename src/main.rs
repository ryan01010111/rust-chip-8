mod cpu;
mod display;
mod keyboard;

use cpu::Cpu;
use display::Display;
use keyboard::Keyboard;

use crossterm::{
    cursor, style, terminal,
};
use std::{
    fs, process,
    io::{Write, self},
    path::Path,
};

fn main() -> Result<(), io::Error> {
    loop {
        // check for ROMS dir
        let roms_path = Path::new("./roms");
        if !roms_path.exists() || !roms_path.is_dir() {
            eprintln!("\
Please add a folder named \"roms\" containing your ROMs to the same folder as this program.");
            process::exit(1);
        }

        let file_names = fs::read_dir(roms_path)?
            .flatten() // remove Errs
            .filter(|dir_entry| match dir_entry.file_type() { // collect only files
                Ok(file_type) => file_type.is_file(),
                Err(_) => false,
            })
            .map(|dir| dir.file_name())
            .collect::<Vec<_>>();

        // ROM selection
        let rom_idx = prompt_rom_selection(&file_names)?;

        let file_name = &file_names[rom_idx];
        let rom_path = roms_path.join(file_name);

        // start up CHIP-8
        let display = Display::new();
        let keyboard = Keyboard::new();
        let mut cpu = Cpu::new(display, keyboard);

        cpu.init(rom_path)?;
    }
}

fn prompt_rom_selection(file_names: &Vec<std::ffi::OsString>) -> Result<usize, io::Error> {
    let mut stdout = io::stdout();
    let spacer = "=".repeat(64);
    let header = " ".repeat(29) + "CHIP-8";

    let mut is_first_try = true;
    loop {
        crossterm::execute!(stdout,
            terminal::Clear(terminal::ClearType::All),
            style::SetForegroundColor(style::Color::Green),
            cursor::MoveTo(0, 0),
        )?;

        println!("{}", spacer);
        println!("{}", header);
        println!("{}", spacer);
        println!(" Detected ROMs:\n");
        for (idx, file_name) in file_names.iter().enumerate() {
            println!("  [{}] {}", idx, file_name.to_string_lossy());
        }
        println!("\n q = quit");
        println!("{}", spacer);

        if is_first_try {
            println!("\nSelect a ROM:");
            is_first_try = false;
        } else {
            println!("\nInvalid selection. Please enter the number for a ROM, and press ENTER.")
        }
        print!("> ");
        stdout.flush()?;

        let stdin = io::stdin();
        let mut raw_input = String::new();
        stdin.read_line(&mut raw_input)?;
        let input = raw_input.trim();

        if input == "q" { process::exit(0); }
        if let Ok(num) = input.parse::<usize>() {
            if num < file_names.len() { return Ok(num); }
        }
    }

}
