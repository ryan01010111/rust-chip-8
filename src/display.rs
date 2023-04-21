use crossterm::{
    cursor, execute, queue,
    style::{Color, Print, SetForegroundColor},
    terminal,
};
use std::io::{self, stdout, Write};

pub const COLS: usize = 64;
pub const ROWS: usize = 32;
const NUM_OF_BLOCKS: usize = COLS * ROWS;

pub struct Display {
    stdout: io::Stdout,
    block_arr: [u8; NUM_OF_BLOCKS],
}

impl Display {
    pub fn new() -> Self {
        Self {
            stdout: stdout(),
            block_arr: [0; NUM_OF_BLOCKS],
        }
    }

    pub fn init(&mut self) -> Result<(), io::Error> {
        terminal::enable_raw_mode()?;
        execute!(
            self.stdout,
            terminal::EnterAlternateScreen,
            cursor::Hide,
            SetForegroundColor(Color::Green),
        )
        .unwrap_or_else(|err| {
            terminal::disable_raw_mode().unwrap();
            panic!("Failed to initialize display: {:?}", err);
        });

        Ok(())
    }

    pub fn exit(&mut self) -> Result<(), io::Error> {
        terminal::disable_raw_mode()?;
        execute!(self.stdout, terminal::LeaveAlternateScreen, cursor::Show,)?;

        Ok(())
    }

    pub fn set_block(&mut self, x: u16, y: u16) -> bool {
        let block_idx = (x + (y * COLS as u16)) as usize;
        self.block_arr[block_idx] ^= 1; // toggle block

        self.block_arr[block_idx] == 0 // returns true if block erased
    }

    pub fn clear(&mut self) {
        self.block_arr.fill(0);
    }

    pub fn render(&mut self) -> Result<(), io::Error> {
        queue!(self.stdout, cursor::MoveTo(0, 0))?;

        let top_bottom_border = "=".repeat(COLS * 2);
        queue!(
            self.stdout,
            Print(" "),
            Print(&top_bottom_border),
            cursor::MoveDown(1),
            cursor::MoveToColumn(0),
            Print("|"),
        )?;

        for (idx, block) in self.block_arr.iter().enumerate() {
            let row = idx / COLS;

            queue!(self.stdout, Print(if *block == 1 { "██" } else { "  " }))?;

            // end of row
            if (idx + 1) % COLS == 0 {
                queue!(
                    self.stdout,
                    Print("|"),
                    cursor::MoveDown(1),
                    cursor::MoveToColumn(0),
                )?;

                // row left border
                if row != ROWS - 1 {
                    queue!(self.stdout, Print("|"))?;
                }
            }
        }

        queue!(self.stdout, Print(" "), Print(&top_bottom_border),)?;

        self.render_bottom_bar(false)?;

        self.stdout.flush()?;

        Ok(())
    }

    fn render_bottom_bar(&mut self, paused: bool) -> Result<(), io::Error> {
        queue!(
            self.stdout,
            cursor::MoveTo(0, ROWS as u16 + 2),
            terminal::Clear(terminal::ClearType::UntilNewLine),
            Print(format!(
                " {} KEY MAP: SPACE",
                if paused {
                    "RESUME / HIDE"
                } else {
                    "PAUSE / SHOW"
                },
            )),
            cursor::MoveToColumn((COLS as u16 * 2) - 8),
            Print("EXIT: ESC\n"),
            cursor::MoveToColumn(1),
            Print("=".repeat(COLS * 2)),
        )?;

        Ok(())
    }

    pub fn render_key_map(&mut self) -> Result<(), io::Error> {
        let margin = 16;
        let row_len = 16;
        let y_start = 12;
        let grid_1_x = 41;
        let grid_2_x = grid_1_x + row_len + margin;

        queue!(self.stdout, terminal::Clear(terminal::ClearType::All))?;

        queue!(
            self.stdout,
            cursor::MoveTo(grid_1_x, y_start),
            Print("HEX\n\n"),
            cursor::MoveToColumn(grid_1_x),
            Print("1    2    3    C\n\n"),
            cursor::MoveToColumn(grid_1_x),
            Print("4    5    6    D\n\n"),
            cursor::MoveToColumn(grid_1_x),
            Print("7    8    9    E\n\n"),
            cursor::MoveToColumn(grid_1_x),
            Print("A    0    B    F"),
        )?;

        queue!(
            self.stdout,
            cursor::MoveTo(grid_1_x + row_len + (margin / 2) - 2, y_start + 5,),
            Print("--->"),
        )?;

        queue!(
            self.stdout,
            cursor::MoveTo(grid_2_x, y_start),
            Print("QWERTY\n\n"),
            cursor::MoveToColumn(grid_2_x),
            Print("1    2    3    4\n\n"),
            cursor::MoveToColumn(grid_2_x),
            Print("q    w    e    r\n\n"),
            cursor::MoveToColumn(grid_2_x),
            Print("a    s    d    f\n\n"),
            cursor::MoveToColumn(grid_2_x),
            Print("z    x    c    v"),
        )?;

        self.render_bottom_bar(true)?;

        self.stdout.flush()?;

        Ok(())
    }
}
