use crossterm::{
    cursor, execute,
    terminal::{self, ClearType},
};
use std::{
    io::{Result, Write, stdout},
    thread,
    time::Duration,
};

struct Crab<'a> {
    frames: [&'a str; 4],
    frame_idx: usize,
    pos: usize,
}

impl<'a> Crab<'a> {
    fn next_frame(&mut self) -> &str {
        self.frame_idx = (self.frame_idx + 1) % self.frames.len();
        self.frames[self.frame_idx]
    }
}

fn main() -> Result<()> {
    let mut stdout = stdout();

    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::Clear(ClearType::All))?;

    // 4-frame cycle
    let crab1_frames = [">(,∞,,)~<", ">~(,,∞,)☼<", ">(,∞,,)~<", ">~(,,∞,)><"];

    let var_name = ["<~(,,∞,)<", "<☼(∞,,)~<", "<~(,,∞,)<", "<>(∞,,)~<"];
    let crab2_frames = var_name;

    let mut crabs = [
        Crab {
            frames: crab1_frames,
            frame_idx: 0,
            pos: 0,
        },
        Crab {
            frames: crab2_frames,
            frame_idx: 0,
            pos: 10,
        },
    ];

    let width = 60;
    let mut lane: Vec<char> = "-".repeat(width).chars().collect();

    loop {
        // rebuild lane each frame
        for c in lane.iter_mut() {
            if *c == ' ' {
                *c = '-';
            }
        }

        // update crabs
        for crab in crabs.iter_mut() {
            let current_pos = crab.pos;
            let glyph = crab.next_frame();

            // eat if dash in front
            if current_pos < width && lane[current_pos] == '-' {
                lane[current_pos] = ' ';
            }

            // print frame
            execute!(stdout, cursor::MoveTo(current_pos as u16, 0),)?;
            print!("{}", glyph);

            // move forward - update position using modulo to wrap around
            crab.pos = (current_pos + 1) % width;
        }

        stdout.flush()?;
        thread::sleep(Duration::from_millis(120));

        // Clear row
        execute!(
            stdout,
            cursor::MoveTo(0, 0),
            terminal::Clear(ClearType::CurrentLine)
        )?;
        // Re-print lane
        let lane_str: String = lane.iter().collect();
        println!("{}", lane_str);
    }
}
