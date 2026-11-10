use std::io::{self, BufRead, Write};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        writeln!(stdout, "echo: {line}")?;
        stdout.flush()?;
    }
    Ok(())
}
